use crate::build_state::{BuildEntry, EntryStatus, MAX_ATTEMPTS, State, package_dir};

pub const PENDING_TEXT: &str = "bekliyor";
pub const NOT_PREPARED_PREFIX: &str = "otomatik hazırlanamadı";
const FILE_BRANCH: &str = "HEAD";
pub const TRANSIENT_PREFIX: &str = "geçici hata, tekrar denenecek";
const NO_DIFF_STATUSES: [EntryStatus; 2] =
    [EntryStatus::PrepareFailed, EntryStatus::TransientFailed];

pub fn mark_for(status: EntryStatus) -> Option<&'static str> {
    match status {
        EntryStatus::Built => Some("✅"),
        EntryStatus::BuildFailed => Some("❌"),
        EntryStatus::TransientFailed => Some("⏳"),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct BuildLinks {
    pub state: State,
    pub web_url: Option<String>,
    pub relative_files: bool,
}

impl BuildLinks {
    pub fn new(state: State, web_url: Option<String>, relative_files: bool) -> Self {
        Self {
            state,
            web_url,
            relative_files,
        }
    }

    fn entry_for(&self, recipe_path: &str, version: Option<&str>) -> Option<&BuildEntry> {
        let entry = self.state.get(&package_dir(recipe_path))?;
        if Some(entry.version.as_str()) == version {
            Some(entry)
        } else {
            None
        }
    }

    fn diff_url(&self, recipe_path: &str) -> String {
        let path = format!("hazir/{}/pspec.diff", package_dir(recipe_path));
        match &self.web_url {
            Some(url) if !self.relative_files => format!("{url}/blob/{FILE_BRANCH}/{path}"),
            _ => path,
        }
    }

    fn run_url(&self, entry: &BuildEntry) -> Option<String> {
        let url = self.web_url.as_ref()?;
        let run_id = entry.run_id.as_ref()?;
        Some(format!("{url}/actions/runs/{run_id}"))
    }

    pub fn prepared_cell(&self, recipe_path: &str, version: Option<&str>) -> Option<String> {
        let entry = self.entry_for(recipe_path, version)?;
        if NO_DIFF_STATUSES.contains(&entry.status) {
            return None;
        }
        Some(format!("[pspec.diff]({})", self.diff_url(recipe_path)))
    }

    pub fn build_cell(&self, recipe_path: &str, version: Option<&str>) -> Option<String> {
        let entry = self.entry_for(recipe_path, version)?;
        match entry.status {
            EntryStatus::PrepareFailed => Some(format!(
                "{NOT_PREPARED_PREFIX}: {}",
                entry.reason.as_deref().unwrap_or_default()
            )),
            EntryStatus::TransientFailed => Some(format!(
                "{TRANSIENT_PREFIX} ({}/{MAX_ATTEMPTS}): {}",
                entry.attempts,
                entry.reason.as_deref().unwrap_or_default()
            )),
            EntryStatus::Prepared => Some(PENDING_TEXT.to_string()),
            _ => Some(result_cell(entry, self.run_url(entry))),
        }
    }
}

fn result_cell(entry: &BuildEntry, url: Option<String>) -> String {
    let mark = mark_for(entry.status).unwrap_or_default();
    match url {
        Some(url) => format!("[{mark}]({url})"),
        None => mark.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_pending_text_when_entry_is_prepared() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v2", EntryStatus::Prepared, "d"),
        );
        let links = BuildLinks::new(state, None, false);
        assert_eq!(
            links.build_cell("alpha/pspec.xml", Some("v2")),
            Some(PENDING_TEXT.to_string())
        );
    }

    #[test]
    fn should_return_none_when_version_does_not_match() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v1", EntryStatus::Built, "d"),
        );
        let links = BuildLinks::new(state, None, false);
        assert_eq!(links.build_cell("alpha/pspec.xml", Some("v2")), None);
        assert_eq!(links.prepared_cell("alpha/pspec.xml", Some("v2")), None);
    }

    #[test]
    fn should_omit_diff_link_when_prepare_failed() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v2", EntryStatus::PrepareFailed, "d").with_reason("boom"),
        );
        let links = BuildLinks::new(state, None, false);
        assert_eq!(links.prepared_cell("alpha/pspec.xml", Some("v2")), None);
        assert_eq!(
            links.build_cell("alpha/pspec.xml", Some("v2")),
            Some(format!("{NOT_PREPARED_PREFIX}: boom"))
        );
    }

    #[test]
    fn should_report_transient_attempt_count() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v2", EntryStatus::TransientFailed, "d")
                .with_reason("zaman aşımı")
                .with_attempts(2),
        );
        let links = BuildLinks::new(state, None, false);
        assert_eq!(
            links.build_cell("alpha/pspec.xml", Some("v2")),
            Some(format!(
                "{TRANSIENT_PREFIX} (2/{MAX_ATTEMPTS}): zaman aşımı"
            ))
        );
    }

    #[test]
    fn should_render_relative_diff_link_when_requested() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v2", EntryStatus::Prepared, "d"),
        );
        let links = BuildLinks::new(state, Some("https://github.com/o/r".to_string()), true);
        assert_eq!(
            links.prepared_cell("alpha/pspec.xml", Some("v2")),
            Some("[pspec.diff](hazir/alpha/pspec.diff)".to_string())
        );
    }
}
