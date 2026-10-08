use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::error::{FetchError, GithubError, RateLimitExceeded, UpstreamError};
use crate::http_client::{HttpResponse, USER_AGENT};

pub const API_ROOT: &str = "https://api.github.com";

const ARCHIVE_EXTENSION: &str = r"(?:tar\.gz|tgz|zip|tar\.bz2|tar\.xz)";

fn owner_repo() -> &'static str {
    r"(?P<owner>[^/]+)/(?P<repo>[^/]+)"
}

static ARCHIVE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(&format!(r"^https?://github\.com/{}/releases/download/(?P<tag>[^/]+)/[^/]+$", owner_repo())).unwrap(),
        Regex::new(&format!(r"^https?://github\.com/{}/archive/refs/tags/(?P<tag>[^/]+?)\.{ARCHIVE_EXTENSION}$", owner_repo())).unwrap(),
        Regex::new(&format!(r"^https?://github\.com/{}/archive/(?P<tag>[^/]+?)\.{ARCHIVE_EXTENSION}$", owner_repo())).unwrap(),
        Regex::new(&format!(
            r"^https?://codeload\.github\.com/{}/(?:tar\.gz|zip)/(?:refs/tags/)?(?P<tag>[^/]+)$",
            owner_repo()
        ))
        .unwrap(),
        Regex::new(&format!(
            r"^https?://codeload\.github\.com/{}/(?:legacy\.)?(?:tar\.gz|zip)/(?:refs/tags/)?(?P<tag>[^/]+)$",
            owner_repo()
        ))
        .unwrap(),
    ]
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubArchive {
    pub owner: String,
    pub repo: String,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRelease {
    pub tag: String,
    pub release_url: String,
    pub assets: Option<Vec<String>>,
}

pub fn is_release_download(url: &str) -> bool {
    ARCHIVE_PATTERNS[0].is_match(url)
}

pub fn parse_github_archive(url: &str) -> Option<GithubArchive> {
    for pattern in ARCHIVE_PATTERNS.iter() {
        if let Some(captures) = pattern.captures(url) {
            return Some(GithubArchive {
                owner: captures["owner"].to_string(),
                repo: captures["repo"].to_string(),
                tag: captures["tag"].to_string(),
            });
        }
    }
    None
}

fn asset_names(payload: &Value) -> Option<Vec<String>> {
    let assets = payload.get("assets")?.as_array()?;
    Some(
        assets
            .iter()
            .filter_map(|item| item.get("name")?.as_str().map(str::to_string))
            .collect(),
    )
}

fn is_rate_limited(response: &HttpResponse) -> bool {
    if response.status == 429 {
        return true;
    }
    response.status == 403 && response.header("x-ratelimit-remaining") == Some("0")
}

fn parse_json(response: &HttpResponse) -> Result<Value, UpstreamError> {
    serde_json::from_slice(&response.body)
        .map_err(|_error| UpstreamError::message("GitHub yanıtı çözümlenemedi"))
}

pub type Fetch = Box<dyn Fn(&str, &[(String, String)]) -> Result<HttpResponse, FetchError>>;

pub struct GithubLookup {
    fetch: Fetch,
    token: Option<String>,
    cache: RefCell<HashMap<(String, String), Result<LatestRelease, UpstreamError>>>,
    rate_limited: Cell<bool>,
}

impl GithubLookup {
    pub fn new(fetch: Fetch, token: Option<String>) -> Self {
        Self {
            fetch,
            token,
            cache: RefCell::new(HashMap::new()),
            rate_limited: Cell::new(false),
        }
    }

    pub fn latest(&self, owner: &str, repo: &str) -> Result<LatestRelease, GithubError> {
        let key = (owner.to_string(), repo.to_string());
        if !self.cache.borrow().contains_key(&key) {
            let resolved = self
                .resolve(owner, repo)
                .map_err(GithubError::RateLimited)?;
            self.cache.borrow_mut().insert(key.clone(), resolved);
        }
        match self.cache.borrow()[&key].clone() {
            Ok(release) => Ok(release),
            Err(error) => Err(GithubError::Upstream(error)),
        }
    }

    fn resolve(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Result<LatestRelease, UpstreamError>, RateLimitExceeded> {
        if self.rate_limited.get() {
            return Err(RateLimitExceeded);
        }
        match self.latest_release(owner, repo) {
            Ok(release) => Ok(Ok(release)),
            Err(GithubError::Upstream(error)) => Ok(Err(error)),
            Err(GithubError::RateLimited(limited)) => Err(limited),
        }
    }

    fn get(&self, path: &str) -> Result<HttpResponse, GithubError> {
        let mut headers = vec![
            ("User-Agent".to_string(), USER_AGENT.to_string()),
            (
                "Accept".to_string(),
                "application/vnd.github+json".to_string(),
            ),
        ];
        if let Some(token) = &self.token {
            headers.push(("Authorization".to_string(), format!("Bearer {token}")));
        }
        let url = format!("{API_ROOT}{path}");
        let response = (self.fetch)(&url, &headers)
            .map_err(|error| GithubError::Upstream(UpstreamError::from(error)))?;
        if is_rate_limited(&response) {
            self.rate_limited.set(true);
            return Err(GithubError::RateLimited(RateLimitExceeded));
        }
        Ok(response)
    }

    fn latest_release(&self, owner: &str, repo: &str) -> Result<LatestRelease, GithubError> {
        let response = self.get(&format!("/repos/{owner}/{repo}/releases/latest"))?;
        if response.status == 404 {
            return self.latest_tag(owner, repo);
        }
        if response.status != 200 {
            return Err(upstream(format!("GitHub HTTP {}", response.status)));
        }
        let payload = parse_json(&response).map_err(GithubError::Upstream)?;
        let is_draft_or_prerelease = payload
            .get("draft")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || payload
                .get("prerelease")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if !payload.is_object() || is_draft_or_prerelease {
            return self.latest_tag(owner, repo);
        }
        let tag = payload.get("tag_name").and_then(Value::as_str);
        release_from_tag(owner, repo, tag, asset_names(&payload))
    }

    fn latest_tag(&self, owner: &str, repo: &str) -> Result<LatestRelease, GithubError> {
        let response = self.get(&format!("/repos/{owner}/{repo}/tags?per_page=1"))?;
        if response.status == 404 {
            return Err(upstream("depo bulunamadı"));
        }
        if response.status != 200 {
            return Err(upstream(format!("GitHub HTTP {}", response.status)));
        }
        let payload = parse_json(&response).map_err(GithubError::Upstream)?;
        let first_name = payload
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str);
        match first_name {
            Some(_) => release_from_tag(owner, repo, first_name, None),
            None => Err(upstream("release veya tag yok")),
        }
    }
}

fn upstream(message: impl Into<String>) -> GithubError {
    GithubError::Upstream(UpstreamError::message(message))
}

fn release_from_tag(
    owner: &str,
    repo: &str,
    tag: Option<&str>,
    assets: Option<Vec<String>>,
) -> Result<LatestRelease, GithubError> {
    match tag {
        Some(tag) if !tag.is_empty() => Ok(LatestRelease {
            tag: tag.to_string(),
            release_url: format!("https://github.com/{owner}/{repo}/releases/tag/{tag}"),
            assets,
        }),
        _ => Err(upstream("tag adı okunamadı")),
    }
}

#[cfg(test)]
mod tests {
    use super::{GithubArchive, GithubLookup, parse_github_archive};
    use crate::error::{GithubError, RateLimitExceeded};
    use crate::http_client::HttpResponse;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    fn json_response(body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn empty_response(status: u16, headers: &[(&str, &str)]) -> HttpResponse {
        let headers = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            .collect();
        HttpResponse {
            status,
            headers,
            body: Vec::new(),
        }
    }

    fn routed_lookup(
        routes: Vec<(&'static str, HttpResponse)>,
        calls: Rc<RefCell<Vec<String>>>,
        token: Option<String>,
    ) -> GithubLookup {
        let fetch = Box::new(move |url: &str, _headers: &[(String, String)]| {
            calls.borrow_mut().push(url.to_string());
            for (prefix, response) in &routes {
                if url.starts_with(prefix) {
                    return Ok(response.clone());
                }
            }
            Ok(empty_response(404, &[]))
        });
        GithubLookup::new(fetch, token)
    }

    #[test]
    fn should_parse_release_download_url() {
        let url = "https://github.com/atom/atom/releases/download/v1.57.0/atom-amd64.tar.gz";
        assert_eq!(
            parse_github_archive(url),
            Some(GithubArchive {
                owner: "atom".into(),
                repo: "atom".into(),
                tag: "v1.57.0".into()
            })
        );
    }

    #[test]
    fn should_parse_refs_tags_archive_url() {
        let url = "https://github.com/ttcdt/mp-5.x/archive/refs/tags/5.55.tar.gz";
        assert_eq!(
            parse_github_archive(url),
            Some(GithubArchive {
                owner: "ttcdt".into(),
                repo: "mp-5.x".into(),
                tag: "5.55".into()
            })
        );
    }

    #[test]
    fn should_parse_plain_archive_url_when_tag_contains_dots() {
        let url = "https://github.com/bulletphysics/bullet3/archive/3.08.tar.gz";
        assert_eq!(parse_github_archive(url).unwrap().tag, "3.08");
    }

    #[test]
    fn should_parse_codeload_url() {
        let url = "https://codeload.github.com/soimort/translate-shell/tar.gz/refs/tags/v0.9.6.12";
        assert_eq!(
            parse_github_archive(url),
            Some(GithubArchive {
                owner: "soimort".into(),
                repo: "translate-shell".into(),
                tag: "v0.9.6.12".into()
            })
        );
    }

    #[test]
    fn should_return_none_when_archive_is_branch_snapshot() {
        assert_eq!(
            parse_github_archive("https://github.com/a/b/archive/refs/heads/master.zip"),
            None
        );
    }

    #[test]
    fn should_return_none_when_host_is_not_github() {
        assert_eq!(
            parse_github_archive("https://example.org/a/b/archive/1.0.tar.gz"),
            None
        );
    }

    #[test]
    fn should_return_none_when_url_has_typo_scheme() {
        assert_eq!(
            parse_github_archive("hhttps://github.com/a/b/archive/1.0.tar.gz"),
            None
        );
    }

    #[test]
    fn should_return_none_when_url_is_raw_file() {
        assert_eq!(
            parse_github_archive("https://github.com/groni/Sources/raw/master/x-1.0.tar.gz"),
            None
        );
    }

    #[test]
    fn should_return_latest_release_when_endpoint_succeeds() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![(
                "https://api.github.com/repos/o/r/releases/latest",
                json_response(r#"{"tag_name":"v2.0"}"#),
            )],
            calls,
            None,
        );
        let result = lookup.latest("o", "r").unwrap();
        assert_eq!(
            (result.tag.as_str(), result.release_url.as_str()),
            ("v2.0", "https://github.com/o/r/releases/tag/v2.0")
        );
    }

    #[test]
    fn should_fall_back_to_tags_when_no_release_exists() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![
                (
                    "https://api.github.com/repos/o/r/releases/latest",
                    empty_response(404, &[]),
                ),
                (
                    "https://api.github.com/repos/o/r/tags",
                    json_response(r#"[{"name":"t1"}]"#),
                ),
            ],
            calls,
            None,
        );
        assert_eq!(lookup.latest("o", "r").unwrap().tag, "t1");
    }

    #[test]
    fn should_fall_back_to_tags_when_release_is_prerelease() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![
                (
                    "https://api.github.com/repos/o/r/releases/latest",
                    json_response(r#"{"tag_name":"x","prerelease":true}"#),
                ),
                (
                    "https://api.github.com/repos/o/r/tags",
                    json_response(r#"[{"name":"t2"}]"#),
                ),
            ],
            calls,
            None,
        );
        assert_eq!(lookup.latest("o", "r").unwrap().tag, "t2");
    }

    #[test]
    fn should_raise_upstream_error_when_repository_missing() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(vec![], calls, None);
        assert!(matches!(
            lookup.latest("o", "r"),
            Err(GithubError::Upstream(_))
        ));
    }

    #[test]
    fn should_raise_rate_limit_when_quota_exhausted() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![(
                "https://api.github.com/repos/o/r",
                empty_response(403, &[("x-ratelimit-remaining", "0")]),
            )],
            calls,
            None,
        );
        assert!(matches!(
            lookup.latest("o", "r"),
            Err(GithubError::RateLimited(RateLimitExceeded))
        ));
    }

    #[test]
    fn should_not_call_api_again_when_rate_limited() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![("https://api.github.com/repos/o", empty_response(429, &[]))],
            calls.clone(),
            None,
        );
        assert!(lookup.latest("o", "r").is_err());
        assert!(lookup.latest("o", "other").is_err());
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn should_request_once_when_same_repository_is_asked_twice() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![(
                "https://api.github.com/repos/o/r/releases/latest",
                json_response(r#"{"tag_name":"v1"}"#),
            )],
            calls.clone(),
            None,
        );
        lookup.latest("o", "r").unwrap();
        lookup.latest("o", "r").unwrap();
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn should_send_authorization_header_when_token_given() {
        let seen: Rc<RefCell<HashMap<String, String>>> = Rc::new(RefCell::new(HashMap::new()));
        let seen_clone = seen.clone();
        let fetch = Box::new(move |_url: &str, headers: &[(String, String)]| {
            for (name, value) in headers {
                seen_clone.borrow_mut().insert(name.clone(), value.clone());
            }
            Ok(json_response(r#"{"tag_name":"v1"}"#))
        });
        let lookup = GithubLookup::new(fetch, Some("secret".to_string()));
        lookup.latest("o", "r").unwrap();
        assert_eq!(
            seen.borrow().get("Authorization"),
            Some(&"Bearer secret".to_string())
        );
    }

    #[test]
    fn should_raise_upstream_error_when_json_is_invalid() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let lookup = routed_lookup(
            vec![(
                "https://api.github.com/repos/o/r/releases/latest",
                empty_response(200, &[]),
            )],
            calls,
            None,
        );
        assert!(matches!(
            lookup.latest("o", "r"),
            Err(GithubError::Upstream(_))
        ));
    }
}
