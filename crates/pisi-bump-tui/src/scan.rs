use std::sync::mpsc::Sender;
use std::thread;

use pisi_bump_bot::github_upstream::GithubLookup;
use pisi_bump_bot::http_client::{HttpResponse, fetch};
use pisi_bump_bot::package_checker::PackageChecker;
use pisi_bump_common::PackageRecipe;

use crate::app::WorkerMessage;

fn fetch_observing_rate_limit(
    tx: Sender<WorkerMessage>,
) -> impl Fn(&str, &[(String, String)]) -> Result<HttpResponse, pisi_bump_bot::error::FetchError> {
    move |url, headers| {
        let response = fetch(url, headers)?;
        if let Some(remaining) = response
            .header("x-ratelimit-remaining")
            .and_then(|value| value.parse::<u32>().ok())
        {
            let _ = tx.send(WorkerMessage::RateLimitUpdate { remaining });
        }
        Ok(response)
    }
}

pub fn spawn_scan(tx: Sender<WorkerMessage>, recipes: Vec<PackageRecipe>, token: Option<String>) {
    thread::spawn(move || {
        let rate_tx = tx.clone();
        let lookup = GithubLookup::new(Box::new(fetch_observing_rate_limit(rate_tx)), token);
        let checker = PackageChecker::new(&lookup, None);
        let total = recipes.len();
        for (index, recipe) in recipes.iter().enumerate() {
            let report = checker.check(recipe);
            let sent = tx.send(WorkerMessage::ScanProgress {
                index,
                done: index + 1,
                total,
                report: Box::new(report),
            });
            if sent.is_err() {
                return;
            }
        }
        let _ = tx.send(WorkerMessage::ScanComplete);
    });
}

pub fn spawn_single_check(
    tx: Sender<WorkerMessage>,
    row_index: usize,
    recipe: PackageRecipe,
    token: Option<String>,
) {
    thread::spawn(move || {
        let rate_tx = tx.clone();
        let lookup = GithubLookup::new(Box::new(fetch_observing_rate_limit(rate_tx)), token);
        let checker = PackageChecker::new(&lookup, None);
        let report = checker.check(&recipe);
        let _ = tx.send(WorkerMessage::SingleCheckComplete {
            row_index,
            report: Box::new(report),
        });
    });
}
