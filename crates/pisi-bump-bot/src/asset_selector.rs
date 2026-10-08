use std::sync::LazyLock;

use fancy_regex::{NoExpand, Regex as FancyRegex};
use regex::Regex;

use crate::asset_filters::{
    is_helper_file, matches_architecture, matches_file_type, matches_system,
};
use crate::candidate_url::version_variants;
use crate::difflib::SequenceMatcher;

pub const MIN_SIMILARITY: f64 = 0.6;
pub const MIN_MARGIN: f64 = 0.05;
const MAX_LISTED_NAMES: usize = 5;
const VERSION_PLACEHOLDER: &str = "@version@";
const ARCHITECTURE_PLACEHOLDER: &str = "@arch@";

static ARCHITECTURE_SYNONYMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:x86[_-]64|amd64|x64|linux64|64-?bit)").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPair<'a> {
    pub old: &'a str,
    pub new: &'a str,
}

impl<'a> VersionPair<'a> {
    pub fn new(old: &'a str, new: &'a str) -> Self {
        Self { old, new }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetSelection {
    pub name: Option<String>,
    pub similarity: f64,
    pub reason: Option<String>,
}

fn mask_token(name: &str, variant: &str) -> FancyRegex {
    let pattern = format!(
        r"(?i)(?<![A-Za-z0-9])v?{}(?!\d|\.\d)",
        regex::escape(variant)
    );
    FancyRegex::new(&pattern)
        .unwrap_or_else(|_| panic!("asset_selector mask_version pattern must compile for {name}"))
}

pub fn mask_version(name: &str, version: &str) -> String {
    let mut masked = name.to_string();
    for variant in version_variants(version) {
        let regex = mask_token(name, &variant);
        masked = regex
            .replace_all(&masked, NoExpand(VERSION_PLACEHOLDER))
            .into_owned();
    }
    masked
}

pub fn comparable(name: &str, version: &str) -> String {
    let masked = mask_version(name, version).to_lowercase();
    ARCHITECTURE_SYNONYMS
        .replace_all(&masked, ARCHITECTURE_PLACEHOLDER)
        .into_owned()
}

fn collapse_separators() -> &'static Regex {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-_.\s]+").unwrap());
    &PATTERN
}

pub fn product_stem(name: &str, version: &str) -> String {
    let text = comparable(name, version);
    let stem = text.split(VERSION_PLACEHOLDER).next().unwrap_or("");
    collapse_separators()
        .replace_all(stem, "-")
        .trim_matches('-')
        .to_string()
}

pub fn has_same_product(old_name: &str, candidate: &str, versions: VersionPair) -> bool {
    product_stem(old_name, versions.old) == product_stem(candidate, versions.new)
}

pub fn similarity(old_name: &str, candidate: &str, versions: VersionPair) -> f64 {
    let left: Vec<char> = comparable(old_name, versions.old).chars().collect();
    let right: Vec<char> = comparable(candidate, versions.new).chars().collect();
    SequenceMatcher::new(&left, &right).ratio()
}

pub fn eligible_assets<'a>(
    old_name: &str,
    assets: &'a [String],
    versions: VersionPair,
) -> Vec<&'a str> {
    assets
        .iter()
        .map(String::as_str)
        .filter(|name| !is_helper_file(name))
        .filter(|name| {
            matches_file_type(old_name, name)
                && matches_architecture(old_name, name)
                && matches_system(old_name, name)
                && has_same_product(old_name, name, versions)
        })
        .collect()
}

pub fn refusal_reason<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let listed: Vec<&str> = names.take(MAX_LISTED_NAMES).collect();
    let joined = if listed.is_empty() {
        "yok".to_string()
    } else {
        listed.join(", ")
    };
    format!("uygun dosya bulunamadı (adaylar: {joined})")
}

fn is_ambiguous(ranked: &[(f64, &str)]) -> bool {
    ranked.len() > 1 && ranked[0].0 - ranked[1].0 < MIN_MARGIN
}

pub fn select_asset(old_name: &str, versions: VersionPair, assets: &[String]) -> AssetSelection {
    let eligible = eligible_assets(old_name, assets, versions);
    let mut ranked: Vec<(f64, &str)> = eligible
        .iter()
        .map(|&name| (similarity(old_name, name, versions), name))
        .collect();
    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap()
            .then_with(|| left.1.cmp(right.1))
    });

    let shown: Vec<&str> = if !ranked.is_empty() {
        ranked.iter().map(|(_, name)| *name).collect()
    } else {
        assets
            .iter()
            .map(String::as_str)
            .filter(|name| !is_helper_file(name))
            .collect()
    };

    if ranked.is_empty() || ranked[0].0 < MIN_SIMILARITY || is_ambiguous(&ranked) {
        let best = ranked.first().map(|(score, _)| *score).unwrap_or(0.0);
        return AssetSelection {
            name: None,
            similarity: best,
            reason: Some(refusal_reason(shown.into_iter())),
        };
    }

    AssetSelection {
        name: Some(ranked[0].1.to_string()),
        similarity: ranked[0].0,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetSelection, VersionPair, select_asset};
    use crate::asset_filters::{architecture_family, file_type, is_helper_file, system_family};
    use std::fs;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/assets")
    }

    fn load_assets(name: &str) -> Vec<String> {
        let path = fixtures_dir().join(format!("{name}.json"));
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        serde_json::from_str(&text).unwrap()
    }

    fn pick(fixture: &str, old_name: &str, old: &str, new: &str) -> Option<String> {
        select_asset(old_name, VersionPair::new(old, new), &load_assets(fixture)).name
    }

    #[test]
    fn should_pick_freecad_appimage_when_conda_marker_was_dropped() {
        let picked = pick(
            "freecad",
            "FreeCAD_1.0.1-conda-Linux-x86_64-py311.AppImage",
            "1.0.1",
            "1.1.4",
        );
        assert_eq!(
            picked,
            Some("FreeCAD_1.1.4-Linux-x86_64-py311.AppImage".to_string())
        );
    }

    #[test]
    fn should_pick_pencil2d_amd64_when_old_name_has_no_v_prefix() {
        let picked = pick(
            "pencil2d",
            "pencil2d-linux-amd64-0.6.6.AppImage",
            "0.6.6",
            "0.7.2",
        );
        assert_eq!(
            picked,
            Some("pencil2d-linux-amd64-v0.7.2.AppImage".to_string())
        );
    }

    #[test]
    fn should_pick_synfig_linux64_when_date_and_hash_changed() {
        let old_name = "SynfigStudio-1.5.1-2021.10.21-linux64-2cb6c.appimage";
        let picked = pick("synfig", old_name, "1.5.1", "1.5.5");
        assert_eq!(
            picked,
            Some("SynfigStudio-1.5.5-2026.03.15-linux64-79bf7.appimage".to_string())
        );
    }

    #[test]
    fn should_pick_iptvnator_deb_when_naming_scheme_changed() {
        let picked = pick(
            "iptvnator",
            "iptvnator_0.15.0_amd64.deb",
            "0.15.0",
            "0.24.0",
        );
        assert_eq!(picked, Some("iptvnator-0.24.0-linux-amd64.deb".to_string()));
    }

    #[test]
    fn should_refuse_with_candidate_list_when_only_foreign_platforms_exist() {
        let assets: Vec<String> = [
            "app-1.1-win64.zip",
            "app-1.1-mac.dmg",
            "app-1.1-linux-arm64.zip",
            "a.zsync",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let selection = select_asset(
            "app-1.0-linux-x86_64.zip",
            VersionPair::new("1.0", "1.1"),
            &assets,
        );
        assert_eq!(selection.name, None);
        assert_eq!(
            selection.reason,
            Some("uygun dosya bulunamadı (adaylar: app-1.1-win64.zip, app-1.1-mac.dmg, app-1.1-linux-arm64.zip)".to_string())
        );
    }

    #[test]
    fn should_refuse_when_top_two_candidates_are_within_margin() {
        let assets: Vec<String> = [
            "tool-1.1-linux-x86_64-gtk.tar.gz",
            "tool-1.1-linux-x86_64-qt5.tar.gz",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let selection = select_asset(
            "tool-1.0-linux-x86_64.tar.gz",
            VersionPair::new("1.0", "1.1"),
            &assets,
        );
        assert_eq!(selection.name, None);
        assert!(selection.reason.unwrap().contains("gtk"));
    }

    #[test]
    fn should_refuse_when_best_similarity_is_below_threshold() {
        let assets = vec!["completely-different-name.zip".to_string()];
        let selection = select_asset("alpha-1.0.zip", VersionPair::new("1.0", "2.0"), &assets);
        assert_eq!(selection.name, None);
    }

    #[test]
    fn should_limit_listed_names_to_five() {
        let mut assets: Vec<String> = (0..8).map(|index| format!("x-{index}-win.zip")).collect();
        assets.push("x-linux.tar.gz".to_string());
        let selection = select_asset("x-1.0-linux.zip", VersionPair::new("1.0", "2.0"), &assets);
        assert_eq!(selection.reason.unwrap().matches(',').count(), 4);
    }

    #[test]
    fn should_flag_helper_files_when_extension_is_checksum_or_metadata() {
        for name in [
            "a.AppImage.zsync",
            "SHA256SUMS.txt",
            "a.deb.sha256",
            "latest-linux.yml",
            "a.sbom.json",
            "x.intoto.jsonl",
        ] {
            assert!(is_helper_file(name), "{name}");
        }
        assert!(!is_helper_file("a.AppImage"));
    }

    #[test]
    fn should_normalize_file_types_when_compound_or_aliased() {
        assert_eq!(
            (
                file_type("A.TAR.GZ"),
                file_type("a.tgz"),
                file_type("x.AppImage"),
                file_type("x.tar.xz")
            ),
            (
                Some("tar.gz".to_string()),
                Some("tar.gz".to_string()),
                Some("appimage".to_string()),
                Some("tar.xz".to_string())
            )
        );
    }

    #[test]
    fn should_detect_architecture_families() {
        let names = [
            "a-x86_64.zip",
            "a-amd64.deb",
            "a-aarch64.zip",
            "a-armv7l.deb",
            "a-i686.zip",
            "a-linux32.zip",
            "a.zip",
        ];
        let expected = [
            Some("x64"),
            Some("x64"),
            Some("arm64"),
            Some("arm"),
            Some("x86"),
            Some("x86"),
            None,
        ];
        for (name, family) in names.iter().zip(expected.iter()) {
            assert_eq!(architecture_family(name), *family);
        }
    }

    #[test]
    fn should_detect_foreign_systems_when_name_has_windows_or_mac_tokens() {
        let names = [
            "a-win64.zip",
            "a-macOS.dmg",
            "setup.exe",
            "a-linux64.tar.gz",
            "a.zip",
        ];
        let expected = [
            Some("windows"),
            Some("mac"),
            Some("windows"),
            Some("linux"),
            None,
        ];
        for (name, family) in names.iter().zip(expected.iter()) {
            assert_eq!(system_family(name), *family);
        }
    }

    #[test]
    fn should_say_no_candidates_when_release_has_no_assets() {
        let selection = select_asset("a-1.0.zip", VersionPair::new("1.0", "2.0"), &[]);
        assert_eq!(
            selection.reason,
            Some("uygun dosya bulunamadı (adaylar: yok)".to_string())
        );
    }

    fn select_brave(old_name: &str, old: &str) -> AssetSelection {
        select_asset(
            old_name,
            VersionPair::new(old, "1.96.61"),
            &load_assets("brave"),
        )
    }

    #[test]
    fn should_pick_stable_zip_when_old_file_is_stable_brave() {
        let selection = select_brave("brave-browser-1.93.129-linux-amd64.zip", "1.93.129");
        assert_eq!(
            selection.name,
            Some("brave-browser-1.96.61-linux-amd64.zip".to_string())
        );
    }

    #[test]
    fn should_refuse_when_old_file_is_nightly_and_release_has_other_products() {
        let selection = select_brave("brave-browser-nightly-1.95.29-linux-amd64.zip", "1.95.29");
        assert_eq!(selection.name, None);
        assert!(
            selection
                .reason
                .unwrap()
                .starts_with("uygun dosya bulunamadı (adaylar: ")
        );
    }

    #[test]
    fn should_compare_whole_name_when_old_file_has_no_version() {
        let assets: Vec<String> = [
            "zen-x86_64.AppImage",
            "zen-aarch64.AppImage",
            "other-x86_64.AppImage",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let selection = select_asset(
            "zen-x86_64.AppImage",
            VersionPair::new("1.0", "2.0"),
            &assets,
        );
        assert_eq!(selection.name, Some("zen-x86_64.AppImage".to_string()));
    }
}
