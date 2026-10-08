use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::StateError;

pub const PREPARE_LOGIC_VERSION: u32 = 2;
pub const LEGACY_LOGIC_VERSION: u32 = 1;
pub const MAX_ATTEMPTS: u32 = 3;

fn legacy_logic_version() -> u32 {
    LEGACY_LOGIC_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatus {
    #[serde(rename = "hazir")]
    Prepared,
    #[serde(rename = "hazirlanamadi")]
    PrepareFailed,
    #[serde(rename = "gecici_hata")]
    TransientFailed,
    #[serde(rename = "basarili")]
    Built,
    #[serde(rename = "basarisiz")]
    BuildFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildEntry {
    pub version: String,
    pub status: EntryStatus,
    pub date: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub artifact_name: Option<String>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default = "legacy_logic_version")]
    pub logic_version: u32,
}

impl BuildEntry {
    pub fn new(version: impl Into<String>, status: EntryStatus, date: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            status,
            date: date.into(),
            reason: None,
            run_id: None,
            artifact_name: None,
            attempts: 0,
            logic_version: PREPARE_LOGIC_VERSION,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts;
        self
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }
}

pub type State = BTreeMap<String, BuildEntry>;

pub fn package_dir(recipe_path: &str) -> String {
    recipe_path
        .strip_suffix("/pspec.xml")
        .unwrap_or(recipe_path)
        .to_string()
}

#[derive(Serialize, Deserialize)]
struct StateDocument {
    packages: BTreeMap<String, BuildEntry>,
}

pub fn load_state(path: &Path) -> Result<State, StateError> {
    if !path.is_file() {
        return Ok(State::new());
    }
    let text = fs::read_to_string(path)?;
    let document: StateDocument = serde_json::from_str(&text)?;
    Ok(document.packages)
}

pub fn render_state(state: &State) -> Result<String, StateError> {
    let document = StateDocument {
        packages: state.clone(),
    };
    let mut rendered = serde_json::to_string_pretty(&document)?;
    rendered.push('\n');
    Ok(rendered)
}

pub fn save_state(path: &Path, state: &State) -> Result<(), StateError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render_state(state)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn live_state_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures/state/live-builds.json")
    }

    #[test]
    fn should_load_live_state_with_legacy_logic_version_and_zero_attempts() {
        let state = load_state(&live_state_path()).unwrap();
        let entry = &state["multimedia/graphics/freecad"];
        assert_eq!(state.len(), 10);
        assert_eq!(entry.status, EntryStatus::PrepareFailed);
        assert_eq!(entry.logic_version, 1);
        assert_eq!(entry.attempts, 0);
    }

    #[test]
    fn should_round_trip_live_state_with_new_fields() {
        let state = load_state(&live_state_path()).unwrap();
        let rendered = render_state(&state).unwrap();
        let document: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let entry = &document["packages"]["multimedia/graphics/freecad"];
        assert_eq!(entry["logic_version"], 1);
        assert_eq!(entry["attempts"], 0);
    }

    #[test]
    fn should_default_missing_state_file_to_empty() {
        let state = load_state(Path::new("/nonexistent/path/builds.json")).unwrap();
        assert!(state.is_empty());
    }

    #[test]
    fn should_round_trip_fresh_entry_through_save_and_load() {
        let temp = tempfile_path();
        let mut state = State::new();
        state.insert(
            "editor/jedit".to_string(),
            BuildEntry::new("5.6.0", EntryStatus::Prepared, "2026-10-08"),
        );
        save_state(&temp, &state).unwrap();
        let reloaded = load_state(&temp).unwrap();
        assert_eq!(
            reloaded["editor/jedit"].logic_version,
            PREPARE_LOGIC_VERSION
        );
        assert_eq!(reloaded["editor/jedit"].attempts, 0);
        let _ = fs::remove_file(&temp);
    }

    fn tempfile_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pisi-bump-bot-build-state-test-{}.json",
            std::process::id()
        ));
        path
    }

    #[test]
    fn should_serialize_null_optional_fields_like_python_asdict() {
        let mut state = State::new();
        state.insert(
            "a/b".to_string(),
            BuildEntry::new("v1", EntryStatus::PrepareFailed, "2026-10-08"),
        );
        let rendered = render_state(&state).unwrap();
        assert!(rendered.contains("\"reason\": null"));
        assert!(rendered.contains("\"run_id\": null"));
        assert!(rendered.contains("\"artifact_name\": null"));
    }
}
