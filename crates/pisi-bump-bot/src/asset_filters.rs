use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

const HELPER_SUFFIXES: &[&str] = &[
    ".zsync",
    ".sha256",
    ".sha512",
    ".sha1",
    ".md5",
    ".sig",
    ".asc",
    ".txt",
    ".blockmap",
    ".yml",
    ".yaml",
    ".json",
    ".minisig",
    ".sha256sum",
    ".sha512sum",
    ".pem",
    ".sigstore",
    ".bundle",
];

const FILE_TYPES: &[&str] = &[
    "tar.gz", "tar.xz", "tar.bz2", "tar.zst", "tar", "tgz", "txz", "tbz2", "tbz", "appimage",
    "deb", "zip", "rpm", "7z", "snap", "flatpak", "pacman", "exe", "dmg", "msi",
];

const FOREIGN_SYSTEM_SUFFIXES: &[&str] = &[".exe", ".dmg", ".msi", ".pkg", ".apk"];
const FOREIGN_SYSTEMS: &[&str] = &["windows", "mac"];

fn file_type_alias(extension: &str) -> &str {
    match extension {
        "tgz" => "tar.gz",
        "txz" => "tar.xz",
        "tbz2" | "tbz" => "tar.bz2",
        other => other,
    }
}

static HELPER_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:sbom|intoto)").unwrap());

fn token(alternatives: &[&str]) -> FancyRegex {
    let pattern = format!("(?<![a-z0-9])(?:{})(?![a-z0-9])", alternatives.join("|"));
    FancyRegex::new(&pattern).expect("asset_filters token pattern must compile")
}

static ARM64_PATTERN: LazyLock<FancyRegex> = LazyLock::new(|| token(&["arm64", "aarch64"]));
static ARM_PATTERN: LazyLock<FancyRegex> =
    LazyLock::new(|| token(&["arm", r"armv\d+\w*", "armhf", "armel"]));
static X86_PATTERN: LazyLock<FancyRegex> = LazyLock::new(|| {
    token(&[
        "i[3-6]86",
        "linux32",
        "32-?bit",
        "ia32",
        "x32",
        r"x86(?![_-]?64)",
    ])
});
static X64_PATTERN: LazyLock<FancyRegex> =
    LazyLock::new(|| token(&["x86[_-]64", "amd64", "x64", "linux64", "64-?bit"]));

static WINDOWS_PATTERN: LazyLock<FancyRegex> =
    LazyLock::new(|| token(&["win", "windows", "win32", "win64"]));
static MAC_PATTERN: LazyLock<FancyRegex> =
    LazyLock::new(|| token(&["mac", "macos", "macosx", "osx", "darwin"]));
static LINUX_PATTERN: LazyLock<FancyRegex> =
    LazyLock::new(|| token(&["linux", "linux32", "linux64"]));

fn architecture_patterns() -> [(&'static str, &'static FancyRegex); 4] {
    [
        ("arm64", &ARM64_PATTERN),
        ("arm", &ARM_PATTERN),
        ("x86", &X86_PATTERN),
        ("x64", &X64_PATTERN),
    ]
}

fn system_patterns() -> [(&'static str, &'static FancyRegex); 3] {
    [
        ("windows", &WINDOWS_PATTERN),
        ("mac", &MAC_PATTERN),
        ("linux", &LINUX_PATTERN),
    ]
}

fn matches(pattern: &FancyRegex, text: &str) -> bool {
    pattern
        .is_match(text)
        .expect("asset_filters pattern must not fail to match")
}

pub fn is_helper_file(name: &str) -> bool {
    let lowered = name.to_lowercase();
    HELPER_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
        || HELPER_MARKER.is_match(&lowered)
}

pub fn file_type(name: &str) -> Option<String> {
    let lowered = name.to_lowercase();
    for extension in FILE_TYPES {
        if lowered.ends_with(&format!(".{extension}")) {
            return Some(file_type_alias(extension).to_string());
        }
    }
    if !lowered.contains('.') {
        return None;
    }
    let suffix = lowered.rsplit('.').next().unwrap_or("");
    if !suffix.is_empty()
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        Some(suffix.to_string())
    } else {
        None
    }
}

pub fn architecture_family(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    architecture_patterns()
        .into_iter()
        .find(|(_, pattern)| matches(pattern, &lowered))
        .map(|(family, _)| family)
}

pub fn system_family(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    if FOREIGN_SYSTEM_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
    {
        return Some(if lowered.ends_with(".exe") || lowered.ends_with(".msi") {
            "windows"
        } else {
            "mac"
        });
    }
    system_patterns()
        .into_iter()
        .find(|(_, pattern)| matches(pattern, &lowered))
        .map(|(family, _)| family)
}

pub fn matches_architecture(old_name: &str, candidate: &str) -> bool {
    let expected = architecture_family(old_name).unwrap_or("x64");
    match architecture_family(candidate) {
        None => true,
        Some(found) => found == expected,
    }
}

pub fn matches_system(old_name: &str, candidate: &str) -> bool {
    match system_family(candidate) {
        None => true,
        Some(found) => !FOREIGN_SYSTEMS.contains(&found) || Some(found) == system_family(old_name),
    }
}

pub fn matches_file_type(old_name: &str, candidate: &str) -> bool {
    match file_type(old_name) {
        None => true,
        Some(expected) => file_type(candidate) == Some(expected),
    }
}

#[cfg(test)]
mod tests {
    use super::{architecture_family, file_type, is_helper_file, system_family};

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
        assert_eq!(file_type("A.TAR.GZ"), Some("tar.gz".to_string()));
        assert_eq!(file_type("a.tgz"), Some("tar.gz".to_string()));
        assert_eq!(file_type("x.AppImage"), Some("appimage".to_string()));
        assert_eq!(file_type("x.tar.xz"), Some("tar.xz".to_string()));
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
            assert_eq!(architecture_family(name), *family, "{name}");
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
            assert_eq!(system_family(name), *family, "{name}");
        }
    }
}
