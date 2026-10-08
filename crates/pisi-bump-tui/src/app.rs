use std::path::PathBuf;

use crossterm::event::KeyCode;
use pisi_bump_bot::report_model::{PackageReport, Status};
use pisi_bump_common::PackageRecipe;

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
pub struct RowState {
    pub name: String,
    pub recipe_path: String,
    pub scannable: bool,
    pub report: Option<PackageReport>,
    pub prepare: PrepareState,
    pub written: bool,
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
        }
    }

    pub fn status_text(&self) -> &'static str {
        match &self.report {
            None => "taranıyor",
            Some(report) => report.status.as_str(),
        }
    }
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
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        crate::table::visible_indices(&self.rows, self.show_all_statuses, &self.filter_text)
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
                }
                self.status_message = Some(format!(
                    "pspec.xml güncellendi ({})",
                    self.rows
                        .get(row_index)
                        .map(|row| row.recipe_path.as_str())
                        .unwrap_or("pspec.xml")
                ));
                self.screen = Screen::Table;
                self.detail_index = None;
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
        }
    }

    pub fn reset_for_rescan(&mut self) {
        for row in self.rows.iter_mut() {
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
            KeyCode::Char('r') => {
                if !self.scanning && !self.write_in_progress {
                    return Some(Action::Rescan);
                }
            }
            KeyCode::Enter => {
                if let Some(&absolute) = visible.get(self.selected) {
                    let is_outdated = self
                        .rows
                        .get(absolute)
                        .and_then(|row| row.report.as_ref())
                        .map(|report| report.status == Status::Outdated)
                        .unwrap_or(false);
                    if is_outdated {
                        self.detail_index = Some(absolute);
                        self.detail_scroll = 0;
                        self.status_message = None;
                        self.screen = Screen::Detail;
                    }
                }
            }
            _ => {}
        }
        None
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
            KeyCode::Up => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(10);
            }
            _ => {}
        }
        None
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
    fn should_mark_row_written_and_return_to_table_when_write_completes() {
        let recipes = vec![recipe("brave-browser", "network/browser/brave/pspec.xml")];
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &recipes, &[], true);
        app.screen = Screen::Confirm;
        app.detail_index = Some(0);
        app.write_in_progress = true;
        app.apply(WorkerMessage::WriteComplete { row_index: 0 });
        assert!(app.rows[0].written);
        assert!(!app.write_in_progress);
        assert_eq!(app.screen, Screen::Table);
        assert_eq!(app.detail_index, None);
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
    fn should_enter_detail_only_for_outdated_selection() {
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
        assert_eq!(app.screen, Screen::Table);

        app.selected = 1;
        assert_eq!(app.handle_key(KeyCode::Enter), None);
        assert_eq!(app.screen, Screen::Detail);
        assert_eq!(app.detail_index, Some(1));
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
    fn should_clear_filter_text_on_escape() {
        let mut app = App::new(PathBuf::from("/tmp/contrib"), &[], &[], true);
        app.filter_mode = true;
        app.filter_text = "bra".to_string();
        app.handle_key(KeyCode::Esc);
        assert!(!app.filter_mode);
        assert_eq!(app.filter_text, "");
    }
}
