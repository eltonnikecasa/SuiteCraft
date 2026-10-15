use crate::models::{DirectoryStatus, IntegrationArtifact};
use std::path::{Path, PathBuf};

pub fn user_install_root() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|x| x.join("SuiteCraft").join("apps"))
        .ok_or("LocalAppData unavailable".into())
}
pub fn inspect_root(path: &Path) -> DirectoryStatus {
    let allowed = user_install_root()
        .map(|root| path.starts_with(root))
        .unwrap_or(false);
    DirectoryStatus {
        writable: allowed,
        reason: (!allowed).then(|| "Windows installation is not implemented in this alpha.".into()),
    }
}
pub fn integrate(_: &str, _: &str, _: &Path) -> Result<Option<PathBuf>, String> {
    Ok(None)
}
pub fn integration_artifacts(_: &str, _: &Path) -> Result<Vec<IntegrationArtifact>, String> {
    Ok(vec![])
}
pub fn remove_integrations(_: &str, _: &Path, _: &[String]) -> Result<(), String> {
    Ok(())
}
pub fn read_valid_icon(_: &Path) -> Result<Option<(&'static str, Vec<u8>)>, String> {
    Ok(None)
}
