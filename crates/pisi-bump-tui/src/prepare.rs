use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use pisi_bump_bot::archive_download::download_sha1;
use pisi_bump_bot::pspec_updater::{UpdateRequest, prepare_update};
use pisi_bump_bot::report_model::PackageReport;

use crate::app::WorkerMessage;

fn today() -> String {
    let date = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

pub fn spawn_prepare(
    tx: Sender<WorkerMessage>,
    row_index: usize,
    recipes_dir: PathBuf,
    report: PackageReport,
    old_archive_url: Option<String>,
) {
    thread::spawn(move || {
        let message = match run_prepare(&recipes_dir, &report, old_archive_url.as_deref()) {
            Ok((diff, new_pspec_text)) => WorkerMessage::PrepareComplete {
                row_index,
                diff,
                new_pspec_text,
            },
            Err(reason) => WorkerMessage::PrepareFailed { row_index, reason },
        };
        let _ = tx.send(message);
    });
}

fn run_prepare(
    recipes_dir: &std::path::Path,
    report: &PackageReport,
    old_archive_url: Option<&str>,
) -> Result<(String, String), String> {
    let candidate_url = report
        .candidate_url
        .as_deref()
        .ok_or_else(|| "aday arşiv URL'si yok".to_string())?;
    let latest_version = report
        .latest_version
        .as_deref()
        .ok_or_else(|| "yeni sürüm bilgisi yok".to_string())?;

    let download = download_sha1(candidate_url).map_err(|error| error.message)?;

    let pspec_path = recipes_dir.join(&report.recipe_path);
    let pspec_text = fs::read_to_string(&pspec_path)
        .map_err(|error| format!("pspec.xml okunamadı ({}): {error}", pspec_path.display()))?;

    let run_date = today();
    let request = UpdateRequest {
        recipe_path: &report.recipe_path,
        pspec_text: &pspec_text,
        actions_text: None,
        new_version: latest_version,
        new_archive_url: candidate_url,
        new_sha1: &download.sha1,
        run_date: &run_date,
        old_archive_url,
    };
    let prepared = prepare_update(&request).map_err(|error| error.to_string())?;
    Ok((prepared.unified_diff, prepared.new_pspec_text))
}

pub fn spawn_write(
    tx: Sender<WorkerMessage>,
    row_index: usize,
    pspec_path: PathBuf,
    new_pspec_text: String,
) {
    thread::spawn(move || {
        let message = match fs::write(&pspec_path, new_pspec_text) {
            Ok(()) => WorkerMessage::WriteComplete { row_index },
            Err(error) => WorkerMessage::WriteFailed {
                row_index,
                reason: error.to_string(),
            },
        };
        let _ = tx.send(message);
    });
}
