use pisi_bump_bot::report_model::Status;

use crate::app::RowState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub total: usize,
    pub outdated: usize,
    pub current: usize,
    pub unsupported: usize,
    pub uncomparable: usize,
    pub error: usize,
}

pub fn visible_indices(rows: &[RowState], show_all: bool, filter_text: &str) -> Vec<usize> {
    let needle = filter_text.to_lowercase();
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.report.is_none() || show_all || is_outdated(row))
        .filter(|(_, row)| needle.is_empty() || row.name.to_lowercase().contains(&needle))
        .map(|(index, _)| index)
        .collect()
}

fn is_outdated(row: &RowState) -> bool {
    row.report
        .as_ref()
        .map(|report| report.status == Status::Outdated)
        .unwrap_or(false)
}

pub fn count_statuses(rows: &[RowState]) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for row in rows {
        counts.total += 1;
        match row.report.as_ref().map(|report| report.status) {
            Some(Status::Outdated) => counts.outdated += 1,
            Some(Status::Current) => counts.current += 1,
            Some(Status::Unsupported) => counts.unsupported += 1,
            Some(Status::Uncomparable) => counts.uncomparable += 1,
            Some(Status::Error) => counts.error += 1,
            None => {}
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PrepareState;
    use pisi_bump_bot::report_model::PackageReport;

    fn row(name: &str, report: Option<PackageReport>) -> RowState {
        RowState {
            name: name.to_string(),
            recipe_path: format!("{name}/pspec.xml"),
            scannable: true,
            report,
            prepare: PrepareState::Idle,
            written: false,
        }
    }

    fn outdated(name: &str) -> PackageReport {
        PackageReport::new(name, Status::Outdated, "1.0")
    }

    fn current(name: &str) -> PackageReport {
        PackageReport::new(name, Status::Current, "1.0")
    }

    #[test]
    fn should_always_show_rows_that_are_still_scanning() {
        let rows = vec![row("brave-browser", None)];
        assert_eq!(visible_indices(&rows, false, ""), vec![0]);
    }

    #[test]
    fn should_hide_non_outdated_rows_when_show_all_is_false() {
        let rows = vec![
            row("outdated-pkg", Some(outdated("outdated-pkg"))),
            row("current-pkg", Some(current("current-pkg"))),
        ];
        assert_eq!(visible_indices(&rows, false, ""), vec![0]);
    }

    #[test]
    fn should_show_all_rows_when_toggle_is_on() {
        let rows = vec![
            row("outdated-pkg", Some(outdated("outdated-pkg"))),
            row("current-pkg", Some(current("current-pkg"))),
        ];
        assert_eq!(visible_indices(&rows, true, ""), vec![0, 1]);
    }

    #[test]
    fn should_filter_by_name_case_insensitively() {
        let rows = vec![
            row("Brave-Browser", Some(current("Brave-Browser"))),
            row("Firefox", Some(current("Firefox"))),
        ];
        assert_eq!(visible_indices(&rows, true, "brave"), vec![0]);
    }

    #[test]
    fn should_combine_status_filter_and_search_filter() {
        let rows = vec![
            row("brave-outdated", Some(outdated("brave-outdated"))),
            row("brave-current", Some(current("brave-current"))),
            row("firefox-outdated", Some(outdated("firefox-outdated"))),
        ];
        assert_eq!(visible_indices(&rows, false, "brave"), vec![0]);
    }

    #[test]
    fn should_count_each_status_bucket() {
        let rows = vec![
            row("a", Some(outdated("a"))),
            row("b", Some(current("b"))),
            row("c", None),
        ];
        let counts = count_statuses(&rows);
        assert_eq!(
            counts,
            StatusCounts {
                total: 3,
                outdated: 1,
                current: 1,
                unsupported: 0,
                uncomparable: 0,
                error: 0,
            }
        );
    }
}
