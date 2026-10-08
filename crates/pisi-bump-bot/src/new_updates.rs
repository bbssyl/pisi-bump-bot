use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pisi_bump_common::{compare_versions, normalize_version};

use crate::report_model::{PackageReport, Report, Status};

pub fn package_key(name: &str, recipe_path: &str) -> String {
    if recipe_path.is_empty() {
        name.to_string()
    } else {
        recipe_path.to_string()
    }
}

pub fn load_previous_outdated(path: &Path) -> Option<HashMap<String, Option<String>>> {
    let text = fs::read_to_string(path).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    let packages = document.get("packages")?.as_array()?;
    let mut previous = HashMap::new();
    for package in packages {
        let status = package.get("status").and_then(serde_json::Value::as_str);
        if status != Some(Status::Outdated.as_str()) {
            continue;
        }
        let name = package.get("name").and_then(serde_json::Value::as_str)?;
        let recipe_path = package
            .get("recipe_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let latest_version = package
            .get("latest_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        previous.insert(package_key(name, recipe_path), latest_version);
    }
    Some(previous)
}

fn is_newer(latest: Option<&str>, previous: Option<&str>) -> bool {
    if latest == previous {
        return false;
    }
    let parsed_latest = normalize_version(latest.unwrap_or(""), &[]);
    let parsed_previous = normalize_version(previous.unwrap_or(""), &[]);
    match (parsed_latest, parsed_previous) {
        (Some(latest), Some(previous)) => compare_versions(&latest, &previous) > 0,
        _ => true,
    }
}

fn is_new_update(package: &PackageReport, previous: &HashMap<String, Option<String>>) -> bool {
    let key = package_key(&package.name, &package.recipe_path);
    match previous.get(&key) {
        None => true,
        Some(previous_version) => is_newer(
            package.latest_version.as_deref(),
            previous_version.as_deref(),
        ),
    }
}

pub fn render_new_updates(report: &Report, previous: &HashMap<String, Option<String>>) -> String {
    let fresh: Vec<&PackageReport> = report
        .with_status(Status::Outdated)
        .into_iter()
        .filter(|package| is_new_update(package, previous))
        .collect();
    if fresh.is_empty() {
        return String::new();
    }
    let mut lines = vec!["## Yeni güncellemeler".to_string(), String::new()];
    for package in fresh {
        let reference = match (&package.latest_version, &package.release_url) {
            (Some(latest), Some(url)) => format!("[{latest}]({url})"),
            (Some(latest), None) => latest.clone(),
            (None, _) => String::new(),
        };
        lines.push(format!(
            "- **{}** (`{}`): {} -> {}",
            package.name, package.recipe_path, package.current_version, reference
        ));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_consistency::IndexConsistency;

    fn consistency() -> IndexConsistency {
        IndexConsistency {
            found: true,
            error: None,
            missing_from_index: Vec::new(),
            version_mismatches: Vec::new(),
        }
    }

    fn sample_report(latest: &str) -> Report {
        let alpha = PackageReport::new("alpha", Status::Outdated, "1.0")
            .with_latest_version(latest)
            .with_upstream("o/alpha")
            .with_release_url("https://r/alpha")
            .with_recipe_path("alpha/pspec.xml");
        Report::new(Some("abc".to_string()), vec![alpha], consistency())
    }

    #[test]
    fn should_return_empty_text_when_nothing_changed() {
        let mut previous = HashMap::new();
        previous.insert("alpha/pspec.xml".to_string(), Some("v2".to_string()));
        assert_eq!(render_new_updates(&sample_report("v2"), &previous), "");
    }

    #[test]
    fn should_list_package_when_it_was_not_outdated_before() {
        let previous = HashMap::new();
        assert!(render_new_updates(&sample_report("v2"), &previous).contains("**alpha**"));
    }

    #[test]
    fn should_list_package_when_latest_version_increased() {
        let mut previous = HashMap::new();
        previous.insert("alpha/pspec.xml".to_string(), Some("v2".to_string()));
        assert!(render_new_updates(&sample_report("v3"), &previous).contains("**alpha**"));
    }

    #[test]
    fn should_return_none_when_previous_file_is_missing() {
        assert!(load_previous_outdated(Path::new("/nonexistent/previous.json")).is_none());
    }
}
