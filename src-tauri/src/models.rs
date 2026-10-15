use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRule {
    pub extensions: Vec<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub architecture: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogApp {
    pub id: String,
    pub name: String,
    pub repository_url: Option<String>,
    pub icon_url: Option<String>,
    pub asset_rule: AssetRule,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub checksum: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub release_url: String,
    pub published_at: Option<String>,
    pub asset: Option<ReleaseAsset>,
    pub reason: Option<String>,
    pub cached: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub app_id: String,
    pub version: String,
    pub executable_path: String,
    pub installed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct DirectoryStatus {
    pub writable: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationArtifact {
    pub path: String,
    pub kind: String,
    pub external: bool,
    pub removable: bool,
    pub detail: String,
}
