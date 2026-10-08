use fancy_regex::{NoExpand, Regex as FancyRegex};
use regex::Regex;

use pisi_bump_common::extract_version_text;

const SEPARATORS: [&str; 2] = ["_", "-"];
const NOT_AFTER_TOKEN: &str = r"(?!\d|\.\d)";
const NOT_BEFORE_TOKEN: &str = r"(?<![A-Za-z0-9])";

pub fn version_variants(version: &str) -> Vec<String> {
    let mut variants = vec![version.to_string()];
    if version.contains('.') {
        for separator in SEPARATORS {
            variants.push(version.replace('.', separator));
        }
    }
    variants
}

pub fn replace_token(text: &str, old_token: &str, new_token: &str) -> String {
    if old_token.is_empty() || old_token == new_token {
        return text.to_string();
    }
    let pattern = format!(
        "{NOT_BEFORE_TOKEN}{}{NOT_AFTER_TOKEN}",
        regex::escape(old_token)
    );
    let regex = FancyRegex::new(&pattern).expect("candidate_url token pattern must compile");
    regex.replace_all(text, NoExpand(new_token)).into_owned()
}

pub fn replace_version(text: &str, old_version: &str, new_version: &str) -> String {
    let old_variants = version_variants(old_version);
    let new_variants = version_variants(new_version);
    let mut result = text.to_string();
    for (old_variant, new_variant) in old_variants.iter().zip(new_variants.iter()) {
        result = replace_token(&result, old_variant, new_variant);
    }
    result
}

pub fn build_candidate_url(
    archive_url: &str,
    old_tag: &str,
    new_tag: &str,
    old_version: &str,
    new_version: &str,
) -> String {
    let retagged = replace_token(archive_url, old_tag, new_tag);
    match retagged.rfind('/') {
        Some(index) => {
            let (directory, rest) = retagged.split_at(index + 1);
            format!(
                "{directory}{}",
                replace_version(rest, old_version, new_version)
            )
        }
        None => replace_version(&retagged, old_version, new_version),
    }
}

fn underscore_to_dot() -> &'static Regex {
    use std::sync::LazyLock;
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[_-]").unwrap());
    &PATTERN
}

pub fn derive_new_version(
    new_tag: &str,
    prefixes: &[&str],
    current_version: &str,
) -> Option<String> {
    let text = extract_version_text(new_tag, prefixes)?;
    if current_version.contains('.') {
        Some(underscore_to_dot().replace_all(&text, ".").into_owned())
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::build_candidate_url;

    #[test]
    fn should_replace_tag_and_file_version_when_brave_style() {
        let url = "https://github.com/brave/brave-browser/releases/download/v1.93.129/brave-browser-1.93.129-linux-amd64.zip";
        let result = build_candidate_url(url, "v1.93.129", "v1.94.1", "1.93.129", "1.94.1");
        assert_eq!(
            result,
            "https://github.com/brave/brave-browser/releases/download/v1.94.1/brave-browser-1.94.1-linux-amd64.zip"
        );
    }

    #[test]
    fn should_update_dotted_filename_when_tag_uses_underscores() {
        let url = "https://github.com/atari800/atari800/releases/download/ATARI800_5_2_0/atari800-5.2.0-src.tgz";
        let result = build_candidate_url(url, "ATARI800_5_2_0", "ATARI800_7_2_1", "5.2.0", "7.2.1");
        assert_eq!(
            result,
            "https://github.com/atari800/atari800/releases/download/ATARI800_7_2_1/atari800-7.2.1-src.tgz"
        );
    }

    #[test]
    fn should_keep_file_name_when_it_has_no_version() {
        let url =
            "https://github.com/OpenRA/d2/releases/download/release-20250330/Dune2000-linux.zip";
        let result = build_candidate_url(
            url,
            "release-20250330",
            "release-20250601",
            "20250330",
            "20250601",
        );
        assert_eq!(
            result,
            "https://github.com/OpenRA/d2/releases/download/release-20250601/Dune2000-linux.zip"
        );
    }

    #[test]
    fn should_replace_release_prefixed_tag_when_github_desktop_style() {
        let url = "https://github.com/shiftkey/desktop/releases/download/release-3.4.13-linux1/GitHubDesktop-linux-x86_64-3.4.13-linux1.AppImage";
        let result = build_candidate_url(
            url,
            "release-3.4.13-linux1",
            "release-3.4.14-linux1",
            "3.4.13",
            "3.4.14",
        );
        assert_eq!(
            result,
            "https://github.com/shiftkey/desktop/releases/download/release-3.4.14-linux1/GitHubDesktop-linux-x86_64-3.4.14-linux1.AppImage"
        );
    }

    #[test]
    fn should_replace_tag_inside_archive_file_name_when_gnofract4d_style() {
        let url = "https://github.com/fract4d/gnofract4d/archive/v4.4.tar.gz";
        let result = build_candidate_url(url, "v4.4", "v4.5", "4.4", "4.5");
        assert_eq!(
            result,
            "https://github.com/fract4d/gnofract4d/archive/v4.5.tar.gz"
        );
    }

    #[test]
    fn should_replace_version_in_appimage_name_when_freecad_style() {
        let url = "https://github.com/FreeCAD/FreeCAD/releases/download/1.0.1/FreeCAD_1.0.1-conda-Linux-x86_64-py311.AppImage";
        let result = build_candidate_url(url, "1.0.1", "1.0.2", "1.0.1", "1.0.2");
        assert_eq!(
            result,
            "https://github.com/FreeCAD/FreeCAD/releases/download/1.0.2/FreeCAD_1.0.2-conda-Linux-x86_64-py311.AppImage"
        );
    }

    #[test]
    fn should_not_touch_longer_version_when_old_version_is_prefix() {
        let url = "https://example.org/dl/tool-1.10.2.tar.gz";
        let result = build_candidate_url(url, "v1.1", "v1.2", "1.1", "1.2");
        assert_eq!(result, url);
    }

    #[test]
    fn should_return_url_unchanged_when_nothing_matches() {
        let url = "https://example.org/dl/latest.zip";
        let result = build_candidate_url(url, "v1", "v2", "1", "2");
        assert_eq!(result, url);
    }
}
