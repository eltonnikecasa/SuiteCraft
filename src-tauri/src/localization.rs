use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const SUPPORTED_PREFERENCES: [&str; 3] = ["system", "en", "pt-BR"];

#[derive(Serialize, Deserialize)]
struct LanguageConfig {
    language: String,
}

fn config_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|path| path.join("suitecraft/language.json"))
        .ok_or("User configuration directory is unavailable".into())
}
fn read_from(path: &Path) -> String {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<LanguageConfig>(&bytes).ok())
        .map(|config| config.language)
        .filter(|value| SUPPORTED_PREFERENCES.contains(&value.as_str()))
        .unwrap_or_else(|| "system".into())
}
fn write_to(path: &Path, language: &str) -> Result<(), String> {
    if !SUPPORTED_PREFERENCES.contains(&language) {
        return Err("Unsupported language preference".into());
    }
    let parent = path.parent().ok_or("Invalid configuration path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(&LanguageConfig {
        language: language.into(),
    })
    .map_err(|error| error.to_string())?;
    fs::write(&temporary, content).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}
pub fn get() -> Result<String, String> {
    Ok(read_from(&config_path()?))
}
pub fn set(language: &str) -> Result<(), String> {
    write_to(&config_path()?, language)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_and_reloads_language() {
        let directory =
            std::env::temp_dir().join(format!("suitecraft-i18n-{}", std::process::id()));
        let path = directory.join("language.json");
        write_to(&path, "pt-BR").unwrap();
        assert_eq!(read_from(&path), "pt-BR");
        assert!(write_to(&path, "invalid").is_err());
        let _ = fs::remove_dir_all(directory);
    }
    #[test]
    fn invalid_or_missing_config_uses_system_default() {
        let path =
            std::env::temp_dir().join(format!("suitecraft-i18n-missing-{}", std::process::id()));
        assert_eq!(read_from(&path), "system");
    }
}
