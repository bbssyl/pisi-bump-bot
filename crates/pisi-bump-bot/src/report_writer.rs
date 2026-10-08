use serde::Serialize;

use crate::build_columns::BuildLinks;
use crate::index_consistency::IndexConsistency;
use crate::report_model::{PackageReport, Report, Status};

const EMPTY_CELL: &str = "-";
const SOURCE_URL: &str = "https://git.pisilinux.org/Pisilinux/contrib";

const COLLAPSED_SECTIONS: [(Status, &str); 4] = [
    (Status::Current, "Güncel paketler"),
    (Status::Uncomparable, "Karşılaştırılamayan paketler"),
    (Status::Unsupported, "Desteklenmeyen paketler"),
    (Status::Error, "Hata veren paketler"),
];

pub fn sort_packages(mut packages: Vec<PackageReport>) -> Vec<PackageReport> {
    packages.sort_by(|left, right| {
        let left_key = (
            left.name.to_lowercase(),
            left.name.clone(),
            left.recipe_path.clone(),
        );
        let right_key = (
            right.name.to_lowercase(),
            right.name.clone(),
            right.recipe_path.clone(),
        );
        left_key.cmp(&right_key)
    });
    packages
}

#[derive(Debug, Clone, Serialize)]
struct Summary {
    toplam: usize,
    eski: usize,
    guncel: usize,
    desteklenmiyor: usize,
    karsilastirilamadi: usize,
    hata: usize,
    index_te_olmayan: usize,
    index_surum_farki: usize,
}

fn summary_counts(report: &Report) -> Summary {
    Summary {
        toplam: report.packages.len(),
        eski: report.count(Status::Outdated),
        guncel: report.count(Status::Current),
        desteklenmiyor: report.count(Status::Unsupported),
        karsilastirilamadi: report.count(Status::Uncomparable),
        hata: report.count(Status::Error),
        index_te_olmayan: report.index_consistency.missing_from_index.len(),
        index_surum_farki: report.index_consistency.version_mismatches.len(),
    }
}

#[derive(Debug, Serialize)]
struct ReportDocument<'a> {
    source_url: &'a str,
    source_commit: &'a Option<String>,
    summary: Summary,
    index_consistency: &'a IndexConsistency,
    packages: &'a [PackageReport],
}

pub fn render_json(report: &Report) -> String {
    let document = ReportDocument {
        source_url: SOURCE_URL,
        source_commit: &report.source_commit,
        summary: summary_counts(report),
        index_consistency: &report.index_consistency,
        packages: &report.packages,
    };
    let mut rendered = serde_json::to_string_pretty(&document).expect("report must serialize");
    rendered.push('\n');
    rendered
}

fn escape_cell(value: Option<&str>) -> String {
    match value {
        None => EMPTY_CELL.to_string(),
        Some("") => EMPTY_CELL.to_string(),
        Some(value) => value.replace('|', "\\|").replace('\n', " "),
    }
}

fn table(headers: &[&str], rows: &[Vec<Option<String>>]) -> Vec<String> {
    let mut lines = vec![
        format!("| {} |", headers.join(" | ")),
        format!(
            "|{}|",
            headers
                .iter()
                .map(|_| " --- ")
                .collect::<Vec<_>>()
                .join("|")
        ),
    ];
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| escape_cell(cell.as_deref()))
            .collect();
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    lines
}

fn link(label: Option<&str>, url: Option<&str>) -> Option<String> {
    match (label, url) {
        (Some(label), Some(url)) if !label.is_empty() && !url.is_empty() => {
            Some(format!("[{label}]({url})"))
        }
        _ => label.map(str::to_string),
    }
}

fn code(value: Option<&str>) -> Option<String> {
    value.filter(|v| !v.is_empty()).map(|v| format!("`{v}`"))
}

fn outdated_row(package: &PackageReport, links: Option<&BuildLinks>) -> Vec<Option<String>> {
    let mut row = vec![
        Some(package.name.clone()),
        code(Some(&package.recipe_path)),
        Some(package.current_version.clone()),
        package.latest_version.clone(),
        link(package.upstream.as_deref(), package.release_url.as_deref()),
        code(package.candidate_url.as_deref()),
    ];
    if let Some(links) = links {
        let version = package.latest_version.as_deref();
        row.push(links.prepared_cell(&package.recipe_path, version));
        row.push(links.build_cell(&package.recipe_path, version));
    }
    row
}

fn outdated_table(packages: &[&PackageReport], links: Option<&BuildLinks>) -> Vec<String> {
    if packages.is_empty() {
        return vec!["Eski paket yok.".to_string()];
    }
    let mut headers = vec![
        "Paket",
        "Pspec",
        "Mevcut",
        "Yeni",
        "Kaynak",
        "Aday arşiv URL",
    ];
    if links.is_some() {
        headers.push("Hazır pspec");
        headers.push("Derleme");
    }
    let rows: Vec<Vec<Option<String>>> = packages
        .iter()
        .map(|package| outdated_row(package, links))
        .collect();
    table(&headers, &rows)
}

fn generic_table(packages: &[&PackageReport]) -> Vec<String> {
    let rows: Vec<Vec<Option<String>>> = packages
        .iter()
        .map(|package| {
            vec![
                Some(package.name.clone()),
                code(Some(&package.recipe_path)),
                Some(package.current_version.clone()),
                package.latest_version.clone(),
                package.upstream.clone(),
                package.detail.clone(),
            ]
        })
        .collect();
    table(
        &["Paket", "Pspec", "Mevcut", "Yeni", "Kaynak", "Ayrıntı"],
        &rows,
    )
}

fn details_block(title: &str, count: usize, body: Vec<String>) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        "<details>".to_string(),
        format!("<summary>{title} ({count})</summary>"),
        String::new(),
    ];
    lines.extend(body);
    lines.push(String::new());
    lines.push("</details>".to_string());
    lines
}

fn summary_lines(report: &Report) -> Vec<String> {
    let consistency = &report.index_consistency;
    let labels: [(&str, usize); 8] = [
        ("Toplam paket", report.packages.len()),
        ("Eski", report.count(Status::Outdated)),
        ("Güncel", report.count(Status::Current)),
        ("Desteklenmiyor", report.count(Status::Unsupported)),
        ("Karşılaştırılamadı", report.count(Status::Uncomparable)),
        ("Hata", report.count(Status::Error)),
        ("Index'te olmayan", consistency.missing_from_index.len()),
        ("Index sürüm farkı", consistency.version_mismatches.len()),
    ];
    labels
        .iter()
        .map(|(label, value)| format!("- {label}: {value}"))
        .collect()
}

fn consistency_body(report: &Report) -> Vec<String> {
    let consistency = &report.index_consistency;
    if !consistency.found {
        return vec!["Depoda pisi-index.xml.xz veya pisi-index.xml bulunamadı.".to_string()];
    }
    if let Some(error) = &consistency.error {
        return vec![error.clone()];
    }
    let mut body = vec![
        format!(
            "Index'te olmayan paketler ({}):",
            consistency.missing_from_index.len()
        ),
        String::new(),
    ];
    body.extend(
        consistency
            .missing_from_index
            .iter()
            .map(|path| format!("- `{path}`")),
    );
    body.push(String::new());
    body.push(format!(
        "Pspec sürümü index sürümünden farklı olanlar ({}):",
        consistency.version_mismatches.len()
    ));
    body.push(String::new());
    let rows: Vec<Vec<Option<String>>> = consistency
        .version_mismatches
        .iter()
        .map(|mismatch| {
            vec![
                Some(mismatch.name.clone()),
                code(Some(&mismatch.recipe_path)),
                Some(mismatch.pspec_version.clone()),
                Some(mismatch.index_version.clone()),
            ]
        })
        .collect();
    body.extend(table(
        &["Paket", "Pspec", "Pspec sürümü", "Index sürümü"],
        &rows,
    ));
    body
}

fn consistency_block(report: &Report) -> Vec<String> {
    let consistency = &report.index_consistency;
    let count = consistency.missing_from_index.len() + consistency.version_mismatches.len();
    details_block("Index tutarsızlıkları", count, consistency_body(report))
}

fn source_line(report: &Report) -> String {
    let commit = match &report.source_commit {
        Some(commit) => format!(", commit `{commit}`"),
        None => String::new(),
    };
    format!("Kaynak: {SOURCE_URL} (pspec.xml dosyaları{commit})")
}

pub fn render_markdown(report: &Report, links: Option<&BuildLinks>) -> String {
    let mut lines = vec![
        "# Pisi Linux contrib güncellik raporu".to_string(),
        String::new(),
        source_line(report),
        String::new(),
    ];
    lines.push("## Özet".to_string());
    lines.push(String::new());
    lines.extend(summary_lines(report));
    lines.push(String::new());
    lines.push("## Eski paketler".to_string());
    lines.push(String::new());
    lines.extend(outdated_table(&report.with_status(Status::Outdated), links));
    for (status, title) in COLLAPSED_SECTIONS {
        let packages = report.with_status(status);
        if !packages.is_empty() {
            lines.extend(details_block(
                title,
                packages.len(),
                generic_table(&packages),
            ));
        }
    }
    lines.extend(consistency_block(report));
    lines.join("\n") + "\n"
}

pub fn render_console(report: &Report) -> String {
    let consistency = &report.index_consistency;
    let mut lines = vec![format!("Toplam: {}", report.packages.len())];
    for status in Status::ALL {
        lines.push(format!("{}: {}", status.as_str(), report.count(status)));
    }
    lines.push(format!(
        "index'te olmayan: {}",
        consistency.missing_from_index.len()
    ));
    lines.push(format!(
        "index sürüm farkı: {}",
        consistency.version_mismatches.len()
    ));
    lines.push(String::new());
    for package in report.with_status(Status::Outdated) {
        lines.push(format!(
            "{} [{}]: {} -> {}",
            package.name,
            package.recipe_path,
            package.current_version,
            package.latest_version.as_deref().unwrap_or_default()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_state::{BuildEntry, EntryStatus, State};

    const WEB: &str = "https://github.com/o/r";

    pub(crate) fn sample_report(latest: &str) -> Report {
        let beta = PackageReport::new("beta", Status::Current, "1.0")
            .with_latest_version("v1.0")
            .with_upstream("o/beta")
            .with_release_url("https://r/beta");
        let alpha = PackageReport::new("alpha", Status::Outdated, "1.0")
            .with_latest_version(latest)
            .with_upstream("o/alpha")
            .with_release_url("https://r/alpha")
            .with_candidate_url(Some("https://c/alpha".to_string()));
        let gamma = PackageReport::new("gamma", Status::Unsupported, "1.0")
            .with_detail(Some("x|y".to_string()));
        let mut packages = vec![beta, alpha, gamma];
        packages.sort_by(|left, right| left.name.cmp(&right.name));
        let consistency = IndexConsistency {
            found: true,
            error: None,
            missing_from_index: vec!["zzz/pspec.xml".to_string()],
            version_mismatches: Vec::new(),
        };
        Report::new(Some("abc1234".to_string()), packages, consistency)
    }

    #[test]
    fn should_put_outdated_table_before_collapsed_sections() {
        let text = render_markdown(&sample_report("v2"), None);
        assert!(text.find("| alpha |").unwrap() < text.find("<details>").unwrap());
    }

    #[test]
    fn should_list_missing_packages_when_index_is_incomplete() {
        assert!(render_markdown(&sample_report("v2"), None).contains("`zzz/pspec.xml`"));
    }

    #[test]
    fn should_escape_pipe_characters_in_cells() {
        assert!(render_markdown(&sample_report("v2"), None).contains("x\\|y"));
    }

    #[test]
    fn should_render_identical_output_when_report_is_unchanged() {
        assert_eq!(
            render_json(&sample_report("v2")),
            render_json(&sample_report("v2"))
        );
    }

    #[test]
    fn should_report_index_failure_when_error_is_set() {
        let report = Report::new(
            None,
            Vec::new(),
            IndexConsistency {
                found: true,
                error: Some("index okunamadı: x".to_string()),
                missing_from_index: Vec::new(),
                version_mismatches: Vec::new(),
            },
        );
        assert!(render_markdown(&report, None).contains("index okunamadı: x"));
    }

    #[test]
    fn should_show_source_commit_in_header() {
        assert!(render_markdown(&sample_report("v2"), None).contains("commit `abc1234`"));
    }

    fn report_with_recipe_path() -> Report {
        let package = PackageReport::new("alpha", Status::Outdated, "1.0")
            .with_latest_version("v2")
            .with_upstream("o/alpha")
            .with_recipe_path("alpha/pspec.xml");
        Report::new(
            Some("abc".to_string()),
            vec![package],
            IndexConsistency {
                found: true,
                error: None,
                missing_from_index: Vec::new(),
                version_mismatches: Vec::new(),
            },
        )
    }

    fn entry(status: EntryStatus) -> BuildEntry {
        BuildEntry::new("v2", status, "2026-10-07")
    }

    fn render_with_links(state: State, relative_files: bool) -> String {
        let links = BuildLinks::new(state, Some(WEB.to_string()), relative_files);
        render_markdown(&report_with_recipe_path(), Some(&links))
    }

    #[test]
    fn should_show_pending_and_diff_link_when_prepared() {
        let mut state = State::new();
        state.insert("alpha".to_string(), entry(EntryStatus::Prepared));
        let text = render_with_links(state, false);
        assert!(text.contains(&format!(
            "[pspec.diff]({WEB}/blob/HEAD/hazir/alpha/pspec.diff) | bekliyor |"
        )));
    }

    #[test]
    fn should_use_relative_diff_link_when_requested() {
        let mut state = State::new();
        state.insert("alpha".to_string(), entry(EntryStatus::Prepared));
        let text = render_with_links(state, true);
        assert!(text.contains("[pspec.diff](hazir/alpha/pspec.diff)"));
    }

    #[test]
    fn should_show_reason_when_preparation_failed() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            entry(EntryStatus::PrepareFailed).with_reason("indirme başarısız"),
        );
        let text = render_with_links(state, false);
        assert!(text.contains("otomatik hazırlanamadı: indirme başarısız"));
    }

    #[test]
    fn should_link_run_when_build_failed() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            entry(EntryStatus::BuildFailed).with_run_id("77"),
        );
        let text = render_with_links(state, false);
        assert!(text.contains(&format!("[❌]({WEB}/actions/runs/77)")));
    }

    #[test]
    fn should_ignore_entry_when_version_differs() {
        let mut state = State::new();
        state.insert(
            "alpha".to_string(),
            BuildEntry::new("v1", EntryStatus::Built, "d").with_run_id("1"),
        );
        let text = render_with_links(state, false);
        assert!(!text.contains('✅'));
    }

    #[test]
    fn should_keep_old_columns_when_no_links_given() {
        assert!(!render_markdown(&sample_report("v2"), None).contains("Derleme"));
    }

    #[test]
    fn should_sort_packages_case_insensitively_then_by_recipe_path() {
        let packages = vec![
            PackageReport::new("Zeta", Status::Current, "1").with_recipe_path("z/pspec.xml"),
            PackageReport::new("alpha", Status::Current, "1").with_recipe_path("a/pspec.xml"),
            PackageReport::new("Alpha", Status::Current, "1").with_recipe_path("b/pspec.xml"),
        ];
        let sorted = sort_packages(packages);
        let names: Vec<&str> = sorted.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "alpha", "Zeta"]);
    }
}
