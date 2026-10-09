use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use pisi_bump_bot::archive_download::download_sha1;
use pisi_bump_bot::candidate_url::derive_new_version;
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

fn resolve_new_version(report: &PackageReport) -> Result<String, String> {
    let raw_tag = report
        .latest_version
        .as_deref()
        .ok_or_else(|| "yeni sürüm bilgisi yok".to_string())?;
    let upstream_repo = report
        .upstream
        .as_deref()
        .map(|upstream| upstream.rsplit('/').next().unwrap_or(""))
        .unwrap_or("");
    let prefixes = [upstream_repo, report.name.as_str()];
    derive_new_version(raw_tag, &prefixes, &report.current_version)
        .ok_or_else(|| "yeni sürüm etiketten okunamadı".to_string())
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
    let new_version = resolve_new_version(report)?;

    let download = download_sha1(candidate_url).map_err(|error| error.message)?;

    let pspec_path = recipes_dir.join(&report.recipe_path);
    let pspec_text = fs::read_to_string(&pspec_path)
        .map_err(|error| format!("pspec.xml okunamadı ({}): {error}", pspec_path.display()))?;

    let run_date = today();
    let request = UpdateRequest {
        recipe_path: &report.recipe_path,
        pspec_text: &pspec_text,
        actions_text: None,
        new_version: &new_version,
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

#[cfg(test)]
mod tests {
    use super::*;
    use pisi_bump_bot::report_model::Status;

    const MINIMAL_PSPEC: &str = r#"<?xml version="1.0" ?>
<!DOCTYPE PISI SYSTEM "https://pisilinux.org/projeler/pisi/pisi-spec.dtd">
<PISI>
    <Source>
        <Name>iptvnator</Name>
        <Archive sha1sum="0000000000000000000000000000000000000000" type="binary">https://github.com/4gray/iptvnator/releases/download/v0.15.0/iptvnator_0.15.0_amd64.deb</Archive>
    </Source>
    <Package>
        <Name>iptvnator</Name>
    </Package>
    <History>
        <Update release="1">
            <Date>2026-01-01</Date>
            <Version>0.15.0</Version>
            <Comment>First release</Comment>
            <Name>pisi-bump-bot</Name>
            <Email>pisi-bump-bot@users.noreply.github.com</Email>
        </Update>
    </History>
</PISI>
"#;

    fn report_with(name: &str, upstream: &str, current: &str, latest_tag: &str) -> PackageReport {
        PackageReport::new(name, Status::Outdated, current)
            .with_latest_version(latest_tag)
            .with_upstream(upstream)
    }

    #[test]
    fn should_strip_leading_v_from_the_real_iptvnator_tag_that_broke_every_build() {
        let report = report_with("iptvnator", "4gray/iptvnator", "0.15.0", "v0.24.0");
        assert_eq!(resolve_new_version(&report), Ok("0.24.0".to_string()));
    }

    #[test]
    fn should_still_handle_a_tag_without_a_v_prefix() {
        let report = report_with("atari800", "atari800/atari800", "4.2.0", "4.2.1");
        assert_eq!(resolve_new_version(&report), Ok("4.2.1".to_string()));
    }

    #[test]
    fn should_report_missing_latest_version_distinctly_from_an_unparseable_one() {
        let report = PackageReport::new("pkg", Status::Outdated, "1.0");
        assert_eq!(
            resolve_new_version(&report),
            Err("yeni sürüm bilgisi yok".to_string())
        );
    }

    #[test]
    fn should_fail_loudly_instead_of_silently_falling_back_to_an_unparseable_raw_tag() {
        let report = report_with("pkg", "o/r", "1.0", "");
        let result = resolve_new_version(&report);
        assert!(
            result.is_err(),
            "an empty/unparseable tag must be a hard failure, never the raw unstripped value"
        );
    }

    #[test]
    fn should_write_the_stripped_version_into_the_prepared_pspec_for_the_real_iptvnator_case() {
        let report = report_with("iptvnator", "4gray/iptvnator", "0.15.0", "v0.24.0")
            .with_candidate_url(Some(
                "https://github.com/4gray/iptvnator/releases/download/v0.24.0/iptvnator_0.24.0_amd64.deb"
                    .to_string(),
            ))
            .with_recipe_path("multimedia/tv/iptvnator/pspec.xml");

        let new_version = resolve_new_version(&report).expect("version must resolve");
        assert_eq!(new_version, "0.24.0");

        let request = UpdateRequest {
            recipe_path: &report.recipe_path,
            pspec_text: MINIMAL_PSPEC,
            actions_text: None,
            new_version: &new_version,
            new_archive_url: report.candidate_url.as_deref().unwrap(),
            new_sha1: "1111111111111111111111111111111111111111",
            run_date: "2026-10-08",
            old_archive_url: None,
        };
        let prepared =
            prepare_update(&request).expect("prepare_update should succeed on a well-formed pspec");

        assert!(
            prepared
                .new_pspec_text
                .contains("<Version>0.24.0</Version>"),
            "prepared pspec must contain the stripped version"
        );
        assert!(
            !prepared
                .new_pspec_text
                .contains("<Version>v0.24.0</Version>"),
            "prepared pspec must never contain a leading-v version string, PiSi's own parser rejects that"
        );
    }
}
