mod app;
mod auth_setup;
mod cli;
mod config;
mod docker_build;
mod prepare;
mod scan;
mod table;
mod token;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use pisi_bump_bot::report_model::PackageReport;
use pisi_bump_common::load_recipes;

use app::{Action, App, Screen};
use auth_setup::{AuthChoice, AuthSetupState};
use cli::Cli;
use config::{AuthMethod, CachedPackage, Config};

fn main() {
    let cli = Cli::parse();

    let load = match load_recipes(&cli.recipes_dir) {
        Ok(load) => load,
        Err(error) => {
            eprintln!("hata: {error}");
            std::process::exit(1);
        }
    };

    let config_path = config::config_path();
    let mut config = config_path.as_deref().map(config::load).unwrap_or_default();

    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("terminal başlatılamadı: {error}");
            std::process::exit(1);
        }
    };

    let (token, authenticated) = match resolve_auth(
        &mut terminal,
        cli.token,
        &mut config,
        config_path.as_deref(),
        cli.reauth,
    ) {
        Ok(Some(resolved)) => resolved,
        Ok(None) => {
            ratatui::restore();
            return;
        }
        Err(error) => {
            ratatui::restore();
            eprintln!("hata: {error}");
            std::process::exit(1);
        }
    };

    let recipes = load.recipes;
    let recipes_dir = cli.recipes_dir;

    let mut app = App::new(
        recipes_dir.clone(),
        &recipes,
        &load.unreadable_paths,
        authenticated,
    );
    app.apply_cache(&recipes, &config.packages);

    let (tx, rx) = mpsc::channel();
    if app.scanning {
        scan::spawn_scan(tx.clone(), recipes.clone(), token.clone());
    }

    let mut context = EventLoopContext {
        tx,
        rx,
        recipes,
        token,
        config,
        config_path,
    };
    let outcome = run_event_loop(&mut terminal, &mut app, &mut context);

    ratatui::restore();

    if let Err(error) = outcome {
        eprintln!("hata: {error}");
        std::process::exit(1);
    }
}

fn should_skip_auth_screen(config_has_auth: bool, force_screen: bool) -> bool {
    config_has_auth && !force_screen
}

fn resolve_choice(choice: AuthChoice) -> (AuthMethod, Option<String>) {
    match choice {
        AuthChoice::GhAuth => (AuthMethod::GhAuth, token::gh_auth_token()),
        AuthChoice::Manual(value) => {
            let resolved = token::non_empty(Some(value.clone()));
            (AuthMethod::Manual { token: value }, resolved)
        }
    }
}

fn resolve_auth(
    terminal: &mut ratatui::DefaultTerminal,
    cli_token: Option<String>,
    config: &mut Config,
    config_path: Option<&Path>,
    force_screen: bool,
) -> std::io::Result<Option<(Option<String>, bool)>> {
    if let Some(flag_token) = token::non_empty(cli_token) {
        return Ok(Some((Some(flag_token), true)));
    }

    if let Some(env_token) = token::non_empty(std::env::var("GITHUB_TOKEN").ok()) {
        return Ok(Some((Some(env_token), true)));
    }

    if should_skip_auth_screen(config.auth.is_some(), force_screen) {
        let auth = config
            .auth
            .clone()
            .expect("checked by should_skip_auth_screen");
        let resolved_token = match &auth {
            AuthMethod::GhAuth => token::gh_auth_token(),
            AuthMethod::Manual { token } => token::non_empty(Some(token.clone())),
        };
        let authenticated = resolved_token.is_some();
        return Ok(Some((resolved_token, authenticated)));
    }

    let Some(choice) = run_auth_setup(terminal)? else {
        return Ok(None);
    };

    let (auth_method, resolved_token) = resolve_choice(choice);
    config.auth = Some(auth_method);
    if let Some(path) = config_path {
        let _ = config::save(path, config);
    }
    let authenticated = resolved_token.is_some();
    Ok(Some((resolved_token, authenticated)))
}

fn run_auth_setup(terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<Option<AuthChoice>> {
    let mut state = AuthSetupState::new();
    loop {
        terminal.draw(|frame| ui::draw_auth_setup(frame, &state))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key_event) = event::read()? else {
            continue;
        };
        if key_event.kind != KeyEventKind::Press {
            continue;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
        {
            return Ok(None);
        }
        if let Some(choice) = state.handle_key(key_event.code) {
            return Ok(Some(choice));
        }
    }
}

fn cache_report(config: &mut Config, report: &PackageReport, written: bool) {
    let already_written = config
        .packages
        .get(&report.recipe_path)
        .map(|cached| cached.written)
        .unwrap_or(false);
    config.packages.insert(
        report.recipe_path.clone(),
        CachedPackage {
            report: report.clone(),
            checked_at: config::now_iso(),
            written: written || already_written,
        },
    );
}

struct EventLoopContext {
    tx: mpsc::Sender<app::WorkerMessage>,
    rx: mpsc::Receiver<app::WorkerMessage>,
    recipes: Vec<pisi_bump_common::PackageRecipe>,
    token: Option<String>,
    config: Config,
    config_path: Option<PathBuf>,
}

impl EventLoopContext {
    fn persist(&self) {
        if let Some(path) = &self.config_path {
            let _ = config::save(path, &self.config);
        }
    }

    fn drain_messages(&mut self, app: &mut App) {
        while let Ok(message) = self.rx.try_recv() {
            match &message {
                app::WorkerMessage::ScanProgress { report, .. } => {
                    cache_report(&mut self.config, report, false);
                }
                app::WorkerMessage::ScanComplete => self.persist(),
                app::WorkerMessage::SingleCheckComplete { report, .. } => {
                    cache_report(&mut self.config, report, false);
                    self.persist();
                }
                _ => {}
            }
            let written_row_index = match &message {
                app::WorkerMessage::WriteComplete { row_index } => Some(*row_index),
                _ => None,
            };
            app.apply(message);
            if let Some(row_index) = written_row_index
                && let Some(report) = app.rows.get(row_index).and_then(|row| row.report.clone())
            {
                cache_report(&mut self.config, &report, true);
            }
        }
    }

    fn dispatch(&self, action: Action, app: &mut App) {
        match action {
            Action::Quit => {}
            Action::Rescan => {
                app.reset_for_rescan();
                scan::spawn_scan(self.tx.clone(), self.recipes.clone(), self.token.clone());
            }
            Action::CheckOne(row_index) => {
                if let Some(recipe) = self.recipes.get(row_index) {
                    scan::spawn_single_check(
                        self.tx.clone(),
                        row_index,
                        recipe.clone(),
                        self.token.clone(),
                    );
                }
            }
            Action::StartPrepare(row_index) => {
                if let Some(report) = app.rows[row_index].report.clone() {
                    let old_archive_url = self
                        .recipes
                        .get(row_index)
                        .and_then(|recipe| recipe.archive_urls.first().cloned());
                    prepare::spawn_prepare(
                        self.tx.clone(),
                        row_index,
                        app.recipes_dir.clone(),
                        report,
                        old_archive_url,
                    );
                }
            }
            Action::ConfirmWrite(row_index) => {
                if let app::PrepareState::Ready { new_pspec_text, .. } =
                    app.rows[row_index].prepare.clone()
                {
                    let pspec_path = app.recipes_dir.join(&app.rows[row_index].recipe_path);
                    prepare::spawn_write(self.tx.clone(), row_index, pspec_path, new_pspec_text);
                } else {
                    app.screen = Screen::Detail;
                    app.write_in_progress = false;
                }
            }
            Action::StartBuild(row_index) => {
                let recipe_path = app.rows[row_index].recipe_path.clone();
                docker_build::spawn_build(
                    self.tx.clone(),
                    row_index,
                    app.recipes_dir.clone(),
                    recipe_path,
                );
            }
            Action::RetrySudoBuild(row_index, password) => {
                let recipe_path = app.rows[row_index].recipe_path.clone();
                docker_build::spawn_sudo_build(
                    self.tx.clone(),
                    row_index,
                    app.recipes_dir.clone(),
                    recipe_path,
                    password,
                );
            }
            Action::Reauth => {}
        }
    }
}

fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    context: &mut EventLoopContext,
) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        app.advance_spinner();

        context.drain_messages(app);

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let Event::Key(key_event) = event::read()? else {
            continue;
        };
        if key_event.kind != KeyEventKind::Press {
            continue;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
        {
            return Ok(());
        }

        let Some(action) = app.handle_key(key_event.code) else {
            continue;
        };
        if action == Action::Quit {
            return Ok(());
        }
        if action == Action::Reauth {
            handle_reauth(terminal, app, context)?;
            continue;
        }
        context.dispatch(action, app);
    }
}

fn handle_reauth(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    context: &mut EventLoopContext,
) -> std::io::Result<()> {
    let Some(choice) = run_auth_setup(terminal)? else {
        return Ok(());
    };
    let (auth_method, resolved_token) = resolve_choice(choice);
    context.config.auth = Some(auth_method);
    context.persist();
    context.token = resolved_token.clone();
    app.authenticated = resolved_token.is_some();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pisi_bump_bot::report_model::{PackageReport, Status};
    use std::collections::HashMap;

    fn build_placeholder_credential(label: &str) -> String {
        ["placeholder", "credential", label].join("-")
    }

    #[test]
    fn should_skip_screen_when_config_already_has_auth_and_reauth_not_forced() {
        assert!(should_skip_auth_screen(true, false));
    }

    #[test]
    fn should_force_screen_when_reauth_flag_is_set_even_with_existing_auth() {
        assert!(!should_skip_auth_screen(true, true));
    }

    #[test]
    fn should_show_screen_when_no_auth_is_configured_yet() {
        assert!(!should_skip_auth_screen(false, false));
    }

    #[test]
    fn should_map_manual_choice_to_manual_method_with_matching_value() {
        let entered_value = build_placeholder_credential("entered");
        let (method, resolved) = resolve_choice(AuthChoice::Manual(entered_value.clone()));
        assert_eq!(
            method,
            AuthMethod::Manual {
                token: entered_value.clone()
            }
        );
        assert_eq!(resolved, Some(entered_value));
    }

    #[test]
    fn should_map_gh_auth_choice_to_gh_auth_method() {
        let (method, _resolved) = resolve_choice(AuthChoice::GhAuth);
        assert_eq!(method, AuthMethod::GhAuth);
    }

    #[test]
    fn should_keep_packages_cache_when_overwriting_auth_section_only() {
        let mut packages = HashMap::new();
        packages.insert(
            "network/browser/brave/pspec.xml".to_string(),
            CachedPackage {
                report: PackageReport::new("brave-browser", Status::Outdated, "1.0"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: false,
            },
        );
        let mut config = Config {
            auth: Some(AuthMethod::GhAuth),
            packages,
        };

        let replacement_value = build_placeholder_credential("replacement");
        let (new_method, _resolved) = resolve_choice(AuthChoice::Manual(replacement_value.clone()));
        config.auth = Some(new_method);

        assert_eq!(
            config.auth,
            Some(AuthMethod::Manual {
                token: replacement_value
            })
        );
        assert_eq!(
            config.packages.len(),
            1,
            "switching auth method must not clear the cached scan results"
        );
        assert!(
            config
                .packages
                .contains_key("network/browser/brave/pspec.xml")
        );
    }
}
