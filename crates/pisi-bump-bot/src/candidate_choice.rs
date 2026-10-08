use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use pisi_bump_common::PackageRecipe;

use crate::asset_selector::{VersionPair, select_asset};
use crate::candidate_url::{build_candidate_url, derive_new_version};
use crate::github_upstream::{GithubArchive, LatestRelease, is_release_download};

const RELEASE_DOWNLOAD_ROOT: &str = "https://github.com";

const PYTHON_QUOTE_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'_')
    .remove(b'.')
    .remove(b'-')
    .remove(b'~')
    .remove(b'/');

pub fn quote(text: &str) -> String {
    utf8_percent_encode(text, PYTHON_QUOTE_SAFE).to_string()
}

pub fn unquote(text: &str) -> String {
    percent_decode_str(text).decode_utf8_lossy().into_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateChoice {
    pub url: Option<String>,
    pub reason: Option<String>,
}

impl CandidateChoice {
    fn url(url: impl Into<String>) -> Self {
        Self {
            url: Some(url.into()),
            reason: None,
        }
    }

    fn none() -> Self {
        Self {
            url: None,
            reason: None,
        }
    }

    fn refused(reason: impl Into<String>) -> Self {
        Self {
            url: None,
            reason: Some(reason.into()),
        }
    }
}

fn file_name(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or(url)
}

fn select_release_asset(
    recipe: &PackageRecipe,
    archive: &GithubArchive,
    release: &LatestRelease,
    new_version: &str,
) -> CandidateChoice {
    let old_name = unquote(file_name(&recipe.archive_urls[0]));
    let assets = release.assets.clone().unwrap_or_default();
    let selection = select_asset(
        &old_name,
        VersionPair::new(&recipe.current_version, new_version),
        &assets,
    );
    match selection.name {
        None => CandidateChoice::refused(selection.reason.unwrap_or_default()),
        Some(name) => {
            let base = format!(
                "{RELEASE_DOWNLOAD_ROOT}/{}/{}/releases/download/{}",
                archive.owner, archive.repo, release.tag
            );
            CandidateChoice::url(format!("{base}/{}", quote(&name)))
        }
    }
}

fn substitute_candidate(
    recipe: &PackageRecipe,
    archive: &GithubArchive,
    release: &LatestRelease,
    new_version: &str,
) -> CandidateChoice {
    let archive_url = &recipe.archive_urls[0];
    let candidate = build_candidate_url(
        archive_url,
        &archive.tag,
        &release.tag,
        &recipe.current_version,
        new_version,
    );
    if candidate == *archive_url {
        CandidateChoice::none()
    } else {
        CandidateChoice::url(candidate)
    }
}

pub fn choose_candidate(
    recipe: &PackageRecipe,
    archive: &GithubArchive,
    release: &LatestRelease,
) -> CandidateChoice {
    let prefixes = [archive.repo.as_str(), recipe.name.as_str()];
    let new_version = match derive_new_version(&release.tag, &prefixes, &recipe.current_version) {
        Some(version) => version,
        None => return CandidateChoice::none(),
    };
    if release.assets.is_some() && is_release_download(&recipe.archive_urls[0]) {
        return select_release_asset(recipe, archive, release, &new_version);
    }
    substitute_candidate(recipe, archive, release, &new_version)
}

#[cfg(test)]
mod tests {
    use super::{CandidateChoice, choose_candidate};
    use pisi_bump_common::PackageRecipe;

    use crate::github_upstream::GithubArchive;
    use crate::github_upstream::LatestRelease;

    fn recipe(url: &str) -> PackageRecipe {
        PackageRecipe {
            recipe_path: "r/pspec.xml".to_string(),
            name: "r".to_string(),
            current_version: "1.0".to_string(),
            current_release: 1,
            archive_urls: vec![url.to_string()],
        }
    }

    fn archive() -> GithubArchive {
        GithubArchive {
            owner: "o".to_string(),
            repo: "r".to_string(),
            tag: "v1.0".to_string(),
        }
    }

    fn release(assets: Option<Vec<&str>>) -> LatestRelease {
        LatestRelease {
            tag: "v1.1".to_string(),
            release_url: "https://github.com/o/r/releases/tag/v1.1".to_string(),
            assets: assets.map(|names| names.into_iter().map(str::to_string).collect()),
        }
    }

    const OLD_URL: &str =
        "https://github.com/o/r/releases/download/v1.0/r-linux-amd64-1.0.AppImage";

    #[test]
    fn should_use_release_asset_when_url_is_release_download() {
        let release = release(Some(vec![
            "r-linux-amd64-1.1.AppImage",
            "r-linux-amd64-1.1.AppImage.zsync",
            "r-win64-1.1.zip",
        ]));
        let choice = choose_candidate(&recipe(OLD_URL), &archive(), &release);
        assert_eq!(
            choice.url,
            Some(
                "https://github.com/o/r/releases/download/v1.1/r-linux-amd64-1.1.AppImage"
                    .to_string()
            )
        );
    }

    #[test]
    fn should_refuse_with_reason_when_no_asset_fits() {
        let release = release(Some(vec!["r-win64-1.1.zip"]));
        let choice = choose_candidate(&recipe(OLD_URL), &archive(), &release);
        assert_eq!(
            choice,
            CandidateChoice {
                url: None,
                reason: Some("uygun dosya bulunamadı (adaylar: r-win64-1.1.zip)".to_string())
            }
        );
    }

    #[test]
    fn should_substitute_when_release_payload_has_no_asset_list() {
        let release = release(None);
        let choice = choose_candidate(&recipe(OLD_URL), &archive(), &release);
        assert_eq!(
            choice.url,
            Some(
                "https://github.com/o/r/releases/download/v1.1/r-linux-amd64-1.1.AppImage"
                    .to_string()
            )
        );
    }

    #[test]
    fn should_keep_substitution_when_url_is_auto_generated_archive() {
        let url = "https://github.com/o/r/archive/refs/tags/v1.0.tar.gz";
        let release = release(Some(vec!["r-1.1.zip"]));
        let choice = choose_candidate(&recipe(url), &archive(), &release);
        assert_eq!(
            choice.url,
            Some("https://github.com/o/r/archive/refs/tags/v1.1.tar.gz".to_string())
        );
    }

    #[test]
    fn should_quote_asset_name_when_it_has_special_characters() {
        let old = "https://github.com/o/r/releases/download/v1.0/r_1.0 x86_64.AppImage";
        let release = release(Some(vec!["r_1.1 x86_64.AppImage"]));
        let choice = choose_candidate(&recipe(old), &archive(), &release);
        assert!(choice.url.unwrap().ends_with("/r_1.1%20x86_64.AppImage"));
    }
}
