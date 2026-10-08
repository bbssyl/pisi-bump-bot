use std::path::Path;

use pisi_bump_common::{RecipeLoad, load_recipes};

use crate::cli::args::ReportArgs;
use crate::cli::runtime::Runtime;
use crate::github_upstream::GithubLookup;
use crate::index_consistency::check_index_consistency;
use crate::new_updates::{load_previous_outdated, package_key, render_new_updates};
use crate::package_checker::PackageChecker;
use crate::report_model::{PackageReport, Report, Status};
use crate::report_writer::{render_console, render_json, render_markdown, sort_packages};

const UNREADABLE_DETAIL: &str = "pspec.xml okunamadı";

fn write_text(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

fn unreadable_report(recipe_path: &str) -> PackageReport {
    let name = Path::new(recipe_path)
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(recipe_path);
    PackageReport::new(name, Status::Error, "")
        .with_detail(Some(UNREADABLE_DETAIL.to_string()))
        .with_recipe_path(recipe_path)
}

fn check_packages(load: &RecipeLoad, runtime: Runtime, with_hash: bool) -> Vec<PackageReport> {
    let token = runtime.token.clone();
    let hash_archive = runtime.hash_archive;
    let lookup = GithubLookup::new(runtime.fetch, token);
    let hash_ref = if with_hash {
        Some(hash_archive.as_ref())
    } else {
        None
    };
    let checker = PackageChecker::new(&lookup, hash_ref);
    let mut reports: Vec<PackageReport> = load
        .recipes
        .iter()
        .map(|recipe| checker.check(recipe))
        .collect();
    reports.extend(
        load.unreadable_paths
            .iter()
            .map(|path| unreadable_report(path)),
    );
    sort_packages(reports)
}

fn build_report(
    args: &ReportArgs,
    runtime: Runtime,
) -> Result<Report, pisi_bump_common::RecipeError> {
    let load = load_recipes(&args.recipes_dir)?;
    let packages = check_packages(&load, runtime, args.with_hash);
    let consistency = check_index_consistency(&args.recipes_dir, &load.recipes);
    Ok(Report::new(
        args.source_commit.clone(),
        packages,
        consistency,
    ))
}

fn write_new_updates(args: &ReportArgs, report: &Report) {
    let Some(new_updates_path) = &args.new_updates_path else {
        return;
    };
    let previous = args
        .previous_path
        .as_deref()
        .and_then(load_previous_outdated);
    let previous = previous.unwrap_or_else(|| {
        eprintln!("önceki rapor yok, yeni güncelleme listesi boş");
        report
            .packages
            .iter()
            .map(|package| {
                (
                    package_key(&package.name, &package.recipe_path),
                    package.latest_version.clone(),
                )
            })
            .collect()
    });
    let _ = write_text(new_updates_path, &render_new_updates(report, &previous));
}

pub fn run(args: ReportArgs, runtime: Runtime) -> i32 {
    let report = match build_report(&args, runtime) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("tarifler okunamadı: {error}");
            return 1;
        }
    };
    if write_text(&args.json_path, &render_json(&report)).is_err()
        || write_text(&args.markdown_path, &render_markdown(&report, None)).is_err()
    {
        eprintln!("report başarısız: rapor dosyaları yazılamadı");
        return 1;
    }
    write_new_updates(&args, &report);
    println!("{}", render_console(&report));
    0
}
