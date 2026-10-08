use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use pisi_bump_common::{PackageRecipe, read_recipe};

use crate::archive_download::DownloadResult;
use crate::build_state::{BuildEntry, EntryStatus, State, package_dir};
use crate::candidate_url::{build_candidate_url, derive_new_version};
use crate::error::{ArchiveDownloadError, PrepareFailure};
use crate::failure_policy::{BUILD_LOST_REASON, failure_outcome, next_attempts, transient_outcome};
use crate::github_upstream::parse_github_archive;
use crate::prepare_queue::{
    PreparePackage, PreparePackageStatus, QueueItem, QueueKind, build_queue, is_exhausted_pending,
};
use crate::pspec_updater::{PreparedUpdate, UpdateRequest, prepare_update};
use crate::report_model::Report;

pub const DEFAULT_LIMIT: usize = 10;

static SAFE_DIRECTORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$").unwrap());

pub type Downloader<'a> = dyn Fn(&str) -> Result<DownloadResult, ArchiveDownloadError> + 'a;

#[derive(Debug, Clone)]
pub struct PrepareSettings {
    pub recipes_dir: PathBuf,
    pub output_dir: PathBuf,
    pub run_date: String,
    pub limit: usize,
}

impl PrepareSettings {
    pub fn new(
        recipes_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        run_date: impl Into<String>,
    ) -> Self {
        Self {
            recipes_dir: recipes_dir.into(),
            output_dir: output_dir.into(),
            run_date: run_date.into(),
            limit: DEFAULT_LIMIT,
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct PrepareReport {
    pub packages: Vec<PreparePackage>,
}

impl PrepareReport {
    pub fn from_report(report: &Report) -> Self {
        Self {
            packages: report.packages.iter().map(PreparePackage::from).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareOutcome {
    pub state: State,
    pub build_list: Vec<String>,
    pub queue: Vec<QueueItem>,
}

fn new_version_for(
    package: &PreparePackage,
    current_version: &str,
) -> Result<String, PrepareFailure> {
    let upstream_repo = package
        .upstream
        .as_deref()
        .map(|upstream| upstream.rsplit('/').next().unwrap_or(""))
        .unwrap_or("");
    let prefixes = [upstream_repo, package.name.as_str()];
    derive_new_version(
        package.latest_version.as_deref().unwrap_or(""),
        &prefixes,
        current_version,
    )
    .ok_or_else(|| PrepareFailure::permanent("yeni sürüm etiketten okunamadı"))
}

fn read_text_exact(path: &Path) -> Result<String, PrepareFailure> {
    fs::read_to_string(path).map_err(|_| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dosya");
        PrepareFailure::permanent(format!("{name} okunamadı"))
    })
}

fn load_recipe(
    settings: &PrepareSettings,
    package: &PreparePackage,
) -> Result<PackageRecipe, PrepareFailure> {
    match read_recipe(&settings.recipes_dir, &package.recipe_path) {
        Some(recipe) if !recipe.archive_urls.is_empty() => Ok(recipe),
        _ => Err(PrepareFailure::permanent(
            "pspec.xml okunamadı veya arşiv URL'si yok",
        )),
    }
}

fn resolve_candidate(
    package: &PreparePackage,
    recipe: &PackageRecipe,
    new_version: &str,
) -> Result<String, PrepareFailure> {
    if let Some(url) = package
        .candidate_url
        .as_deref()
        .filter(|url| !url.is_empty())
    {
        return Ok(url.to_string());
    }
    if let Some(detail) = package
        .detail
        .as_deref()
        .filter(|detail| !detail.is_empty())
    {
        return Err(PrepareFailure::permanent(detail.to_string()));
    }
    let archive_url = &recipe.archive_urls[0];
    let archive = parse_github_archive(archive_url);
    let (archive, latest_version) = match (archive, package.latest_version.as_deref()) {
        (Some(archive), Some(version)) => (archive, version),
        _ => {
            return Err(PrepareFailure::permanent(
                "GitHub arşiv URL'si çözümlenemedi",
            ));
        }
    };
    let candidate = build_candidate_url(
        archive_url,
        &archive.tag,
        latest_version,
        &recipe.current_version,
        new_version,
    );
    if candidate == *archive_url {
        return Err(PrepareFailure::permanent("aday arşiv URL'si üretilemedi"));
    }
    Ok(candidate)
}

fn download_archive(download: &Downloader, url: &str) -> Result<DownloadResult, PrepareFailure> {
    download(url).map_err(PrepareFailure::from)
}

fn prepare_package(
    package: &PreparePackage,
    settings: &PrepareSettings,
    download: &Downloader,
) -> Result<PreparedUpdate, PrepareFailure> {
    let directory = package_dir(&package.recipe_path);
    let recipe = load_recipe(settings, package)?;
    let new_version = new_version_for(package, &recipe.current_version)?;
    let candidate = resolve_candidate(package, &recipe, &new_version)?;
    let result = download_archive(download, &candidate)?;
    let source = settings.recipes_dir.join(&directory);
    let actions_path = source.join("actions.py");
    let actions_text = if actions_path.is_file() {
        Some(read_text_exact(&actions_path)?)
    } else {
        None
    };
    let pspec_text = read_text_exact(&source.join("pspec.xml"))?;
    let request = UpdateRequest {
        recipe_path: &directory,
        pspec_text: &pspec_text,
        actions_text: actions_text.as_deref(),
        new_version: &new_version,
        new_archive_url: &candidate,
        new_sha1: &result.sha1,
        run_date: &settings.run_date,
        old_archive_url: Some(&recipe.archive_urls[0]),
    };
    prepare_update(&request).map_err(PrepareFailure::from)
}

fn write_prepared(output_dir: &Path, directory: &str, pspec_text: &str, diff_text: &str) {
    let target = output_dir.join(directory);
    fs::create_dir_all(&target).expect("hazir dizini oluşturulamadı");
    fs::write(target.join("pspec.xml"), pspec_text).expect("pspec.xml yazılamadı");
    fs::write(target.join("pspec.diff"), diff_text).expect("pspec.diff yazılamadı");
}

fn remove_prepared(output_dir: &Path, directory: &str) {
    let target = output_dir.join(directory);
    let _ = fs::remove_dir_all(&target);
    let mut parent = target.parent().map(Path::to_path_buf);
    while let Some(candidate) = parent {
        if candidate == output_dir || !candidate.is_dir() {
            break;
        }
        let is_empty = fs::read_dir(&candidate).is_ok_and(|mut entries| entries.next().is_none());
        if !is_empty {
            break;
        }
        let _ = fs::remove_dir(&candidate);
        parent = candidate.parent().map(Path::to_path_buf);
    }
}

fn relative_posix_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn walk_pspec_directories(root: &Path, directory: &Path, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut subdirectories = Vec::new();
    let mut has_pspec = false;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            subdirectories.push(entry.path());
        } else if entry.file_name().to_str() == Some("pspec.xml") {
            has_pspec = true;
        }
    }
    if has_pspec {
        found.push(relative_posix_path(root, directory));
    }
    for subdirectory in subdirectories {
        walk_pspec_directories(root, &subdirectory, found);
    }
}

fn existing_directories(output_dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk_pspec_directories(output_dir, output_dir, &mut found);
    found.sort();
    found
}

fn prune(report: &PrepareReport, state: State, output_dir: &Path) -> State {
    let keep: HashSet<String> = report
        .packages
        .iter()
        .filter(|package| {
            matches!(
                package.status,
                PreparePackageStatus::Outdated | PreparePackageStatus::Error
            )
        })
        .map(|package| package_dir(&package.recipe_path))
        .collect();
    if output_dir.is_dir() {
        for directory in existing_directories(output_dir) {
            if !keep.contains(&directory) {
                remove_prepared(output_dir, &directory);
            }
        }
    }
    state
        .into_iter()
        .filter(|(key, _)| keep.contains(key))
        .collect()
}

fn candidates(report: &PrepareReport) -> Vec<&PreparePackage> {
    let mut eligible: Vec<&PreparePackage> = report
        .packages
        .iter()
        .filter(|package| package.status == PreparePackageStatus::Outdated)
        .filter(|package| !package.recipe_path.is_empty())
        .filter(|package| SAFE_DIRECTORY.is_match(&package_dir(&package.recipe_path)))
        .filter(|package| {
            package
                .latest_version
                .as_deref()
                .is_some_and(|version| !version.is_empty())
        })
        .collect();
    eligible.sort_by(|left, right| left.recipe_path.cmp(&right.recipe_path));
    eligible
}

fn attempt(
    package: &PreparePackage,
    settings: &PrepareSettings,
    download: &Downloader,
    previous: Option<&BuildEntry>,
) -> BuildEntry {
    let directory = package_dir(&package.recipe_path);
    let version = package.latest_version.clone().unwrap_or_default();
    let attempts = next_attempts(previous, &version);
    match prepare_package(package, settings, download) {
        Err(error) => {
            remove_prepared(&settings.output_dir, &directory);
            let (status, reason) = failure_outcome(&error.message, error.transient, attempts);
            BuildEntry::new(version, status, settings.run_date.clone())
                .with_reason(reason)
                .with_attempts(attempts)
        }
        Ok(prepared) => {
            write_prepared(
                &settings.output_dir,
                &directory,
                &prepared.new_pspec_text,
                &prepared.unified_diff,
            );
            BuildEntry::new(version, EntryStatus::Prepared, settings.run_date.clone())
                .with_attempts(attempts)
        }
    }
}

fn process(
    item: &QueueItem,
    current: &State,
    settings: &PrepareSettings,
    download: &Downloader,
) -> BuildEntry {
    let directory = package_dir(&item.package.recipe_path);
    let entry = current.get(&directory);
    if item.kind == QueueKind::PendingBuild
        && let Some(entry) = entry
    {
        return BuildEntry {
            attempts: entry.attempts + 1,
            date: settings.run_date.clone(),
            ..entry.clone()
        };
    }
    attempt(&item.package, settings, download, entry)
}

fn expire_lost_builds(state: State, settings: &PrepareSettings) -> State {
    state
        .into_iter()
        .map(|(directory, entry)| {
            if is_exhausted_pending(&entry, &settings.output_dir, &directory) {
                let (status, reason) = transient_outcome(BUILD_LOST_REASON, entry.attempts);
                let expired = BuildEntry {
                    status,
                    reason: Some(reason),
                    date: settings.run_date.clone(),
                    ..entry
                };
                (directory, expired)
            } else {
                (directory, entry)
            }
        })
        .collect()
}

pub fn run_prepare(
    report: &PrepareReport,
    state: State,
    settings: &PrepareSettings,
    download: &Downloader,
) -> PrepareOutcome {
    let pruned = prune(report, state, &settings.output_dir);
    let mut current = expire_lost_builds(pruned, settings);
    let eligible = candidates(report);
    let queue = build_queue(&eligible, &current, &settings.output_dir, settings.limit);
    let mut build_list = Vec::new();
    for item in &queue {
        let directory = package_dir(&item.package.recipe_path);
        let entry = process(item, &current, settings, download);
        let is_prepared = entry.status == EntryStatus::Prepared;
        current.insert(directory.clone(), entry);
        if is_prepared {
            build_list.push(directory);
        }
    }
    PrepareOutcome {
        state: current,
        build_list,
        queue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive_download::DownloadResult;
    use crate::error::ArchiveDownloadError;
    use crate::prepare_queue::QueueKind;
    use std::fs;
    use std::path::PathBuf;

    const FAKE_SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
    const ATARI: &str = "game/emulator/atari800";
    const BRAVE: &str = "network/browser/brave";

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures/pspec")
    }

    fn fake_download(url: &str) -> Result<DownloadResult, ArchiveDownloadError> {
        Ok(DownloadResult {
            url: url.to_string(),
            sha1: FAKE_SHA1.to_string(),
            size_bytes: 10,
        })
    }

    fn failing_download(_url: &str) -> Result<DownloadResult, ArchiveDownloadError> {
        Err(ArchiveDownloadError::permanent(
            "indirme başarısız (HTTP 404)",
        ))
    }

    struct Fixture {
        base: PathBuf,
        settings: PrepareSettings,
        report: PrepareReport,
    }

    fn setup() -> Fixture {
        let mut base = std::env::temp_dir();
        base.push(format!(
            "pisi-bump-bot-prepare-runner-{}-{}",
            std::process::id(),
            fastrand_seed()
        ));
        for (directory, fixture) in [(ATARI, "atari800.xml"), (BRAVE, "brave.xml")] {
            let target = base.join("contrib").join(directory);
            fs::create_dir_all(&target).unwrap();
            let text = fs::read_to_string(fixtures_dir().join(fixture)).unwrap();
            fs::write(target.join("pspec.xml"), text).unwrap();
        }
        let settings = PrepareSettings::new(base.join("contrib"), base.join("hazir"), "2026-10-07");
        let report = PrepareReport {
            packages: vec![
                PreparePackage::outdated(
                    "brave-browser",
                    &format!("{BRAVE}/pspec.xml"),
                    "v1.94.1",
                    "brave/brave-browser",
                ),
                PreparePackage::outdated(
                    "atari800",
                    &format!("{ATARI}/pspec.xml"),
                    "ATARI800_7_2_1",
                    "atari800/atari800",
                ),
            ],
        };
        Fixture {
            base,
            settings,
            report,
        }
    }

    fn fastrand_seed() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    #[test]
    fn should_prepare_files_in_path_order_when_downloads_succeed() {
        let fixture = setup();
        let outcome = run_prepare(
            &fixture.report,
            State::new(),
            &fixture.settings,
            &fake_download,
        );
        assert_eq!(
            outcome.build_list,
            vec![ATARI.to_string(), BRAVE.to_string()]
        );
        assert!(
            fixture
                .settings
                .output_dir
                .join(ATARI)
                .join("pspec.diff")
                .is_file()
        );
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_use_converted_version_and_new_url_when_tag_has_underscores() {
        let fixture = setup();
        run_prepare(
            &fixture.report,
            State::new(),
            &fixture.settings,
            &fake_download,
        );
        let text =
            fs::read_to_string(fixture.settings.output_dir.join(ATARI).join("pspec.xml")).unwrap();
        assert!(text.contains("ATARI800_7_2_1/atari800-7.2.1-src.tgz"));
        assert!(text.contains("<Version>7.2.1</Version>"));
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_record_failure_reason_when_download_fails() {
        let fixture = setup();
        let outcome = run_prepare(
            &fixture.report,
            State::new(),
            &fixture.settings,
            &failing_download,
        );
        let entry = &outcome.state[ATARI];
        assert_eq!(entry.status, EntryStatus::PrepareFailed);
        assert_eq!(
            entry.reason.as_deref(),
            Some("indirme başarısız (HTTP 404)")
        );
        assert!(outcome.build_list.is_empty());
        assert!(!fixture.settings.output_dir.join(ATARI).exists());
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_not_retry_when_same_version_already_failed() {
        let fixture = setup();
        let mut state = State::new();
        state.insert(
            ATARI.to_string(),
            BuildEntry::new("ATARI800_7_2_1", EntryStatus::PrepareFailed, "d").with_reason("x"),
        );
        let report = PrepareReport {
            packages: vec![fixture.report.packages[1].clone()],
        };
        let outcome = run_prepare(&report, state, &fixture.settings, &failing_download);
        assert_eq!(outcome.queue.len(), 0);
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_retry_when_upstream_version_changed() {
        let fixture = setup();
        let mut state = State::new();
        state.insert(
            ATARI.to_string(),
            BuildEntry::new("ATARI800_7_2_0", EntryStatus::PrepareFailed, "d").with_reason("x"),
        );
        let report = PrepareReport {
            packages: vec![fixture.report.packages[1].clone()],
        };
        let outcome = run_prepare(&report, state, &fixture.settings, &fake_download);
        assert_eq!(outcome.state[ATARI].status, EntryStatus::Prepared);
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_not_rebuild_when_build_already_finished() {
        let fixture = setup();
        let mut state = State::new();
        state.insert(
            BRAVE.to_string(),
            BuildEntry::new("v1.94.1", EntryStatus::Built, "d").with_run_id("1"),
        );
        let report = PrepareReport {
            packages: vec![fixture.report.packages[0].clone()],
        };
        let outcome = run_prepare(&report, state, &fixture.settings, &fake_download);
        assert!(outcome.build_list.is_empty());
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_respect_limit_when_many_candidates() {
        let fixture = setup();
        let limited = fixture.settings.clone().with_limit(1);
        let outcome = run_prepare(&fixture.report, State::new(), &limited, &fake_download);
        assert_eq!(outcome.build_list, vec![ATARI.to_string()]);
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_prune_files_and_state_when_package_is_no_longer_outdated() {
        let fixture = setup();
        run_prepare(
            &fixture.report,
            State::new(),
            &fixture.settings,
            &fake_download,
        );
        let mut state = State::new();
        state.insert(
            ATARI.to_string(),
            BuildEntry::new("ATARI800_7_2_1", EntryStatus::Prepared, "d"),
        );
        state.insert(
            BRAVE.to_string(),
            BuildEntry::new("v1.94.1", EntryStatus::Built, "d"),
        );
        let current =
            PreparePackage::outdated("brave-browser", &format!("{BRAVE}/pspec.xml"), "", "")
                .with_status(PreparePackageStatus::Other);
        let report = PrepareReport {
            packages: vec![fixture.report.packages[1].clone(), current],
        };
        let outcome = run_prepare(&report, state, &fixture.settings, &fake_download);
        let keys: Vec<&String> = outcome.state.keys().collect();
        assert_eq!(keys, vec![&ATARI.to_string()]);
        assert!(!fixture.settings.output_dir.join("network").exists());
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_requeue_prepared_package_when_build_never_reported() {
        let fixture = setup();
        run_prepare(
            &fixture.report,
            State::new(),
            &fixture.settings,
            &fake_download,
        );
        let mut state = State::new();
        state.insert(
            BRAVE.to_string(),
            BuildEntry::new("v1.94.1", EntryStatus::Prepared, "d"),
        );
        let report = PrepareReport {
            packages: vec![fixture.report.packages[0].clone()],
        };
        let outcome = run_prepare(&report, state, &fixture.settings, &failing_download);
        assert_eq!(outcome.build_list, vec![BRAVE.to_string()]);
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_fill_leftover_slots_by_priority_new_then_pending_then_retry() {
        let fixture = setup();
        let wide = fixture.settings.clone().with_limit(2);
        let brave_only = PrepareReport {
            packages: vec![fixture.report.packages[0].clone()],
        };
        run_prepare(&brave_only, State::new(), &fixture.settings, &fake_download);
        let mut state = State::new();
        state.insert(
            BRAVE.to_string(),
            BuildEntry::new("v1.94.1", EntryStatus::Prepared, "d").with_attempts(1),
        );
        state.insert(
            ATARI.to_string(),
            BuildEntry::new("ATARI800_7_2_1", EntryStatus::TransientFailed, "d")
                .with_reason("x")
                .with_attempts(1),
        );
        let zeta = PreparePackage::outdated("zeta", "zz/zeta/pspec.xml", "v1", "o/zeta");
        let report = PrepareReport {
            packages: vec![
                fixture.report.packages[1].clone(),
                fixture.report.packages[0].clone(),
                zeta,
            ],
        };
        let outcome = run_prepare(&report, state, &wide, &fake_download);
        let kinds: Vec<QueueKind> = outcome.queue.iter().map(|item| item.kind).collect();
        assert_eq!(kinds, vec![QueueKind::New, QueueKind::PendingBuild]);
        let _ = fs::remove_dir_all(&fixture.base);
    }

    #[test]
    fn should_expire_pending_build_when_no_result_after_three_attempts() {
        let fixture = setup();
        let brave_only = PrepareReport {
            packages: vec![fixture.report.packages[0].clone()],
        };
        run_prepare(&brave_only, State::new(), &fixture.settings, &fake_download);
        let mut state = State::new();
        state.insert(
            BRAVE.to_string(),
            BuildEntry::new("v1.94.1", EntryStatus::Prepared, "d").with_attempts(3),
        );
        let outcome = run_prepare(&brave_only, state, &fixture.settings, &fake_download);
        let entry = &outcome.state[BRAVE];
        assert_eq!(entry.status, EntryStatus::PrepareFailed);
        assert_eq!(
            entry.reason.as_deref(),
            Some("3 denemede de başarısız: derleme sonucu alınamadı")
        );
        let _ = fs::remove_dir_all(&fixture.base);
    }
}
