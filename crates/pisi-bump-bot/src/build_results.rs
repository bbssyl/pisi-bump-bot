use std::fs;
use std::path::Path;

use crate::build_columns::mark_for;
use crate::build_state::{BuildEntry, EntryStatus, State};
use crate::failure_policy::transient_outcome;

const NO_STATUS_REASON: &str =
    "derleme durumu üretilmedi (iptal, zaman aşımı, imaj çekme veya klonlama hatası)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildResult {
    pub recipe_path: String,
    pub version: String,
    pub status: EntryStatus,
    pub run_id: Option<String>,
    pub artifact_name: Option<String>,
}

fn status_from_text(text: &str) -> Option<EntryStatus> {
    match text {
        "basarili" => Some(EntryStatus::Built),
        "basarisiz" => Some(EntryStatus::BuildFailed),
        "durum_yok" => Some(EntryStatus::TransientFailed),
        _ => None,
    }
}

fn result_from_document(document: &serde_json::Value) -> Option<BuildResult> {
    let status_text = document.get("status")?.as_str()?;
    let status = status_from_text(status_text)?;
    let recipe_path = document
        .get("recipe_path")?
        .as_str()
        .filter(|value| !value.is_empty())?;
    let version = document
        .get("version")?
        .as_str()
        .filter(|value| !value.is_empty())?;
    let run_id = document.get("run_id").and_then(run_id_text);
    let artifact_name = document
        .get("artifact_name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Some(BuildResult {
        recipe_path: recipe_path.to_string(),
        version: version.to_string(),
        status,
        run_id,
        artifact_name,
    })
}

fn run_id_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
        serde_json::Value::Number(number) if number.as_f64() != Some(0.0) => {
            Some(number.to_string())
        }
        _ => None,
    }
}

fn read_result(path: &Path) -> Option<BuildResult> {
    let text = fs::read_to_string(path).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    if !document.is_object() {
        return None;
    }
    result_from_document(&document)
}

fn is_result_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("result-") && name.ends_with(".json"))
}

fn collect_result_paths(directory: &Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_result_paths(&path, found);
        } else if is_result_file(&path) {
            found.push(path);
        }
    }
}

pub fn load_results(directory: &Path) -> Vec<BuildResult> {
    let mut entries = Vec::new();
    collect_result_paths(directory, &mut entries);
    entries.sort();
    let mut results: Vec<BuildResult> = entries
        .into_iter()
        .filter_map(|path| read_result(&path))
        .collect();
    results.sort_by(|left, right| left.recipe_path.cmp(&right.recipe_path));
    results
}

fn apply_result(entry: &BuildEntry, result: &BuildResult, run_date: &str) -> BuildEntry {
    if result.status == EntryStatus::TransientFailed {
        let (status, reason) = transient_outcome(NO_STATUS_REASON, entry.attempts);
        return BuildEntry {
            status,
            date: run_date.to_string(),
            reason: Some(reason),
            run_id: result.run_id.clone(),
            artifact_name: None,
            ..entry.clone()
        };
    }
    BuildEntry {
        status: result.status,
        date: run_date.to_string(),
        reason: None,
        run_id: result.run_id.clone(),
        artifact_name: result.artifact_name.clone(),
        ..entry.clone()
    }
}

pub fn merge_results(
    state: &State,
    results: &[BuildResult],
    run_date: &str,
) -> (State, Vec<BuildResult>) {
    let mut merged = state.clone();
    let mut applied = Vec::new();
    for result in results {
        let matches = merged.get(&result.recipe_path).is_some_and(|entry| {
            entry.version == result.version && entry.status == EntryStatus::Prepared
        });
        if matches {
            let entry = &merged[&result.recipe_path];
            let updated = apply_result(entry, result, run_date);
            merged.insert(result.recipe_path.clone(), updated);
            applied.push(result.clone());
        }
    }
    (merged, applied)
}

pub fn render_results_comment(applied: &[BuildResult], web_url: Option<&str>) -> String {
    if applied.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "## Derleme sonuçları (deneme derlemesi)".to_string(),
        String::new(),
    ];
    for result in applied {
        let run = match (web_url, &result.run_id) {
            (Some(url), Some(run_id)) => Some(format!("{url}/actions/runs/{run_id}")),
            _ => None,
        };
        let suffix = match run {
            Some(run) => format!(" ([çalışma]({run}))"),
            None => String::new(),
        };
        let mark = mark_for(result.status).unwrap_or_default();
        lines.push(format!(
            "- {mark} `{}` {}{suffix}",
            result.recipe_path, result.version
        ));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(status: EntryStatus) -> BuildEntry {
        BuildEntry::new("v2", status, "2026-10-07")
    }

    #[test]
    fn should_apply_result_when_version_matches_prepared_entry() {
        let mut state = State::new();
        state.insert("alpha".to_string(), entry(EntryStatus::Prepared));
        let result = BuildResult {
            recipe_path: "alpha".to_string(),
            version: "v2".to_string(),
            status: EntryStatus::Built,
            run_id: Some("9".to_string()),
            artifact_name: Some("derleme-alpha".to_string()),
        };
        let (state, applied) = merge_results(&state, &[result], "2026-10-08");
        assert_eq!(state["alpha"].status, EntryStatus::Built);
        assert_eq!(state["alpha"].run_id.as_deref(), Some("9"));
        assert_eq!(applied.len(), 1);
    }

    #[test]
    fn should_ignore_result_when_version_is_stale() {
        let mut state = State::new();
        state.insert("alpha".to_string(), entry(EntryStatus::Prepared));
        let result = BuildResult {
            recipe_path: "alpha".to_string(),
            version: "v1".to_string(),
            status: EntryStatus::Built,
            run_id: Some("9".to_string()),
            artifact_name: None,
        };
        let (state, applied) = merge_results(&state, &[result], "d");
        assert_eq!(state["alpha"].status, EntryStatus::Prepared);
        assert!(applied.is_empty());
    }

    #[test]
    fn should_load_results_and_skip_broken_files_when_directory_has_junk() {
        let mut base = std::env::temp_dir();
        base.push(format!(
            "pisi-bump-bot-build-results-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&base).unwrap();
        fs::write(
            base.join("result-a.json"),
            r#"{"recipe_path":"alpha","version":"v2","status":"basarili","run_id":5,"artifact_name":"x"}"#,
        )
        .unwrap();
        fs::write(base.join("result-b.json"), "{bozuk").unwrap();
        fs::write(
            base.join("result-c.json"),
            r#"{"recipe_path":"c","version":"v","status":"?"}"#,
        )
        .unwrap();
        let results = load_results(&base);
        assert_eq!(
            results
                .iter()
                .map(|r| r.recipe_path.clone())
                .collect::<Vec<_>>(),
            vec!["alpha".to_string()]
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn should_render_comment_with_run_link_when_results_applied() {
        let result = BuildResult {
            recipe_path: "alpha".to_string(),
            version: "v2".to_string(),
            status: EntryStatus::Built,
            run_id: Some("9".to_string()),
            artifact_name: Some("derleme-alpha".to_string()),
        };
        let comment = render_results_comment(&[result], Some("https://github.com/o/r"));
        assert!(comment.contains("(https://github.com/o/r/actions/runs/9)"));
    }

    #[test]
    fn should_render_empty_comment_when_nothing_applied() {
        assert_eq!(
            render_results_comment(&[], Some("https://github.com/o/r")),
            ""
        );
    }
}
