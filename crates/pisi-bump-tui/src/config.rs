use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pisi_bump_bot::report_model::PackageReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AuthMethod {
    GhAuth,
    Manual { token: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedPackage {
    #[serde(flatten)]
    pub report: PackageReport,
    pub checked_at: String,
    #[serde(default)]
    pub written: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: Option<AuthMethod>,
    #[serde(default)]
    pub packages: HashMap<String, CachedPackage>,
}

pub fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".config/pisi-bump-tui/config.json"))
}

pub fn load(path: &Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
    fs::write(path, text)?;
    restrict_permissions(path)
}

pub fn now_iso() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pisi_bump_bot::report_model::Status;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pisi-bump-tui-config-test-{label}.json"))
    }

    fn build_placeholder_credential() -> String {
        ["placeholder", "credential", "value", "for", "testing"].join("-")
    }

    #[test]
    fn should_default_to_empty_config_when_file_does_not_exist() {
        let path = temp_path("missing-file");
        let _ = fs::remove_file(&path);

        let config = load(&path);

        assert_eq!(config.auth, None);
        assert!(config.packages.is_empty());
    }

    #[test]
    fn should_round_trip_gh_auth_method() {
        let path = temp_path("round-trip-gh-auth");
        let original = Config {
            auth: Some(AuthMethod::GhAuth),
            packages: HashMap::new(),
        };

        save(&path, &original).expect("save should succeed");
        let loaded = load(&path);

        assert_eq!(loaded.auth, Some(AuthMethod::GhAuth));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_round_trip_manual_credential_method() {
        let path = temp_path("round-trip-manual");
        let placeholder_value = build_placeholder_credential();
        let original = Config {
            auth: Some(AuthMethod::Manual {
                token: placeholder_value.clone(),
            }),
            packages: HashMap::new(),
        };

        save(&path, &original).expect("save should succeed");
        let loaded = load(&path);

        assert_eq!(
            loaded.auth,
            Some(AuthMethod::Manual {
                token: placeholder_value
            })
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_restrict_file_permissions_to_owner_read_write_after_save() {
        let path = temp_path("permissions");
        let config = Config {
            auth: Some(AuthMethod::Manual {
                token: build_placeholder_credential(),
            }),
            packages: HashMap::new(),
        };

        save(&path, &config).expect("save should succeed");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_round_trip_cached_packages() {
        let path = temp_path("round-trip-packages");
        let mut packages = HashMap::new();
        packages.insert(
            "network/browser/brave/pspec.xml".to_string(),
            CachedPackage {
                report: PackageReport::new("brave-browser", Status::Outdated, "1.93.129")
                    .with_latest_version("1.94.1")
                    .with_recipe_path("network/browser/brave/pspec.xml"),
                checked_at: "2026-10-08T12:00:00Z".to_string(),
                written: true,
            },
        );
        let original = Config {
            auth: Some(AuthMethod::GhAuth),
            packages,
        };

        save(&path, &original).expect("save should succeed");
        let loaded = load(&path);

        let cached = loaded
            .packages
            .get("network/browser/brave/pspec.xml")
            .expect("cached package should round-trip");
        assert_eq!(cached.report.status, Status::Outdated);
        assert_eq!(cached.report.latest_version.as_deref(), Some("1.94.1"));
        assert_eq!(cached.checked_at, "2026-10-08T12:00:00Z");
        assert!(cached.written, "the written flag must round-trip too");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_default_written_to_false_when_field_is_missing_from_an_older_config_file() {
        let path = temp_path("missing-written-field");
        fs::write(
            &path,
            r#"{"packages":{"a/pspec.xml":{"name":"a","status":"eski","current_version":"1.0","checked_at":"2026-10-08T12:00:00Z"}}}"#,
        )
        .expect("write should succeed");

        let loaded = load(&path);

        let cached = loaded
            .packages
            .get("a/pspec.xml")
            .expect("cached package should still load");
        assert!(
            !cached.written,
            "an older config.json written before the written field existed must default it to false, not fail to load"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_default_packages_to_empty_when_field_is_missing_from_file() {
        let path = temp_path("missing-packages-field");
        fs::write(&path, r#"{"auth":{"method":"gh_auth"}}"#).expect("write should succeed");

        let loaded = load(&path);

        assert_eq!(loaded.auth, Some(AuthMethod::GhAuth));
        assert!(loaded.packages.is_empty());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn should_default_auth_to_none_when_field_is_missing_from_file() {
        let path = temp_path("missing-auth-field");
        fs::write(&path, r#"{"packages":{}}"#).expect("write should succeed");

        let loaded = load(&path);

        assert_eq!(loaded.auth, None);
        let _ = fs::remove_file(&path);
    }
}
