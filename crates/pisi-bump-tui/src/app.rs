use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use pisi_bump_bot::github_upstream::parse_github_archive;
use pisi_bump_bot::report_model::{PackageReport, Status};
use pisi_bump_common::{PackageRecipe, compare_versions, normalize_version};

use crate::config::CachedPackage;

pub const UNREADABLE_DETAIL: &str = "pspec.xml okunamadı";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareState {
    Idle,
    InProgress,
    Ready {
        diff: String,
        new_pspec_text: String,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildState {
    Idle,
    CheckingDocker,
    Unavailable(String),
    PermissionDenied(String),
    SudoPassword {
        input: String,
        error: Option<String>,
        denied_reason: String,
    },
    SudoAuthenticating {
        denied_reason: String,
    },
    Pulling,
    Building,
    Done {
        success: bool,
        log: String,
        output_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowState {
    pub name: String,
    pub recipe_path: String,
    pub scannable: bool,
    pub report: Option<PackageReport>,
    pub prepare: PrepareState,
    pub written: bool,
    pub checking: bool,
    pub build: BuildState,
}

impl RowState {
    fn scanning(name: &str, recipe_path: &str) -> Self {
        Self {
            name: name.to_string(),
            recipe_path: recipe_path.to_string(),
            scannable: true,
            report: None,
            prepare: PrepareState::Idle,
            written: false,
            checking: false,
            build: BuildState::Idle,
        }
    }

    fn unreadable(recipe_path: &str) -> Self {
        let name = unreadable_name(recipe_path);
        let report = PackageReport::new(&name, Status::Error, "")
            .with_detail(Some(UNREADABLE_DETAIL.to_string()))
            .with_recipe_path(recipe_path);
        Self {
            name,
            recipe_path: recipe_path.to_string(),
            scannable: false,
            report: Some(report),
            prepare: PrepareState::Idle,
            written: false,
            checking: false,
            build: BuildState::Idle,
        }
    }

    pub fn status_text(&self) -> &'static str {
        if self.checking {
            return "kontrol ediliyor";
        }
        if self.written {
            return "yazıldı";
        }
        match &self.report {
            None => "taranıyor",
            Some(report) => report.status.as_str(),
        }
    }

    fn max_detail_pane_scroll(&self) -> u16 {
        match &self.build {
            BuildState::Done { log, .. } => line_count_minus_one(log),
            BuildState::Idle => match &self.prepare {
                PrepareState::Ready { diff, .. } => line_count_minus_one(diff),
                _ => 0,
            },
            _ => 0,
        }
    }
}

fn line_count_minus_one(text: &str) -> u16 {
    text.lines()
        .count()
        .saturating_sub(1)
        .min(u16::MAX as usize) as u16
}

fn refresh_cached_report(cached: &PackageReport, recipe: &PackageRecipe) -> PackageReport {
    let mut report = cached.clone();
    report.current_version = recipe.current_version.clone();

    let Some(archive) = recipe
        .archive_urls
        .first()
        .and_then(|url| parse_github_archive(url))
    else {
        return report;
    };
    let Some(latest_tag) = report.latest_version.clone() else {
        return report;
    };

    let prefixes = [archive.repo.as_str(), recipe.name.as_str()];
    let current = normalize_version(&recipe.current_version, &prefixes);
    let newest = normalize_version(&latest_tag, &prefixes);
    if let (Some(current), Some(newest)) = (current, newest) {
        report.status = if compare_versions(&newest, &current) <= 0 {
            Status::Current
        } else {
            Status::Outdated
        };
    }
    report
}

fn unreadable_name(recipe_path: &str) -> String {
    std::path::Path::new(recipe_path)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(recipe_path)
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Table,
    Detail,
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    StartPrepare(usize),
    ConfirmWrite(usize),
    Rescan,
    CheckOne(usize),
    StartBuild(usize),
    RetrySudoBuild(usize, String),
    Reauth,
}

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    ScanProgress {
        index: usize,
        done: usize,
        total: usize,
        report: Box<PackageReport>,
    },
    ScanComplete,
    PrepareComplete {
        row_index: usize,
        diff: String,
        new_pspec_text: String,
    },
    PrepareFailed {
        row_index: usize,
        reason: String,
    },
    WriteComplete {
        row_index: usize,
    },
    WriteFailed {
        row_index: usize,
        reason: String,
    },
    RateLimitUpdate {
        remaining: u32,
    },
    SingleCheckComplete {
        row_index: usize,
        report: Box<PackageReport>,
    },
    BuildDockerUnavailable {
        row_index: usize,
        reason: String,
    },
    BuildPulling {
        row_index: usize,
    },
    BuildRunning {
        row_index: usize,
    },
    BuildComplete {
        row_index: usize,
        success: bool,
        log: String,
        output_dir: Option<PathBuf>,
    },
    BuildPermissionDenied {
        row_index: usize,
        reason: String,
    },
    BuildSudoRejected {
        row_index: usize,
        reason: String,
    },
}

pub struct App {
    pub recipes_dir: PathBuf,
    pub rows: Vec<RowState>,
    pub screen: Screen,
    pub selected: usize,
    pub detail_index: Option<usize>,
    pub detail_scroll: u16,
    pub show_all_statuses: bool,
    pub filter_mode: bool,
    pub filter_text: String,
    pub scan_done: usize,
    pub scan_total: usize,
    pub scanning: bool,
    pub authenticated: bool,
    pub status_message: Option<String>,
    pub write_in_progress: bool,
    pub help_visible: bool,
    pub rate_limit_remaining: Option<u32>,
    pub spinner_tick: u64,
}

impl App {
    pub fn new(
        recipes_dir: PathBuf,
        recipes: &[PackageRecipe],
        unreadable_paths: &[String],
        authenticated: bool,
    ) -> Self {
        let mut rows: Vec<RowState> = recipes
            .iter()
            .map(|recipe| RowState::scanning(&recipe.name, &recipe.recipe_path))
            .collect();
        rows.extend(
            unreadable_paths
                .iter()
                .map(|path| RowState::unreadable(path)),
        );
        Self {
            recipes_dir,
            scan_total: recipes.len(),
            rows,
            screen: Screen::Table,
            selected: 0,
            detail_index: None,
            detail_scroll: 0,
            show_all_statuses: false,
            filter_mode: false,
            filter_text: String::new(),
            scan_done: 0,
            scanning: true,
            authenticated,
            status_message: None,
            write_in_progress: false,
            help_visible: false,
            rate_limit_remaining: None,
            spinner_tick: 0,
        }
    }

    pub fn advance_spinner(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        crate::table::visible_indices(&self.rows, self.show_all_statuses, &self.filter_text)
    }

    pub fn apply_cache(
        &mut self,
        recipes: &[PackageRecipe],
        cache: &HashMap<String, CachedPackage>,
    ) {
        if cache.is_empty() {
            return;
        }
        for row in self.rows.iter_mut() {
            if let Some(cached) = cache.get(&row.recipe_path) {
                let recipe = recipes
                    .iter()
                    .find(|recipe| recipe.recipe_path == row.recipe_path);
                row.report = Some(match recipe {
                    Some(recipe) => refresh_cached_report(&cached.report, recipe),
                    None => cached.report.clone(),
                });
                row.written = cached.written;
            }
        }
        self.scanning = false;
        self.scan_done = self.scan_total;
    }

    pub fn apply(&mut self, message: WorkerMessage) {
        match message {
            WorkerMessage::ScanProgress {
                index,
                done,
                total,
                report,
            } => {
                self.scan_done = done;
                self.scan_total = total;
                if let Some(row) = self.rows.get_mut(index) {
                    row.report = Some(*report);
                }
            }
            WorkerMessage::ScanComplete => {
                self.scanning = false;
            }
            WorkerMessage::PrepareComplete {
                row_index,
                diff,
                new_pspec_text,
            } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.prepare = PrepareState::Ready {
                        diff,
                        new_pspec_text,
                    };
                }
                self.detail_scroll = 0;
            }
            WorkerMessage::PrepareFailed { row_index, reason } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.prepare = PrepareState::Failed { reason };
                }
            }
            WorkerMessage::WriteComplete { row_index } => {
                self.write_in_progress = false;
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.written = true;
                    if let Some(report) = row.report.as_mut()
                        && let Some(latest) = report.latest_version.clone()
                    {
                        report.current_version = latest;
                    }
                }
                self.status_message = Some(format!(
                    "pspec.xml güncellendi ({})",
                    self.rows
                        .get(row_index)
                        .map(|row| row.recipe_path.as_str())
                        .unwrap_or("pspec.xml")
                ));
                self.screen = Screen::Detail;
            }
            WorkerMessage::WriteFailed { row_index, reason } => {
                self.write_in_progress = false;
                self.status_message = Some(format!("yazma başarısız: {reason}"));
                if self.detail_index == Some(row_index) {
                    self.screen = Screen::Detail;
                }
            }
            WorkerMessage::RateLimitUpdate { remaining } => {
                self.rate_limit_remaining = Some(remaining);
            }
            WorkerMessage::SingleCheckComplete { row_index, report } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.checking = false;
                    row.report = Some(*report);
                }
            }
            WorkerMessage::BuildDockerUnavailable { row_index, reason } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.build = BuildState::Unavailable(reason);
                }
            }
            WorkerMessage::BuildPulling { row_index } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.build = BuildState::Pulling;
                }
            }
            WorkerMessage::BuildRunning { row_index } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.build = BuildState::Building;
                }
            }
            WorkerMessage::BuildComplete {
                row_index,
                success,
                log,
                output_dir,
            } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.build = BuildState::Done {
                        success,
                        log,
                        output_dir,
                    };
                }
                self.detail_scroll = self
                    .rows
                    .get(row_index)
                    .map(RowState::max_detail_pane_scroll)
                    .unwrap_or(0);
            }
            WorkerMessage::BuildPermissionDenied { row_index, reason } => {
                if let Some(row) = self.rows.get_mut(row_index) {
                    row.build = BuildState::PermissionDenied(reason);
                }
            }
            WorkerMessage::BuildSudoRejected { row_index, reason } => {
                if let Some(row) = self.rows.get_mut(row_index)
                    && let BuildState::SudoAuthenticating { denied_reason } = row.build.clone()
                {
                    row.build = BuildState::SudoPassword {
                        input: String::new(),
                        error: Some(reason),
                        denied_reason,
                    };
                }
            }
        }
    }

    pub fn reset_for_rescan(&mut self) {
        for row in self.rows.iter_mut() {
            row.checking = false;
            if row.scannable {
                row.report = None;
                row.prepare = PrepareState::Idle;
            }
        }
        self.scanning = true;
        self.scan_done = 0;
        self.scan_total = self.rows.iter().filter(|row| row.scannable).count();
        self.screen = Screen::Table;
        self.selected = 0;
        self.detail_index = None;
        self.status_message = None;
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Option<Action> {
        if self.help_visible {
            if matches!(key, KeyCode::Esc | KeyCode::Char('?')) {
                self.help_visible = false;
            }
            return None;
        }
        if !self.filter_mode && key == KeyCode::Char('?') {
            self.help_visible = true;
            return None;
        }
        match self.screen {
            Screen::Table if self.filter_mode => self.handle_filter_key(key),
            Screen::Table => self.handle_table_key(key),
            Screen::Detail => self.handle_detail_key(key),
            Screen::Confirm => self.handle_confirm_key(key),
        }
    }

    fn handle_table_key(&mut self, key: KeyCode) -> Option<Action> {
        let visible = self.visible_indices();
        match key {
            KeyCode::Char('q') => return Some(Action::Quit),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1, &visible),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1, &visible),
            KeyCode::Char('o') => {
                self.show_all_statuses = !self.show_all_statuses;
                self.selected = 0;
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
            }
            KeyCode::Esc => {
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.selected = 0;
                }
            }
            KeyCode::Char('r') => {
                if !self.scanning && !self.write_in_progress {
                    return Some(Action::Rescan);
                }
            }
            KeyCode::Char('v') => {
                if let Some(&absolute) = visible.get(self.selected) {
                    return self.request_single_check(absolute);
                }
            }
            KeyCode::Char('A') => return Some(Action::Reauth),
            KeyCode::Enter => {
                if let Some(&absolute) = visible.get(self.selected) {
                    self.detail_index = Some(absolute);
                    self.detail_scroll = 0;
                    self.status_message = None;
                    self.screen = Screen::Detail;
                }
            }
            _ => {}
        }
        None
    }

    fn request_single_check(&mut self, absolute: usize) -> Option<Action> {
        let can_check = !self.scanning
            && self
                .rows
                .get(absolute)
                .map(|row| row.scannable && !row.checking)
                .unwrap_or(false);
        if !can_check {
            return None;
        }
        self.rows[absolute].checking = true;
        Some(Action::CheckOne(absolute))
    }

    fn move_selection(&mut self, delta: isize, visible: &[usize]) {
        if visible.is_empty() {
            self.selected = 0;
            return;
        }
        let length = visible.len() as isize;
        let next = (self.selected as isize + delta).rem_euclid(length);
        self.selected = next as usize;
    }

    fn handle_filter_key(&mut self, key: KeyCode) -> Option<Action> {
        match key {
            KeyCode::Esc => {
                self.filter_text.clear();
                self.filter_mode = false;
                self.selected = 0;
            }
            KeyCode::Enter => {
                self.filter_mode = false;
            }
            KeyCode::Backspace => {
                self.filter_text.pop();
                self.selected = 0;
            }
            KeyCode::Char(character) => {
                self.filter_text.push(character);
                self.selected = 0;
            }
            _ => {}
        }
        None
    }

    fn handle_detail_key(&mut self, key: KeyCode) -> Option<Action> {
        let Some(index) = self.detail_index else {
            self.screen = Screen::Table;
            return None;
        };
        if matches!(self.rows[index].build, BuildState::SudoPassword { .. }) {
            return self.handle_sudo_password_key(index, key);
        }
        match key {
            KeyCode::Esc => {
                self.screen = Screen::Table;
                self.detail_index = None;
            }
            KeyCode::Char('p') => {
                let can_start = self
                    .rows
                    .get(index)
                    .map(|row| !matches!(row.prepare, PrepareState::InProgress))
                    .unwrap_or(false);
                if can_start {
                    if let Some(row) = self.rows.get_mut(index) {
                        row.prepare = PrepareState::InProgress;
                    }
                    return Some(Action::StartPrepare(index));
                }
            }
            KeyCode::Char('w') => {
                let ready = self
                    .rows
                    .get(index)
                    .map(|row| matches!(row.prepare, PrepareState::Ready { .. }))
                    .unwrap_or(false);
                if ready {
                    self.screen = Screen::Confirm;
                }
            }
            KeyCode::Char('v') => {
                return self.request_single_check(index);
            }
            KeyCode::Char('b') => {
                return self.request_build(index);
            }
            KeyCode::Char('s') => {
                if let BuildState::PermissionDenied(reason) = self.rows[index].build.clone() {
                    self.rows[index].build = BuildState::SudoPassword {
                        input: String::new(),
                        error: None,
                        denied_reason: reason,
                    };
                }
            }
            KeyCode::Up => self.scroll_detail(index, -1),
            KeyCode::Down => self.scroll_detail(index, 1),
            KeyCode::PageUp => self.scroll_detail(index, -10),
            KeyCode::PageDown => self.scroll_detail(index, 10),
            _ => {}
        }
        None
    }

    fn handle_sudo_password_key(&mut self, index: usize, key: KeyCode) -> Option<Action> {
        let BuildState::SudoPassword {
            input,
            denied_reason,
            ..
        } = self.rows[index].build.clone()
        else {
            return None;
        };
        match key {
            KeyCode::Esc => {
                self.rows[index].build = BuildState::PermissionDenied(denied_reason);
                None
            }
            KeyCode::Backspace => {
                let mut next_input = input;
                next_input.pop();
                self.rows[index].build = BuildState::SudoPassword {
                    input: next_input,
                    error: None,
                    denied_reason,
                };
                None
            }
            KeyCode::Char(character) => {
                let mut next_input = input;
                next_input.push(character);
                self.rows[index].build = BuildState::SudoPassword {
                    input: next_input,
                    error: None,
                    denied_reason,
                };
                None
            }
            KeyCode::Enter => {
                if input.is_empty() {
                    return None;
                }
                self.rows[index].build = BuildState::SudoAuthenticating { denied_reason };
                Some(Action::RetrySudoBuild(index, input))
            }
            _ => None,
        }
    }

    fn request_build(&mut self, index: usize) -> Option<Action> {
        let can_start = self
            .rows
            .get(index)
            .map(|row| {
                !matches!(
                    row.build,
                    BuildState::CheckingDocker
                        | BuildState::Pulling
                        | BuildState::Building
                        | BuildState::SudoAuthenticating { .. }
                )
            })
            .unwrap_or(false);
        if !can_start {
            return None;
        }
        self.rows[index].build = BuildState::CheckingDocker;
        self.detail_scroll = 0;
        Some(Action::StartBuild(index))
    }

    fn scroll_detail(&mut self, index: usize, delta: i32) {
        let max_scroll = self
            .rows
            .get(index)
            .map(RowState::max_detail_pane_scroll)
            .unwrap_or(0) as i32;
        let next = (self.detail_scroll as i32 + delta).clamp(0, max_scroll);
        self.detail_scroll = next as u16;
    }

    fn handle_confirm_key(&mut self, key: KeyCode) -> Option<Action> {
        let Some(index) = self.detail_index else {
            self.screen = Screen::Table;
            return None;
        };
        match key {
            KeyCode::Char('y') => {
                self.write_in_progress = true;
                return Some(Action::ConfirmWrite(index));
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.screen = Screen::Detail;
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(name: &str, path: &str) -> PackageRecipe {
        PackageRecipe {
            recipe_path: path.to_string(),
            name: name.to_string(),
            current_version: "1.0".to_string(),
            current_release: 1,
            archive_urls: vec!["https://github.com/o/r/archive/1.0.tar.gz".to_string()],
        }
    }

    fn outdated_report(name: &str, path: &str) -> PackageReport {
        PackageReport::new(name, Status::Outdated, "1.0")
            .with_latest_version("1.1")
            .with_candidate_url(Some("https://example.org/archive-1.1.tar.gz".to_string()))
            .with_recipe_path(path)
    }

    #[test]
    fn should_mark_rows_as_scanning_when_app_is_new() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        assert_eq!(app.rows.len(), 1);
        assert_eq!(app.rows[0].report, None);
        assert!(app.scanning);
        assert_eq!(app.scan_total, 1);
    }

    #[test]
    fn should_append_unreadable_rows_with_error_status() {
        let app = App::new(
            PathBuf::from("/tmp/contrib"),
            &[],
            &["broken/pspec.xml".to_string()],
            true,
        );
        assert_eq!(app.rows.len(), 1);
        let row = &app.rows[0];
        assert!(!row.scannable);
        assert_eq!(row.report.as_ref().unwrap().status, Status::Error);
        assert_eq!(row.name, "broken");
    }

    #[test]
    fn should_fill_in_report_when_scan_progress_arrives() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(outdated_report(
                "brave-browser",
                "network/browser/brave/pspec.xml",
            )),
        });
        assert_eq!(app.scan_done, 1);
        assert_eq!(
            app.rows[0].report.as_ref().unwrap().status,
            Status::Outdated
        );
    }

    #[test]
    fn should_stop_scanning_when_scan_complete_arrives() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        assert!(app.scanning);
        app.apply(WorkerMessage::ScanComplete);
        assert!(!app.scanning);
    }

    #[test]
    fn should_store_diff_when_prepare_completes() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::PrepareComplete {
            row_index: 0,
            diff: "--- a\n+++ b\n".to_string(),
            new_pspec_text: "<PISI/>".to_string(),
        });
        match &app.rows[0].prepare {
            PrepareState::Ready { diff, .. } => assert_eq!(diff, "--- a\n+++ b\n"),
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn should_store_failure_reason_when_prepare_fails() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::PrepareFailed {
            row_index: 0,
            reason: "indirme başarısız".to_string(),
        });
        match &app.rows[0].prepare {
            PrepareState::Failed { reason } => assert_eq!(reason, "indirme başarısız"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn should_mark_row_written_and_stay_on_detail_screen_so_build_is_immediately_available() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Confirm;
        app.detail_index = Some(0);
        app.write_in_progress = true;
        app.apply(WorkerMessage::WriteComplete { row_index: 0 });
        assert!(app.rows[0].written);
        assert!(!app.write_in_progress);
        assert_eq!(
            app.screen,
            Screen::Detail,
            "write completing must keep the user on the same row's detail screen, not kick them back to the table"
        );
        assert_eq!(app.detail_index, Some(0));

        let action = app.handle_key(KeyCode::Char('b'));
        assert_eq!(
            action,
            Some(Action::StartBuild(0)),
            "b must be immediately startable right after write completes, with no extra navigation"
        );
    }

    #[test]
    fn should_return_to_detail_screen_when_write_fails() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Confirm;
        app.detail_index = Some(0);
        app.write_in_progress = true;
        app.apply(WorkerMessage::WriteFailed {
            row_index: 0,
            reason: "izin reddedildi".to_string(),
        });
        assert!(!app.rows[0].written);
        assert!(!app.write_in_progress);
        assert_eq!(app.screen, Screen::Detail);
        assert_eq!(
            app.status_message.as_deref(),
            Some("yazma başarısız: izin reddedildi")
        );
    }

    #[test]
    fn should_enter_detail_for_any_selected_row_regardless_of_status_or_written() {
        let recipes = vec![
            recipe("current-pkg", "a/pspec.xml"),
            recipe("outdated-pkg", "b/pspec.xml"),
        ];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 2,
            report: Box::new(PackageReport::new("current-pkg", Status::Current, "1.0")),
        });
        app.apply(WorkerMessage::ScanProgress {
            index: 1,
            done: 2,
            total: 2,
            report: Box::new(outdated_report("outdated-pkg", "b/pspec.xml")),
        });
        app.show_all_statuses = true;

        app.selected = 0;
        assert_eq!(app.handle_key(KeyCode::Enter), None);
        assert_eq!(
            app.screen,
            Screen::Detail,
            "Enter must open Detail for a plain Status::Current, written:false row \
             (the exact case the user kept hitting) — the entry gate was removed, not just loosened"
        );
        assert_eq!(app.detail_index, Some(0));

        app.screen = Screen::Table;
        app.detail_index = None;
        app.selected = 1;
        assert_eq!(app.handle_key(KeyCode::Enter), None);
        assert_eq!(app.screen, Screen::Detail);
        assert_eq!(app.detail_index, Some(1));
    }

    #[test]
    fn should_enter_detail_for_a_written_row_even_after_it_shows_current() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(PackageReport::new("brave-browser", Status::Current, "1.1")),
        });
        app.rows[0].written = true;
        app.show_all_statuses = true;
        app.selected = 0;

        assert_eq!(app.handle_key(KeyCode::Enter), None);

        assert_eq!(
            app.screen,
            Screen::Detail,
            "a written row must stay enterable (so b/build is reachable) even once its status has refreshed to current"
        );
        assert_eq!(app.detail_index, Some(0));
    }

    #[test]
    fn should_request_prepare_action_when_p_pressed_on_outdated_detail() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(outdated_report("outdated-pkg", "b/pspec.xml")),
        });
        app.screen = Screen::Detail;
        app.detail_index = Some(0);

        let action = app.handle_key(KeyCode::Char('p'));
        assert_eq!(action, Some(Action::StartPrepare(0)));
        assert_eq!(app.rows[0].prepare, PrepareState::InProgress);

        assert_eq!(app.handle_key(KeyCode::Char('p')), None);
    }

    #[test]
    fn should_move_to_confirm_screen_only_when_prepare_is_ready() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);

        app.handle_key(KeyCode::Char('w'));
        assert_eq!(app.screen, Screen::Detail);

        app.rows[0].prepare = PrepareState::Ready {
            diff: "diff".to_string(),
            new_pspec_text: "<PISI/>".to_string(),
        };
        app.handle_key(KeyCode::Char('w'));
        assert_eq!(app.screen, Screen::Confirm);
    }

    #[test]
    fn should_request_write_action_only_on_explicit_y() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Confirm;
        app.detail_index = Some(0);

        assert_eq!(app.handle_key(KeyCode::Char('n')), None);
        assert_eq!(app.screen, Screen::Detail);

        app.screen = Screen::Confirm;
        let action = app.handle_key(KeyCode::Char('y'));
        assert_eq!(action, Some(Action::ConfirmWrite(0)));
        assert!(app.write_in_progress);
    }

    #[test]
    fn should_not_allow_rescan_while_scanning_or_writing() {
        let recipes = vec![recipe("pkg", "a/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        assert!(app.scanning);
        assert_eq!(app.handle_key(KeyCode::Char('r')), None);

        app.scanning = false;
        app.write_in_progress = true;
        assert_eq!(app.handle_key(KeyCode::Char('r')), None);

        app.write_in_progress = false;
        assert_eq!(app.handle_key(KeyCode::Char('r')), Some(Action::Rescan));
    }

    #[test]
    fn should_clear_report_for_scannable_rows_on_rescan() {
        let recipes = vec![recipe("pkg", "a/pspec.xml")];
        let mut app = App::new(
            PathBuf::from("/tmp/contrib"),
            &recipes,
            &["broken/pspec.xml".to_string()],
            true,
        );
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(outdated_report("pkg", "a/pspec.xml")),
        });
        app.apply(WorkerMessage::ScanComplete);
        app.reset_for_rescan();
        assert_eq!(app.rows[0].report, None);
        assert!(app.rows[1].report.is_some());
        assert!(app.scanning);
        assert_eq!(app.scan_total, 1);
    }

    #[test]
    fn should_toggle_help_overlay_and_swallow_other_keys_while_open() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        assert!(!app.help_visible);
        assert_eq!(app.handle_key(KeyCode::Char('?')), None);
        assert!(app.help_visible);
        assert_eq!(app.handle_key(KeyCode::Char('q')), None);
        assert!(app.help_visible, "help overlay should swallow q, not quit");
        assert_eq!(app.handle_key(KeyCode::Esc), None);
        assert!(!app.help_visible);
    }

    #[test]
    fn should_record_rate_limit_remaining_when_update_arrives() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], false);
        assert_eq!(app.rate_limit_remaining, None);
        app.apply(WorkerMessage::RateLimitUpdate { remaining: 42 });
        assert_eq!(app.rate_limit_remaining, Some(42));
    }

    #[test]
    fn should_clamp_diff_scroll_at_the_last_line_when_scrolling_past_the_end() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].prepare = PrepareState::Ready {
            diff: "line1\nline2\nline3\nline4\nline5\n".to_string(),
            new_pspec_text: "<PISI/>".to_string(),
        };

        for _ in 0..50 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(app.detail_scroll, 4, "scroll must stop at line_count - 1");

        for _ in 0..50 {
            app.handle_key(KeyCode::PageDown);
        }
        assert_eq!(
            app.detail_scroll, 4,
            "PageDown must not overshoot past the last line either"
        );

        for _ in 0..50 {
            app.handle_key(KeyCode::Up);
        }
        assert_eq!(app.detail_scroll, 0, "scroll must not go below zero");
    }

    #[test]
    fn should_not_scroll_when_prepare_is_not_ready() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);

        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::PageDown);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn should_replace_stale_status_with_written_label_after_write_completes() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(outdated_report("outdated-pkg", "b/pspec.xml")),
        });
        assert_eq!(app.rows[0].status_text(), "eski");

        app.apply(WorkerMessage::WriteComplete { row_index: 0 });

        assert_ne!(
            app.rows[0].status_text(),
            "eski",
            "status_text must not still report the stale pre-write status"
        );
        assert_eq!(app.rows[0].status_text(), "yazıldı");
        assert!(app.rows[0].written);
        assert_eq!(
            app.rows[0].report.as_ref().unwrap().current_version,
            "1.1",
            "current_version should be updated to the version that was actually written"
        );
    }

    #[test]
    fn should_not_auto_scan_when_cache_is_populated_at_startup() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        assert!(app.scanning, "sanity check: App::new still starts scanning");

        let mut cache = HashMap::new();
        cache.insert(
            "b/pspec.xml".to_string(),
            CachedPackage {
                report: outdated_report("outdated-pkg", "b/pspec.xml"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: false,
            },
        );
        app.apply_cache(&recipes, &cache);

        assert!(
            !app.scanning,
            "a populated cache must disable the automatic startup scan"
        );
        assert_eq!(
            app.rows[0].report.as_ref().unwrap().status,
            Status::Outdated
        );
    }

    #[test]
    fn should_keep_auto_scanning_when_cache_is_empty_at_startup() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);

        app.apply_cache(&recipes, &HashMap::new());

        assert!(
            app.scanning,
            "first-ever run with no cache must still trigger the full scan"
        );
        assert_eq!(app.rows[0].report, None);
    }

    #[test]
    fn should_ignore_cache_entries_for_unknown_recipe_paths() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);

        let mut cache = HashMap::new();
        cache.insert(
            "some/other/pspec.xml".to_string(),
            CachedPackage {
                report: outdated_report("other-pkg", "some/other/pspec.xml"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: false,
            },
        );
        app.apply_cache(&recipes, &cache);

        assert!(!app.scanning, "a non-empty cache still disables auto-scan");
        assert_eq!(
            app.rows[0].report, None,
            "a cache entry for a different recipe_path must not populate this row"
        );
    }

    #[test]
    fn should_prefer_fresh_on_disk_current_version_over_stale_cached_version_on_startup() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        assert_eq!(
            recipes[0].current_version, "1.0",
            "sanity check: the real on-disk recipe already shows the written version"
        );

        let mut cache = HashMap::new();
        cache.insert(
            "b/pspec.xml".to_string(),
            CachedPackage {
                report: PackageReport::new("outdated-pkg", Status::Outdated, "0.9")
                    .with_latest_version("1.0")
                    .with_recipe_path("b/pspec.xml"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: false,
            },
        );

        app.apply_cache(&recipes, &cache);

        let report = app.rows[0].report.as_ref().unwrap();
        assert_eq!(
            report.current_version, "1.0",
            "the fresh on-disk current_version must win over the stale cached one"
        );
        assert_eq!(
            report.status,
            Status::Current,
            "once current_version catches up to latest_version the row must show up to date, not the stale 'eski' from before the write"
        );
    }

    #[test]
    fn should_restore_written_flag_from_cache_and_allow_entry_on_a_fresh_app_after_restart() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);

        let mut cache = HashMap::new();
        cache.insert(
            "network/browser/brave/pspec.xml".to_string(),
            CachedPackage {
                report: PackageReport::new("brave-browser", Status::Current, "1.1")
                    .with_latest_version("1.1")
                    .with_recipe_path("network/browser/brave/pspec.xml"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: true,
            },
        );

        app.apply_cache(&recipes, &cache);

        assert!(
            app.rows[0].written,
            "a row that was written in a previous session must come back as written=true after apply_cache, with no live WriteComplete in this session"
        );

        app.show_all_statuses = true;
        app.selected = 0;
        assert_eq!(app.handle_key(KeyCode::Enter), None);
        assert_eq!(
            app.screen,
            Screen::Detail,
            "Enter must succeed on a row restored as written=true from the cache alone"
        );
    }

    #[test]
    fn should_request_single_check_and_mark_row_as_checking() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(outdated_report("outdated-pkg", "b/pspec.xml")),
        });
        app.apply(WorkerMessage::ScanComplete);

        let action = app.handle_key(KeyCode::Char('v'));

        assert_eq!(action, Some(Action::CheckOne(0)));
        assert!(app.rows[0].checking);
        assert_eq!(app.rows[0].status_text(), "kontrol ediliyor");
    }

    #[test]
    fn should_not_allow_single_check_while_already_checking_or_scanning() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);

        assert_eq!(
            app.handle_key(KeyCode::Char('v')),
            None,
            "must not start a single check while the full scan is still running"
        );

        app.apply(WorkerMessage::ScanComplete);
        assert_eq!(
            app.handle_key(KeyCode::Char('v')),
            Some(Action::CheckOne(0))
        );
        assert_eq!(
            app.handle_key(KeyCode::Char('v')),
            None,
            "must not start a second check while one is already in flight"
        );
    }

    #[test]
    fn should_apply_single_check_result_to_the_correct_row() {
        let recipes = vec![
            recipe("pkg-a", "a/pspec.xml"),
            recipe("pkg-b", "b/pspec.xml"),
        ];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanComplete);
        app.rows[1].checking = true;

        app.apply(WorkerMessage::SingleCheckComplete {
            row_index: 1,
            report: Box::new(PackageReport::new("pkg-b", Status::Current, "2.0")),
        });

        assert!(!app.rows[1].checking);
        assert_eq!(app.rows[1].report.as_ref().unwrap().status, Status::Current);
        assert_eq!(app.rows[0].report, None, "row 0 must be untouched");
    }

    #[test]
    fn should_allow_build_on_a_plain_current_unwritten_row() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 1,
            report: Box::new(PackageReport::new("brave-browser", Status::Current, "1.1")),
        });
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        assert!(!app.rows[0].written);

        let action = app.handle_key(KeyCode::Char('b'));

        assert_eq!(
            action,
            Some(Action::StartBuild(0)),
            "b must start a build on any row, including an already up-to-date one that was never written, \
             so the user can sanity-check a build without going through p/w first"
        );
        assert_eq!(app.rows[0].build, BuildState::CheckingDocker);
    }

    #[test]
    fn should_request_build_action_and_mark_checking_docker_once_written() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].written = true;

        let action = app.handle_key(KeyCode::Char('b'));

        assert_eq!(action, Some(Action::StartBuild(0)));
        assert_eq!(app.rows[0].build, BuildState::CheckingDocker);
    }

    #[test]
    fn should_reset_detail_scroll_when_starting_a_fresh_build_after_a_previous_one_finished() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].written = true;
        app.rows[0].build = BuildState::Done {
            success: true,
            log: "line1\nline2\nline3\nline4\nline5\n".to_string(),
            output_dir: None,
        };
        app.detail_scroll = 4;

        let action = app.handle_key(KeyCode::Char('b'));

        assert_eq!(action, Some(Action::StartBuild(0)));
        assert_eq!(
            app.detail_scroll, 0,
            "retrying a build must reset the scroll position, otherwise the short in-progress status text renders scrolled past and the pane looks blank"
        );
    }

    #[test]
    fn should_not_allow_second_build_while_one_in_flight() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].written = true;

        assert_eq!(
            app.handle_key(KeyCode::Char('b')),
            Some(Action::StartBuild(0))
        );
        assert_eq!(
            app.handle_key(KeyCode::Char('b')),
            None,
            "must not start a second build while one is already running"
        );

        app.rows[0].build = BuildState::Done {
            success: true,
            log: "tamam".to_string(),
            output_dir: None,
        };
        assert_eq!(
            app.handle_key(KeyCode::Char('b')),
            Some(Action::StartBuild(0)),
            "a finished build must allow starting a fresh one"
        );
    }

    #[test]
    fn should_apply_docker_unavailable_message_to_build_state() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::BuildDockerUnavailable {
            row_index: 0,
            reason: "Docker bulunamadı veya çalışmıyor".to_string(),
        });
        assert_eq!(
            app.rows[0].build,
            BuildState::Unavailable("Docker bulunamadı veya çalışmıyor".to_string())
        );
    }

    #[test]
    fn should_move_through_pulling_and_running_states() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);

        app.apply(WorkerMessage::BuildPulling { row_index: 0 });
        assert_eq!(app.rows[0].build, BuildState::Pulling);

        app.apply(WorkerMessage::BuildRunning { row_index: 0 });
        assert_eq!(app.rows[0].build, BuildState::Building);

        app.apply(WorkerMessage::BuildComplete {
            row_index: 0,
            success: true,
            log: "== tamam ==".to_string(),
            output_dir: Some(PathBuf::from("/tmp/contrib-builds/b")),
        });
        match &app.rows[0].build {
            BuildState::Done {
                success,
                log,
                output_dir,
            } => {
                assert!(success);
                assert_eq!(log, "== tamam ==");
                assert_eq!(output_dir, &Some(PathBuf::from("/tmp/contrib-builds/b")));
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn should_scroll_to_the_end_of_the_build_log_when_build_completes() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.detail_scroll = 0;

        app.apply(WorkerMessage::BuildComplete {
            row_index: 0,
            success: false,
            log: "line1\nline2\nline3\nline4\nline5\n".to_string(),
            output_dir: None,
        });

        assert_eq!(
            app.detail_scroll, 4,
            "landing on a failed build must land on the LAST lines, not the top, so the error is visible by default"
        );
    }

    #[test]
    fn should_still_allow_scrolling_up_from_the_end_of_a_completed_build_log() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.apply(WorkerMessage::BuildComplete {
            row_index: 0,
            success: true,
            log: "line1\nline2\nline3\n".to_string(),
            output_dir: None,
        });
        assert_eq!(app.detail_scroll, 2);

        app.handle_key(KeyCode::Up);
        assert_eq!(app.detail_scroll, 1);
    }

    #[test]
    fn should_clamp_build_log_scroll_using_the_same_mechanism_as_the_diff_pane() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].build = BuildState::Done {
            success: false,
            log: "l1\nl2\nl3\n".to_string(),
            output_dir: None,
        };

        for _ in 0..50 {
            app.handle_key(KeyCode::Down);
        }
        assert_eq!(
            app.detail_scroll, 2,
            "build log scroll must clamp at line_count - 1 just like the diff pane"
        );

        for _ in 0..50 {
            app.handle_key(KeyCode::Up);
        }
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn should_request_reauth_action_from_table_screen() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        assert_eq!(app.handle_key(KeyCode::Char('A')), Some(Action::Reauth));
    }

    fn permission_denied_app() -> App {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        app.rows[0].written = true;
        app.rows[0].build = BuildState::PermissionDenied("izin reddedildi".to_string());
        app
    }

    #[test]
    fn should_enter_sudo_password_entry_when_s_pressed_on_permission_denied() {
        let mut app = permission_denied_app();
        assert_eq!(app.handle_key(KeyCode::Char('s')), None);
        match &app.rows[0].build {
            BuildState::SudoPassword {
                input,
                error,
                denied_reason,
            } => {
                assert_eq!(input, "");
                assert_eq!(error, &None);
                assert_eq!(denied_reason, "izin reddedildi");
            }
            other => panic!("expected SudoPassword, got {other:?}"),
        }
    }

    #[test]
    fn should_ignore_s_when_build_state_is_not_permission_denied() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Detail;
        app.detail_index = Some(0);
        assert_eq!(app.handle_key(KeyCode::Char('s')), None);
        assert_eq!(app.rows[0].build, BuildState::Idle);
    }

    #[test]
    fn should_mask_free_text_input_while_entering_sudo_password() {
        let mut app = permission_denied_app();
        app.handle_key(KeyCode::Char('s'));

        app.handle_key(KeyCode::Char('h'));
        app.handle_key(KeyCode::Char('i'));
        match &app.rows[0].build {
            BuildState::SudoPassword { input, .. } => assert_eq!(input, "hi"),
            other => panic!("expected SudoPassword, got {other:?}"),
        }

        app.handle_key(KeyCode::Backspace);
        match &app.rows[0].build {
            BuildState::SudoPassword { input, .. } => assert_eq!(input, "h"),
            other => panic!("expected SudoPassword, got {other:?}"),
        }
    }

    #[test]
    fn should_not_submit_empty_sudo_password() {
        let mut app = permission_denied_app();
        app.handle_key(KeyCode::Char('s'));
        assert_eq!(app.handle_key(KeyCode::Enter), None);
        assert!(matches!(app.rows[0].build, BuildState::SudoPassword { .. }));
    }

    #[test]
    fn should_submit_sudo_password_as_retry_action_and_move_to_authenticating() {
        let mut app = permission_denied_app();
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Char('x'));
        app.handle_key(KeyCode::Char('y'));
        app.handle_key(KeyCode::Char('z'));

        let action = app.handle_key(KeyCode::Enter);

        assert_eq!(action, Some(Action::RetrySudoBuild(0, "xyz".to_string())));
        match &app.rows[0].build {
            BuildState::SudoAuthenticating { denied_reason } => {
                assert_eq!(denied_reason, "izin reddedildi");
            }
            other => panic!("expected SudoAuthenticating, got {other:?}"),
        }
    }

    #[test]
    fn should_cancel_sudo_password_entry_and_restore_permission_denied_on_escape() {
        let mut app = permission_denied_app();
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Char('x'));

        app.handle_key(KeyCode::Esc);

        assert_eq!(
            app.rows[0].build,
            BuildState::PermissionDenied("izin reddedildi".to_string())
        );
    }

    #[test]
    fn should_clear_input_and_show_error_on_wrong_password_without_losing_denied_reason() {
        let mut app = permission_denied_app();
        app.handle_key(KeyCode::Char('s'));
        app.handle_key(KeyCode::Char('x'));
        app.handle_key(KeyCode::Enter);

        app.apply(WorkerMessage::BuildSudoRejected {
            row_index: 0,
            reason: "Yanlış şifre, tekrar deneyin".to_string(),
        });

        match &app.rows[0].build {
            BuildState::SudoPassword {
                input,
                error,
                denied_reason,
            } => {
                assert_eq!(input, "", "input must be cleared after a failed attempt");
                assert_eq!(error.as_deref(), Some("Yanlış şifre, tekrar deneyin"));
                assert_eq!(denied_reason, "izin reddedildi");
            }
            other => panic!("expected SudoPassword, got {other:?}"),
        }
    }

    #[test]
    fn should_apply_permission_denied_message_when_sudo_is_available() {
        let recipes = vec![recipe("outdated-pkg", "b/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::BuildPermissionDenied {
            row_index: 0,
            reason: "Docker soketine erişim izniniz yok.".to_string(),
        });
        assert_eq!(
            app.rows[0].build,
            BuildState::PermissionDenied("Docker soketine erişim izniniz yok.".to_string())
        );
    }

    #[test]
    fn should_clear_filter_text_on_escape() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        app.filter_mode = true;
        app.filter_text = "bra".to_string();
        app.handle_key(KeyCode::Esc);
        assert!(!app.filter_mode);
        assert_eq!(app.filter_text, "");
    }

    #[test]
    fn should_clear_a_confirmed_filter_from_the_table_screen_on_escape() {
        let recipes = vec![
            recipe("brave-browser", "a/pspec.xml"),
            recipe("firefox", "b/pspec.xml"),
        ];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.apply(WorkerMessage::ScanProgress {
            index: 0,
            done: 1,
            total: 2,
            report: Box::new(PackageReport::new("brave-browser", Status::Current, "1.0")),
        });
        app.apply(WorkerMessage::ScanProgress {
            index: 1,
            done: 2,
            total: 2,
            report: Box::new(PackageReport::new("firefox", Status::Current, "1.0")),
        });
        app.show_all_statuses = true;
        let full_count = app.visible_indices().len();
        assert_eq!(
            full_count, 2,
            "sanity check: both rows visible before filtering"
        );

        app.handle_key(KeyCode::Char('/'));
        app.handle_key(KeyCode::Char('b'));
        app.handle_key(KeyCode::Char('r'));
        app.handle_key(KeyCode::Char('a'));
        assert_eq!(
            app.visible_indices().len(),
            1,
            "filter must narrow the list"
        );

        app.handle_key(KeyCode::Enter);
        assert!(
            !app.filter_mode,
            "Enter must confirm and exit filter_mode, keeping the filter text active"
        );
        assert_eq!(
            app.visible_indices().len(),
            1,
            "list must stay filtered right after Enter confirms it"
        );

        app.handle_key(KeyCode::Esc);

        assert_eq!(
            app.filter_text, "",
            "Esc from the table screen must clear a confirmed-but-still-active filter"
        );
        assert_eq!(
            app.visible_indices().len(),
            full_count,
            "the full list must be visible again after clearing the filter"
        );
    }

    #[test]
    fn should_do_nothing_on_escape_from_table_screen_when_filter_is_already_empty() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        assert_eq!(app.filter_text, "");
        assert_eq!(app.handle_key(KeyCode::Esc), None);
        assert_eq!(app.screen, Screen::Table);
        assert_eq!(app.filter_text, "");
    }
}
