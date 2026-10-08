use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::index_consistency::IndexConsistency;
use crate::report_model::{PackageReport, Report};

#[derive(Debug, thiserror::Error)]
pub enum ReportLoadError {
    #[error("rapor okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("rapor okunamadı: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct ReportDocument {
    source_commit: Option<String>,
    packages: Vec<PackageReport>,
    index_consistency: IndexConsistency,
}

pub fn load_report(path: &Path) -> Result<Report, ReportLoadError> {
    let text = fs::read_to_string(path)?;
    let document: ReportDocument = serde_json::from_str(&text)?;
    Ok(Report::new(
        document.source_commit,
        document.packages,
        document.index_consistency,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report_model::Status;
    use crate::report_writer::render_json;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pisi-bump-bot-report-loader-{name}-{}.json",
            std::process::id()
        ));
        path
    }

    #[test]
    fn should_round_trip_report_through_json() {
        let package =
            PackageReport::new("alpha", Status::Outdated, "1.0").with_recipe_path("a/pspec.xml");
        let report = Report::new(
            Some("abc1234".to_string()),
            vec![package],
            IndexConsistency {
                found: true,
                error: None,
                missing_from_index: vec!["zzz/pspec.xml".to_string()],
                version_mismatches: Vec::new(),
            },
        );
        let path = temp_path("roundtrip");
        fs::write(&path, render_json(&report)).unwrap();
        let loaded = load_report(&path).unwrap();
        assert_eq!(render_json(&loaded), render_json(&report));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_fail_when_file_is_missing() {
        assert!(load_report(Path::new("/nonexistent/report.json")).is_err());
    }
}
