use pisi_bump_common::{PackageRecipe, compare_versions, normalize_version};

use crate::candidate_choice::choose_candidate;
use crate::error::{FetchError, GithubError, UpstreamError};
use crate::github_upstream::{GithubArchive, GithubLookup, LatestRelease, parse_github_archive};
use crate::report_model::{PackageReport, Status};

pub const UNSUPPORTED_DETAIL: &str = "GitHub dışı veya desteklenmeyen arşiv URL'si";
pub const RATE_LIMIT_DETAIL: &str = "rate limit";

pub type HashArchive<'a> = dyn Fn(&str) -> Result<String, FetchError> + 'a;

pub struct PackageChecker<'a> {
    lookup: &'a GithubLookup,
    hash_archive: Option<&'a HashArchive<'a>>,
}

struct CommonFields {
    name: String,
    current_version: String,
    latest_version: String,
    upstream: String,
    release_url: String,
    recipe_path: String,
}

impl CommonFields {
    fn apply(&self, status: Status) -> PackageReport {
        PackageReport::new(&self.name, status, &self.current_version)
            .with_latest_version(&self.latest_version)
            .with_upstream(&self.upstream)
            .with_release_url(&self.release_url)
            .with_recipe_path(&self.recipe_path)
    }
}

impl<'a> PackageChecker<'a> {
    pub fn new(lookup: &'a GithubLookup, hash_archive: Option<&'a HashArchive<'a>>) -> Self {
        Self {
            lookup,
            hash_archive,
        }
    }

    pub fn check(&self, recipe: &PackageRecipe) -> PackageReport {
        let archive = recipe
            .archive_urls
            .first()
            .and_then(|url| parse_github_archive(url));
        let Some(archive) = archive else {
            return PackageReport::new(&recipe.name, Status::Unsupported, &recipe.current_version)
                .with_detail(Some(UNSUPPORTED_DETAIL.to_string()))
                .with_recipe_path(&recipe.recipe_path);
        };
        match self.lookup.latest(&archive.owner, &archive.repo) {
            Err(GithubError::RateLimited(_)) => self.failure(recipe, &archive, RATE_LIMIT_DETAIL),
            Err(GithubError::Upstream(error)) => {
                self.failure(recipe, &archive, &error_message(&error))
            }
            Ok(latest) => self.compare(recipe, &archive, &latest),
        }
    }

    fn failure(
        &self,
        recipe: &PackageRecipe,
        archive: &GithubArchive,
        reason: &str,
    ) -> PackageReport {
        let upstream = format!("{}/{}", archive.owner, archive.repo);
        PackageReport::new(&recipe.name, Status::Error, &recipe.current_version)
            .with_upstream(upstream)
            .with_detail(Some(reason.to_string()))
            .with_recipe_path(&recipe.recipe_path)
    }

    fn compare(
        &self,
        recipe: &PackageRecipe,
        archive: &GithubArchive,
        latest: &LatestRelease,
    ) -> PackageReport {
        let prefixes = [archive.repo.as_str(), recipe.name.as_str()];
        let current = normalize_version(&recipe.current_version, &prefixes);
        let newest = normalize_version(&latest.tag, &prefixes);
        let fields = CommonFields {
            name: recipe.name.clone(),
            current_version: recipe.current_version.clone(),
            latest_version: latest.tag.clone(),
            upstream: format!("{}/{}", archive.owner, archive.repo),
            release_url: latest.release_url.clone(),
            recipe_path: recipe.recipe_path.clone(),
        };
        let (Some(current), Some(newest)) = (current, newest) else {
            return fields.apply(Status::Uncomparable);
        };
        if compare_versions(&newest, &current) <= 0 {
            return fields.apply(Status::Current);
        }
        self.outdated(recipe, archive, fields, latest)
    }

    fn outdated(
        &self,
        recipe: &PackageRecipe,
        archive: &GithubArchive,
        fields: CommonFields,
        latest: &LatestRelease,
    ) -> PackageReport {
        let choice = choose_candidate(recipe, archive, latest);
        let (sha1, hash_detail) = self.hash(choice.url.as_deref());
        fields
            .apply(Status::Outdated)
            .with_candidate_url(choice.url)
            .with_candidate_sha1(sha1)
            .with_detail(choice.reason.or(hash_detail))
    }

    fn hash(&self, candidate: Option<&str>) -> (Option<String>, Option<String>) {
        let (Some(hash_archive), Some(candidate)) = (self.hash_archive, candidate) else {
            return (None, None);
        };
        match hash_archive(candidate) {
            Ok(sha1) => (Some(sha1), None),
            Err(error) => (None, Some(format!("sha1 hesaplanamadı: {error}"))),
        }
    }
}

fn error_message(error: &UpstreamError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github_upstream::Fetch;
    use crate::http_client::HttpResponse;
    use std::collections::HashMap;

    const RELEASE_URL: &str = "https://github.com/o/r/archive/refs/tags/v1.0.tar.gz";

    fn recipe(version: &str, url: &str) -> PackageRecipe {
        PackageRecipe {
            recipe_path: "pkg/pspec.xml".to_string(),
            name: "pkg".to_string(),
            current_version: version.to_string(),
            current_release: 1,
            archive_urls: vec![url.to_string()],
        }
    }

    fn json_response(body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn empty_response(status: u16, headers: &[(&str, &str)]) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                .collect(),
            body: Vec::new(),
        }
    }

    fn routed_fetch(routes: Vec<(&'static str, HttpResponse)>) -> Fetch {
        Box::new(move |url: &str, _headers: &[(String, String)]| {
            for (prefix, response) in &routes {
                if url.starts_with(prefix) {
                    return Ok(response.clone());
                }
            }
            Ok(empty_response(404, &[]))
        })
    }

    fn checker_with_tag(tag: Option<&str>) -> GithubLookup {
        let routes = match tag {
            None => vec![],
            Some(tag) => vec![(
                "https://api.github.com/repos/o/r/releases/latest",
                json_response(&format!("{{\"tag_name\":\"{tag}\"}}")),
            )],
        };
        GithubLookup::new(routed_fetch(routes), None)
    }

    #[test]
    fn should_report_outdated_with_candidate_when_newer_release_exists() {
        let lookup = checker_with_tag(Some("v1.1"));
        let checker = PackageChecker::new(&lookup, None);
        let report = checker.check(&recipe("1.0", RELEASE_URL));
        assert_eq!(report.status, Status::Outdated);
        assert_eq!(
            report.candidate_url,
            Some("https://github.com/o/r/archive/refs/tags/v1.1.tar.gz".to_string())
        );
    }

    #[test]
    fn should_report_current_when_versions_match_after_normalization() {
        let lookup = checker_with_tag(Some("v1.0"));
        let checker = PackageChecker::new(&lookup, None);
        assert_eq!(
            checker.check(&recipe("1.0", RELEASE_URL)).status,
            Status::Current
        );
    }

    #[test]
    fn should_report_unsupported_when_archive_is_not_github() {
        let lookup = checker_with_tag(Some("v2"));
        let checker = PackageChecker::new(&lookup, None);
        let report = checker.check(&recipe("1.0", "https://example.org/a.tar.gz"));
        assert_eq!(report.status, Status::Unsupported);
    }

    #[test]
    fn should_report_unsupported_when_no_archive_exists() {
        let lookup = checker_with_tag(Some("v2"));
        let checker = PackageChecker::new(&lookup, None);
        let recipe = PackageRecipe {
            recipe_path: String::new(),
            name: "pkg".to_string(),
            current_version: "1".to_string(),
            current_release: 1,
            archive_urls: Vec::new(),
        };
        assert_eq!(checker.check(&recipe).status, Status::Unsupported);
    }

    #[test]
    fn should_report_uncomparable_when_tag_has_no_version() {
        let lookup = checker_with_tag(Some("latest"));
        let checker = PackageChecker::new(&lookup, None);
        assert_eq!(
            checker.check(&recipe("1.0", RELEASE_URL)).status,
            Status::Uncomparable
        );
    }

    #[test]
    fn should_report_error_when_repository_lookup_fails() {
        let lookup = checker_with_tag(None);
        let checker = PackageChecker::new(&lookup, None);
        assert_eq!(
            checker.check(&recipe("1.0", RELEASE_URL)).status,
            Status::Error
        );
    }

    #[test]
    fn should_report_rate_limit_when_quota_exhausted() {
        let routes = vec![(
            "https://api.github.com/",
            empty_response(403, &[("x-ratelimit-remaining", "0")]),
        )];
        let lookup = GithubLookup::new(routed_fetch(routes), None);
        let checker = PackageChecker::new(&lookup, None);
        let report = checker.check(&recipe("1.0", RELEASE_URL));
        assert_eq!(
            (report.status, report.detail.as_deref()),
            (Status::Error, Some(RATE_LIMIT_DETAIL))
        );
    }

    #[test]
    fn should_include_sha1_when_hash_function_is_given() {
        let lookup = checker_with_tag(Some("v1.1"));
        let hash_fn: &HashArchive = &|_url: &str| Ok("abc123".to_string());
        let checker = PackageChecker::new(&lookup, Some(hash_fn));
        let report = checker.check(&recipe("1.0", RELEASE_URL));
        assert_eq!(report.candidate_sha1, Some("abc123".to_string()));
    }

    #[test]
    fn should_keep_outdated_status_when_hashing_fails() {
        let lookup = checker_with_tag(Some("v1.1"));
        let hash_fn: &HashArchive = &|url: &str| {
            Err(FetchError::Other {
                url: url.to_string(),
                reason: "boom".to_string(),
            })
        };
        let checker = PackageChecker::new(&lookup, Some(hash_fn));
        let report = checker.check(&recipe("1.0", RELEASE_URL));
        assert_eq!(report.status, Status::Outdated);
        assert_eq!(report.candidate_sha1, None);
        assert!(report.detail.unwrap().contains("sha1"));
    }
}
