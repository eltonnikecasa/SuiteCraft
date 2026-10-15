mod child_env;
mod github;
mod install;
mod localization;
mod models;
mod platform;
use github::ReleaseCache;
use models::{CatalogApp, DirectoryStatus, InstalledApp, IntegrationArtifact, ReleaseInfo};
use std::collections::HashMap;

#[tauri::command]
async fn check_releases(
    apps: Vec<CatalogApp>,
    force: Option<bool>,
    cache: tauri::State<'_, ReleaseCache>,
) -> Result<HashMap<String, ReleaseInfo>, String> {
    github::all(apps, force.unwrap_or(false), &cache).await
}
#[tauri::command]
fn list_installed() -> Result<Vec<InstalledApp>, String> {
    install::read_installed()
}
#[tauri::command]
async fn install_app(
    app: CatalogApp,
    release: ReleaseInfo,
    install_root: String,
) -> Result<InstalledApp, String> {
    install::install(app, release, install_root).await
}
#[tauri::command]
async fn launch_app(app_id: String) -> Result<(), String> {
    install::launch(app_id).await
}
#[tauri::command]
fn inspect_app_integrations(app_id: String) -> Result<Vec<IntegrationArtifact>, String> {
    install::integration_artifacts(app_id)
}
#[tauri::command]
fn remove_app(app_id: String, approved_integration_paths: Vec<String>) -> Result<(), String> {
    install::remove(app_id, approved_integration_paths)
}
#[tauri::command]
fn inspect_install_directory(path: String) -> DirectoryStatus {
    install::inspect(&path)
}
#[tauri::command]
fn suggest_user_install_root() -> Result<String, String> {
    platform::user_install_root().map(|x| x.to_string_lossy().into_owned())
}
#[tauri::command]
fn get_app_icon(app_id: String) -> Result<Option<String>, String> {
    install::icon_data(&app_id)
}
#[tauri::command]
fn get_language_preference() -> Result<String, String> {
    localization::get()
}
#[tauri::command]
fn set_language_preference(language: String) -> Result<(), String> {
    localization::set(&language)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ReleaseCache::default())
        .invoke_handler(tauri::generate_handler![
            check_releases,
            list_installed,
            install_app,
            launch_app,
            inspect_app_integrations,
            remove_app,
            inspect_install_directory,
            suggest_user_install_root,
            get_app_icon,
            get_language_preference,
            set_language_preference
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SuiteCraft")
}
