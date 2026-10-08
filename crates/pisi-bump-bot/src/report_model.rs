use serde::{Deserialize, Serialize};

use crate::index_consistency::IndexConsistency;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "eski")]
    Outdated,
    #[serde(rename = "guncel")]
    Current,
    #[serde(rename = "desteklenmiyor")]
    Unsupported,
    #[serde(rename = "karsilastirilamadi")]
    Uncomparable,
    #[serde(rename = "hata")]
    Error,
}

impl Status {
    pub const ALL: [Status; 5] = [
        Status::Outdated,
        Status::Current,
        Status::Unsupported,
        Status::Uncomparable,
        Status::Error,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Outdated => "eski",
            Status::Current => "guncel",
            Status::Unsupported => "desteklenmiyor",
            Status::Uncomparable => "karsilastirilamadi",
            Status::Error => "hata",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageReport {
    pub name: String,
    pub status: Status,
    pub current_version: String,
    #[serde(default)]
    pub latest_version: Option<String>,
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default)]
    pub release_url: Option<String>,
    #[serde(default)]
    pub candidate_url: Option<String>,
    #[serde(default)]
    pub candidate_sha1: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub recipe_path: String,
}

impl PackageReport {
    pub fn new(
        name: impl Into<String>,
        status: Status,
        current_version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status,
            current_version: current_version.into(),
            latest_version: None,
            upstream: None,
            release_url: None,
            candidate_url: None,
            candidate_sha1: None,
            detail: None,
            recipe_path: String::new(),
        }
    }

    pub fn with_status(mut self, status: Status) -> Self {
        self.status = status;
        self
    }

    pub fn with_latest_version(mut self, value: impl Into<String>) -> Self {
        self.latest_version = Some(value.into());
        self
    }

    pub fn with_upstream(mut self, value: impl Into<String>) -> Self {
        self.upstream = Some(value.into());
        self
    }

    pub fn with_release_url(mut self, value: impl Into<String>) -> Self {
        self.release_url = Some(value.into());
        self
    }

    pub fn with_candidate_url(mut self, value: Option<String>) -> Self {
        self.candidate_url = value;
        self
    }

    pub fn with_candidate_sha1(mut self, value: Option<String>) -> Self {
        self.candidate_sha1 = value;
        self
    }

    pub fn with_detail(mut self, value: Option<String>) -> Self {
        self.detail = value;
        self
    }

    pub fn with_recipe_path(mut self, value: impl Into<String>) -> Self {
        self.recipe_path = value.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub source_commit: Option<String>,
    pub packages: Vec<PackageReport>,
    pub index_consistency: IndexConsistency,
}

impl Report {
    pub fn new(
        source_commit: Option<String>,
        packages: Vec<PackageReport>,
        index_consistency: IndexConsistency,
    ) -> Self {
        Self {
            source_commit,
            packages,
            index_consistency,
        }
    }

    pub fn count(&self, status: Status) -> usize {
        self.packages
            .iter()
            .filter(|package| package.status == status)
            .count()
    }

    pub fn with_status(&self, status: Status) -> Vec<&PackageReport> {
        self.packages
            .iter()
            .filter(|package| package.status == status)
            .collect()
    }
}
