use std::path::Path;

use crate::build_state::{BuildEntry, EntryStatus, PREPARE_LOGIC_VERSION, State, package_dir};
use crate::report_model::{PackageReport, Status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparePackageStatus {
    Outdated,
    Error,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparePackage {
    pub name: String,
    pub status: PreparePackageStatus,
    pub recipe_path: String,
    pub latest_version: Option<String>,
    pub upstream: Option<String>,
    pub candidate_url: Option<String>,
    pub detail: Option<String>,
}

impl PreparePackage {
    pub fn outdated(name: &str, recipe_path: &str, latest_version: &str, upstream: &str) -> Self {
        Self {
            name: name.to_string(),
            status: PreparePackageStatus::Outdated,
            recipe_path: recipe_path.to_string(),
            latest_version: Some(latest_version.to_string()),
            upstream: Some(upstream.to_string()),
            candidate_url: None,
            detail: None,
        }
    }

    pub fn with_candidate_url(mut self, url: &str) -> Self {
        self.candidate_url = Some(url.to_string());
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_status(mut self, status: PreparePackageStatus) -> Self {
        self.status = status;
        self
    }
}

impl From<&PackageReport> for PreparePackage {
    fn from(report: &PackageReport) -> Self {
        let status = match report.status {
            Status::Outdated => PreparePackageStatus::Outdated,
            Status::Error => PreparePackageStatus::Error,
            Status::Current | Status::Unsupported | Status::Uncomparable => {
                PreparePackageStatus::Other
            }
        };
        Self {
            name: report.name.clone(),
            status,
            recipe_path: report.recipe_path.clone(),
            latest_version: report.latest_version.clone(),
            upstream: report.upstream.clone(),
            candidate_url: report.candidate_url.clone(),
            detail: report.detail.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind {
    New,
    PendingBuild,
    TransientRetry,
    LogicRetry,
}

const QUEUE_ORDER: [QueueKind; 4] = [
    QueueKind::New,
    QueueKind::PendingBuild,
    QueueKind::TransientRetry,
    QueueKind::LogicRetry,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub package: PreparePackage,
    pub kind: QueueKind,
}

pub fn has_prepared_file(output_dir: &Path, directory: &str) -> bool {
    output_dir.join(directory).join("pspec.xml").is_file()
}

pub fn is_pending_build(entry: &BuildEntry, output_dir: &Path, directory: &str) -> bool {
    entry.status == EntryStatus::Prepared && has_prepared_file(output_dir, directory)
}

pub fn is_exhausted_pending(entry: &BuildEntry, output_dir: &Path, directory: &str) -> bool {
    is_pending_build(entry, output_dir, directory)
        && entry.attempts >= crate::build_state::MAX_ATTEMPTS
}

fn classify_existing(
    package: &PreparePackage,
    entry: &BuildEntry,
    directory: &str,
    output_dir: &Path,
) -> Option<QueueKind> {
    if entry.version != package.latest_version.as_deref().unwrap_or("") {
        return Some(QueueKind::New);
    }
    if is_pending_build(entry, output_dir, directory) {
        return Some(QueueKind::PendingBuild);
    }
    match entry.status {
        EntryStatus::Prepared | EntryStatus::TransientFailed => Some(QueueKind::TransientRetry),
        EntryStatus::PrepareFailed if entry.logic_version < PREPARE_LOGIC_VERSION => {
            Some(QueueKind::LogicRetry)
        }
        _ => None,
    }
}

pub fn classify(package: &PreparePackage, state: &State, output_dir: &Path) -> Option<QueueKind> {
    let directory = package_dir(&package.recipe_path);
    match state.get(&directory) {
        None => Some(QueueKind::New),
        Some(entry) => classify_existing(package, entry, &directory, output_dir),
    }
}

pub fn build_queue(
    packages: &[&PreparePackage],
    state: &State,
    output_dir: &Path,
    limit: usize,
) -> Vec<QueueItem> {
    let items: Vec<QueueItem> = packages
        .iter()
        .filter_map(|package| {
            classify(package, state, output_dir).map(|kind| QueueItem {
                package: (*package).clone(),
                kind,
            })
        })
        .collect();
    let mut ordered = Vec::new();
    for kind in QUEUE_ORDER {
        ordered.extend(items.iter().filter(|item| item.kind == kind).cloned());
    }
    ordered.truncate(limit);
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pisi-bump-bot-prepare-queue-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&path);
        path
    }

    #[test]
    fn should_classify_as_new_when_no_state_entry() {
        let package = PreparePackage::outdated("zeta", "zz/zeta/pspec.xml", "v1", "o/zeta");
        let state = State::new();
        let output_dir = temp_dir("new");
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].kind, QueueKind::New);
    }

    #[test]
    fn should_classify_as_new_when_version_changed() {
        let package =
            PreparePackage::outdated("atari800", "game/atari800/pspec.xml", "v2", "o/atari800");
        let mut state = State::new();
        state.insert(
            "game/atari800".to_string(),
            BuildEntry::new("v1", EntryStatus::PrepareFailed, "d"),
        );
        let output_dir = temp_dir("version-changed");
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert_eq!(queue[0].kind, QueueKind::New);
    }

    #[test]
    fn should_classify_as_pending_build_when_prepared_file_exists() {
        let package = PreparePackage::outdated("brave", "net/brave/pspec.xml", "v1", "o/brave");
        let mut state = State::new();
        state.insert(
            "net/brave".to_string(),
            BuildEntry::new("v1", EntryStatus::Prepared, "d"),
        );
        let output_dir = temp_dir("pending");
        std::fs::create_dir_all(output_dir.join("net/brave")).unwrap();
        std::fs::write(output_dir.join("net/brave/pspec.xml"), "x").unwrap();
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert_eq!(queue[0].kind, QueueKind::PendingBuild);
    }

    #[test]
    fn should_classify_as_transient_retry_when_transient_failed() {
        let package = PreparePackage::outdated("brave", "net/brave/pspec.xml", "v1", "o/brave");
        let mut state = State::new();
        state.insert(
            "net/brave".to_string(),
            BuildEntry::new("v1", EntryStatus::TransientFailed, "d"),
        );
        let output_dir = temp_dir("transient");
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert_eq!(queue[0].kind, QueueKind::TransientRetry);
    }

    #[test]
    fn should_classify_as_logic_retry_when_permanent_failure_under_old_logic() {
        let package = PreparePackage::outdated("brave", "net/brave/pspec.xml", "v1", "o/brave");
        let mut state = State::new();
        state.insert(
            "net/brave".to_string(),
            BuildEntry {
                logic_version: 1,
                ..BuildEntry::new("v1", EntryStatus::PrepareFailed, "d")
            },
        );
        let output_dir = temp_dir("logic-retry");
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert_eq!(queue[0].kind, QueueKind::LogicRetry);
    }

    #[test]
    fn should_skip_when_permanent_failure_under_current_logic() {
        let package = PreparePackage::outdated("brave", "net/brave/pspec.xml", "v1", "o/brave");
        let mut state = State::new();
        state.insert(
            "net/brave".to_string(),
            BuildEntry::new("v1", EntryStatus::PrepareFailed, "d"),
        );
        let output_dir = temp_dir("skip");
        let queue = build_queue(&[&package], &state, &output_dir, 10);
        assert!(queue.is_empty());
    }

    #[test]
    fn should_order_by_priority_new_then_pending_then_transient_then_logic() {
        let zeta = PreparePackage::outdated("zeta", "zz/zeta/pspec.xml", "v1", "o/zeta");
        let brave = PreparePackage::outdated("brave", "net/brave/pspec.xml", "v1", "o/brave");
        let atari =
            PreparePackage::outdated("atari800", "game/atari800/pspec.xml", "v1", "o/atari800");
        let legacy = PreparePackage::outdated("jedit", "editor/jedit/pspec.xml", "v1", "o/jedit");
        let mut state = State::new();
        state.insert(
            "net/brave".to_string(),
            BuildEntry::new("v1", EntryStatus::Prepared, "d"),
        );
        let output_dir = temp_dir("priority");
        std::fs::create_dir_all(output_dir.join("net/brave")).unwrap();
        std::fs::write(output_dir.join("net/brave/pspec.xml"), "x").unwrap();
        state.insert(
            "game/atari800".to_string(),
            BuildEntry::new("v1", EntryStatus::TransientFailed, "d"),
        );
        state.insert(
            "editor/jedit".to_string(),
            BuildEntry {
                logic_version: 1,
                ..BuildEntry::new("v1", EntryStatus::PrepareFailed, "d")
            },
        );
        let queue = build_queue(&[&zeta, &brave, &atari, &legacy], &state, &output_dir, 10);
        let kinds: Vec<QueueKind> = queue.iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            vec![
                QueueKind::New,
                QueueKind::PendingBuild,
                QueueKind::TransientRetry,
                QueueKind::LogicRetry
            ]
        );
    }

    #[test]
    fn should_convert_outdated_package_report_into_prepare_package() {
        let report = PackageReport::new("brave-browser", Status::Outdated, "1.93.129")
            .with_latest_version("v1.96.61")
            .with_upstream("brave/brave-browser")
            .with_candidate_url(Some("https://example.org/brave-1.96.61.zip".to_string()))
            .with_recipe_path("network/browser/brave/pspec.xml");
        let package = PreparePackage::from(&report);
        assert_eq!(package.status, PreparePackageStatus::Outdated);
        assert_eq!(package.recipe_path, "network/browser/brave/pspec.xml");
        assert_eq!(package.latest_version.as_deref(), Some("v1.96.61"));
        assert_eq!(
            package.candidate_url.as_deref(),
            Some("https://example.org/brave-1.96.61.zip")
        );
    }

    #[test]
    fn should_convert_non_outdated_and_non_error_status_into_other() {
        let report = PackageReport::new("beta", Status::Current, "1.0");
        assert_eq!(
            PreparePackage::from(&report).status,
            PreparePackageStatus::Other
        );
        let report = PackageReport::new("gamma", Status::Unsupported, "1.0");
        assert_eq!(
            PreparePackage::from(&report).status,
            PreparePackageStatus::Other
        );
        let report = PackageReport::new("delta", Status::Uncomparable, "1.0");
        assert_eq!(
            PreparePackage::from(&report).status,
            PreparePackageStatus::Other
        );
    }

    #[test]
    fn should_convert_error_status_directly() {
        let report = PackageReport::new("epsilon", Status::Error, "1.0");
        assert_eq!(
            PreparePackage::from(&report).status,
            PreparePackageStatus::Error
        );
    }

    #[test]
    fn should_respect_limit_when_truncating() {
        let a = PreparePackage::outdated("a", "a/pspec.xml", "v1", "o/a");
        let b = PreparePackage::outdated("b", "b/pspec.xml", "v1", "o/b");
        let state = State::new();
        let output_dir = temp_dir("limit");
        let queue = build_queue(&[&a, &b], &state, &output_dir, 1);
        assert_eq!(queue.len(), 1);
    }
}
