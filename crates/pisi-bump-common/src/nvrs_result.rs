use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvrsPackageResult {
    pub recipe_path: String,
    pub source_name: String,
    pub latest_version: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvrsReport {
    pub generated_at: String,
    pub packages: Vec<NvrsPackageResult>,
}
