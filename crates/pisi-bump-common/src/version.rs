use std::sync::LazyLock;

use regex::Regex;

const LEADING_WORDS: [&str; 2] = ["release-", "release_"];

static VERSION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9]+(?:[._-][0-9]+)*").unwrap());
static SEGMENT_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._-]").unwrap());
static PRERELEASE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[-_.]?(alpha|beta|rc|pre|dev|snapshot|nightly)").unwrap());

fn strip_known_prefixes(raw: &str, prefixes: &[&str]) -> String {
    let mut text = raw.trim().to_string();
    let mut ordered_prefixes: Vec<&&str> = prefixes.iter().collect();
    ordered_prefixes.sort_by_key(|prefix| std::cmp::Reverse(prefix.len()));
    for prefix in ordered_prefixes {
        if prefix.is_empty() {
            continue;
        }
        for separator in ["-", "_"] {
            let candidate = format!("{}{separator}", prefix.to_lowercase());
            if text.to_lowercase().starts_with(&candidate) {
                text = text[candidate.len()..].to_string();
            }
        }
    }
    for word in LEADING_WORDS {
        if text.to_lowercase().starts_with(word) {
            text = text[word.len()..].to_string();
        }
    }
    text
}

pub fn extract_version_text(raw: &str, prefixes: &[&str]) -> Option<String> {
    let text = strip_known_prefixes(raw, prefixes);
    let found = VERSION_PATTERN.find(&text)?;
    if PRERELEASE_PATTERN.is_match(&text[found.end()..]) {
        return None;
    }
    Some(found.as_str().to_string())
}

pub fn normalize_version(raw: &str, prefixes: &[&str]) -> Option<Vec<u64>> {
    let version_text = extract_version_text(raw, prefixes)?;
    SEGMENT_SPLIT
        .split(&version_text)
        .map(|segment| segment.parse::<u64>().ok())
        .collect()
}

pub fn compare_versions(left: &[u64], right: &[u64]) -> i32 {
    let length = left.len().max(right.len());
    let padded = |values: &[u64]| -> Vec<u64> {
        let mut padded = values.to_vec();
        padded.resize(length, 0);
        padded
    };
    let padded_left = padded(left);
    let padded_right = padded(right);
    match padded_left.cmp(&padded_right) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_versions, extract_version_text, normalize_version};

    #[test]
    fn should_strip_leading_v_when_tag_has_v_prefix() {
        assert_eq!(normalize_version("v1.2.3", &[]), Some(vec![1, 2, 3]));
    }

    #[test]
    fn should_strip_release_prefix_when_tag_starts_with_release() {
        assert_eq!(
            normalize_version("release-20210321", &[]),
            Some(vec![20210321])
        );
    }

    #[test]
    fn should_strip_repository_prefix_when_given() {
        assert_eq!(normalize_version("lzfse-1.0", &["lzfse"]), Some(vec![1, 0]));
    }

    #[test]
    fn should_parse_underscore_separated_tag_when_tag_uses_underscores() {
        assert_eq!(normalize_version("V_3_14_1", &[]), Some(vec![3, 14, 1]));
    }

    #[test]
    fn should_ignore_trailing_suffix_when_tag_has_platform_suffix() {
        assert_eq!(
            normalize_version("release-2.9.12-linux4", &[]),
            Some(vec![2, 9, 12])
        );
    }

    #[test]
    fn should_return_none_when_tag_has_no_digits() {
        assert_eq!(normalize_version("latest", &[]), None);
    }

    #[test]
    fn should_return_none_when_tag_is_prerelease() {
        assert_eq!(normalize_version("1.0.0-rc1", &[]), None);
    }

    #[test]
    fn should_return_none_when_text_is_empty() {
        assert_eq!(normalize_version("", &[]), None);
    }

    #[test]
    fn should_treat_missing_segments_as_zero_when_lengths_differ() {
        assert_eq!(compare_versions(&[1, 0], &[1, 0, 0]), 0);
    }

    #[test]
    fn should_compare_numerically_when_segments_have_different_widths() {
        assert_eq!(compare_versions(&[1, 10], &[1, 9]), 1);
    }

    #[test]
    fn should_return_negative_when_left_is_older() {
        assert_eq!(compare_versions(&[0, 9, 6], &[0, 9, 6, 1]), -1);
    }

    #[test]
    fn should_return_matched_text_when_tag_contains_version() {
        assert_eq!(
            extract_version_text("v0.9.6.12", &[]),
            Some("0.9.6.12".to_string())
        );
    }
}
