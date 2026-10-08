mod app;
mod cli;
mod prepare;
mod scan;
mod table;
mod token;
mod ui;

use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind, KeyModifiers};
use pisi_bump_common::load_recipes;

use app::{Action, App, Screen};
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    let load = match load_recipes(&cli.recipes_dir) {
        Ok(load) => load,
        Err(error) => {
            eprintln!("hata: {error}");
            std::process::exit(1);
        }
    };

    let token = token::resolve_token(cli.token);
    let authenticated = token.is_some();
    let recipes = load.recipes;
    let recipes_dir = cli.recipes_dir;

    let mut app = App::new(
        recipes_dir.clone(),
        &recipes,
        &load.unreadable_paths,
        authenticated,
    );

    let (tx, rx) = mpsc::channel();
    scan::spawn_scan(tx.clone(), recipes.clone(), token.clone());

    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("terminal başlatılamadı: {error}");
            std::process::exit(1);
        }
    };

    let outcome = run_event_loop(&mut terminal, &mut app, &tx, &rx, &recipes, &token);

    ratatui::restore();

    if let Err(error) = outcome {
        eprintln!("hata: {error}");
        std::process::exit(1);
    }
}

fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &mpsc::Sender<app::WorkerMessage>,
    rx: &mpsc::Receiver<app::WorkerMessage>,
    recipes: &[pisi_bump_common::PackageRecipe],
    token: &Option<String>,
) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        while let Ok(message) = rx.try_recv() {
            app.apply(message);
        }

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
            && key_event.code == crossterm::event::KeyCode::Char('c')
        {
            return Ok(());
        }

        let Some(action) = app.handle_key(key_event.code) else {
            continue;
        };
        match action {
            Action::Quit => return Ok(()),
            Action::Rescan => {
                app.reset_for_rescan();
                scan::spawn_scan(tx.clone(), recipes.to_vec(), token.clone());
            }
            Action::StartPrepare(row_index) => {
                if let Some(report) = app.rows[row_index].report.clone() {
                    let old_archive_url = recipes
                        .get(row_index)
                        .and_then(|recipe| recipe.archive_urls.first().cloned());
                    prepare::spawn_prepare(
                        tx.clone(),
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
                    prepare::spawn_write(tx.clone(), row_index, pspec_path, new_pspec_text);
                } else {
                    app.screen = Screen::Detail;
                    app.write_in_progress = false;
                }
            }
        }
    }
}
