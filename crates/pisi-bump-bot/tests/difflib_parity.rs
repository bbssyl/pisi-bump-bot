use std::fs;
use std::path::PathBuf;

use pisi_bump_bot::difflib::{SequenceMatcher, unified_diff};
use serde::Deserialize;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/difflib")
}

#[test]
fn should_match_python_difflib_ratio_on_every_recorded_pair() {
    let path = fixtures_dir().join("difflib_parity.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    let pairs: Vec<(String, String, f64)> = serde_json::from_str(&text).unwrap();

    assert_eq!(pairs.len(), 5359);

    let mut mismatches = Vec::new();
    for (a, b, expected_ratio) in &pairs {
        let left: Vec<char> = a.chars().collect();
        let right: Vec<char> = b.chars().collect();
        let actual_ratio = SequenceMatcher::new(&left, &right).ratio();
        if actual_ratio != *expected_ratio {
            mismatches.push((a.clone(), b.clone(), *expected_ratio, actual_ratio));
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} ratio mismatches: {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(5)]
    );
}

#[derive(Deserialize)]
struct DiffFixture {
    a: Vec<String>,
    b: Vec<String>,
    diff: Vec<String>,
}

#[test]
fn should_match_python_unified_diff_on_280_line_autojunk_case() {
    let path = fixtures_dir().join("difflib_diff_parity.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    let fixture: DiffFixture = serde_json::from_str(&text).unwrap();

    assert_eq!(fixture.a.len(), 280);

    let a: Vec<&str> = fixture.a.iter().map(String::as_str).collect();
    let b: Vec<&str> = fixture.b.iter().map(String::as_str).collect();
    let actual = unified_diff(&a, &b, "a/file.txt", "b/file.txt", 3);

    assert_eq!(actual, fixture.diff);
}
