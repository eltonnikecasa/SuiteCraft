use crate::models::{CatalogApp, DirectoryStatus, InstalledApp, ReleaseInfo};
use crate::platform;
use base64::Engine;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tokio::io::AsyncWriteExt;

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
fn data_dir() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|x| x.join("suitecraft"))
        .ok_or("Não foi possível localizar o diretório de dados".into())
}
fn state_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("installed.json"))
}
fn read_state() -> Result<Vec<InstalledApp>, String> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Estado local inválido: {e}"))
}
fn reconcile_from_root(
    mut items: Vec<InstalledApp>,
    root: &Path,
) -> Result<(Vec<InstalledApp>, bool), String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((items, false)),
        Err(error) => {
            return Err(format!(
                "Não foi possível consultar instalações existentes: {error}"
            ))
        }
    };
    let mut changed = false;
    for entry in entries.take(4096).flatten() {
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !valid_id(&id) || items.iter().any(|item| item.app_id == id) {
            continue;
        }
        let path = entry.path().join(format!("{id}.AppImage"));
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.file_type().is_file() || metadata.len() < 4 {
            continue;
        }
        let mut magic = [0_u8; 4];
        if fs::File::open(&path)
            .and_then(|mut file| file.read_exact(&mut magic))
            .is_err()
            || magic != *b"\x7fELF"
        {
            continue;
        }
        items.push(InstalledApp {
            app_id: id,
            version: String::new(),
            executable_path: path.to_string_lossy().into_owned(),
            installed_at: String::new(),
            icon_path: None,
        });
        changed = true;
    }
    Ok((items, changed))
}
pub fn read_installed() -> Result<Vec<InstalledApp>, String> {
    let items = read_state()?;
    let root = platform::user_install_root()?;
    let (items, changed) = reconcile_from_root(items, &root)?;
    if changed {
        write_installed(&items)?;
    }
    Ok(items)
}
fn write_installed(items: &[InstalledApp]) -> Result<(), String> {
    let dir = data_dir()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = state_path()?;
    let temp = path.with_extension("json.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
pub fn inspect(path: &str) -> DirectoryStatus {
    platform::inspect_root(Path::new(path))
}

pub async fn install(
    app: CatalogApp,
    release: ReleaseInfo,
    root: String,
) -> Result<InstalledApp, String> {
    if !valid_id(&app.id) {
        return Err("ID de aplicativo inválido".into());
    }
    let asset = release.asset.ok_or("Release sem AppImage validado")?;
    let asset_url = url::Url::parse(&asset.url).map_err(|_| "URL do asset inválida")?;
    let allowed_hosts = [
        "github.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
    ];
    if asset_url.scheme() != "https"
        || !allowed_hosts.contains(&asset_url.host_str().unwrap_or_default())
    {
        return Err("O asset não está hospedado em uma origem GitHub permitida".into());
    }
    let lower_name = asset.name.to_lowercase();
    let matches_rule = app
        .asset_rule
        .extensions
        .iter()
        .any(|x| lower_name.ends_with(&x.to_lowercase()))
        && app
            .asset_rule
            .include
            .iter()
            .any(|x| lower_name.contains(&x.to_lowercase()))
        && !app
            .asset_rule
            .exclude
            .iter()
            .any(|x| lower_name.contains(&x.to_lowercase()));
    if !matches_rule {
        return Err("O asset não atende à regra AppImage/arquitetura do manifesto".into());
    }
    if asset.size == 0 || asset.size > 2_147_483_648 {
        return Err("Tamanho do asset inválido ou acima do limite de 2 GiB".into());
    }
    let status = inspect(&root);
    if !status.writable {
        return Err(status
            .reason
            .unwrap_or("Diretório sem permissão de escrita".into()));
    }
    let dir = PathBuf::from(root).join(&app.id);
    let final_path = dir.join(format!("{}.AppImage", app.id));
    if final_path.exists()
        && !read_installed()?.iter().any(|item| {
            item.app_id == app.id && Path::new(&item.executable_path) == final_path.as_path()
        })
    {
        return Err("Já existe um arquivo nesse destino que não pertence a uma instalação registrada; ele não será sobrescrito.".into());
    }
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Não foi possível criar o diretório: {e}"))?;
    let temp = dir.join(format!(".{}.download", app.id));
    let response = reqwest::Client::builder()
        .user_agent("SuiteCraft/0.1")
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(1800))
        .build()
        .map_err(|e| e.to_string())?
        .get(&asset.url)
        .send()
        .await
        .map_err(|e| format!("Download falhou: {e}"))?;
    if response.url().scheme() != "https"
        || !allowed_hosts.contains(&response.url().host_str().unwrap_or_default())
    {
        return Err("O redirecionamento do download saiu das origens GitHub permitidas".into());
    }
    if !response.status().is_success() {
        return Err(format!("Download respondeu HTTP {}", response.status()));
    }
    if let Some(len) = response.content_length() {
        if len != asset.size {
            return Err("Tamanho informado pelo servidor difere do release".into());
        }
    }
    let mut file = tokio::fs::File::create(&temp)
        .await
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrompido: {e}"))?;
        total += chunk.len() as u64;
        if total > asset.size {
            return Err("Download excedeu o tamanho esperado".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);
    if total != asset.size {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err("Download incompleto".into());
    }
    let head = tokio::fs::read(&temp).await.map_err(|e| e.to_string())?;
    if !head.starts_with(b"\x7fELF") {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err("O arquivo baixado não é um executável ELF/AppImage válido".into());
    }
    if let Some(expected) = asset.checksum {
        if hex::encode(hash.finalize()) != expected.to_lowercase() {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err("Checksum SHA-256 não confere".into());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temp, fs::Permissions::from_mode(0o755))
            .await
            .map_err(|e| e.to_string())?
    }
    let backup = dir.join(format!(".{}.previous", app.id));
    if final_path.exists() {
        let _ = tokio::fs::remove_file(&backup).await;
        tokio::fs::rename(&final_path, &backup)
            .await
            .map_err(|e| format!("Não foi possível preservar a versão anterior: {e}"))?
    }
    if let Err(e) = tokio::fs::rename(&temp, &final_path).await {
        if backup.exists() {
            let _ = tokio::fs::rename(&backup, &final_path).await;
        }
        return Err(format!(
            "Falha ao promover a nova versão; a anterior foi restaurada: {e}"
        ));
    }
    let mut item = InstalledApp {
        app_id: app.id.clone(),
        version: release.version,
        executable_path: final_path.to_string_lossy().into_owned(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        icon_path: None,
    };
    let finalize = (|| -> Result<(), String> {
        item.icon_path = platform::integrate(&app.id, &app.name, &final_path)?
            .map(|x| x.to_string_lossy().into_owned());
        let mut items = read_installed()?;
        items.retain(|x| x.app_id != item.app_id);
        items.push(item.clone());
        write_installed(&items)
    })();
    if let Err(error) = finalize {
        let _ = fs::remove_file(&final_path);
        if backup.exists() {
            let _ = fs::rename(&backup, &final_path);
            let _ = platform::integrate(&app.id, &app.name, &final_path);
        } else {
            let _ = platform::remove_integration(&app.id);
        }
        return Err(format!(
            "Falha ao concluir a instalação; a versão anterior foi restaurada: {error}"
        ));
    }
    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(item)
}
pub fn icon_data(id: &str) -> Result<Option<String>, String> {
    if !valid_id(id) {
        return Err("ID inválido".into());
    }
    let Some(path) = read_installed()?
        .into_iter()
        .find(|x| x.app_id == id)
        .and_then(|x| x.icon_path)
    else {
        return Ok(None);
    };
    let bytes = platform::read_valid_icon(Path::new(&path))?;
    let Some((mime, bytes)) = bytes else {
        return Ok(None);
    };
    Ok(Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}
pub async fn launch(id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("ID inválido".into());
    }
    let item = read_installed()?
        .into_iter()
        .find(|x| x.app_id == id)
        .ok_or("App não instalado")?;
    let path = PathBuf::from(item.executable_path);
    if !path.is_file() {
        return Err("O AppImage instalado não foi encontrado".into());
    }
    tokio::process::Command::new(path)
        .spawn()
        .map_err(|e| format!("Não foi possível abrir o app: {e}"))?;
    Ok(())
}
pub fn remove(id: String) -> Result<(), String> {
    if !valid_id(&id) {
        return Err("ID inválido".into());
    }
    let mut items = read_installed()?;
    let Some(item) = items.iter().find(|x| x.app_id == id).cloned() else {
        return Ok(());
    };
    let path = PathBuf::from(item.executable_path);
    if path.file_name().and_then(|x| x.to_str()) != Some(&format!("{id}.AppImage")) {
        return Err("Caminho registrado não passou na validação de segurança".into());
    }
    if !platform::inspect_root(path.parent().unwrap_or(Path::new("/"))).writable {
        return Err("O caminho registrado está fora da raiz de instalação permitida".into());
    }
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?
    }
    platform::remove_integration(&id)?;
    items.retain(|x| x.app_id != id);
    write_installed(&items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "suitecraft-install-test-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn reconciles_an_existing_appimage_without_moving_it() {
        let root = test_root("existing");
        let app_dir = root.join("deckcraft");
        fs::create_dir_all(&app_dir).unwrap();
        let appimage = app_dir.join("deckcraft.AppImage");
        fs::write(&appimage, b"\x7fELFexisting-content").unwrap();

        let (items, changed) = reconcile_from_root(vec![], &root).unwrap();
        assert!(changed);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].app_id, "deckcraft");
        assert_eq!(items[0].executable_path, appimage.to_string_lossy());
        assert_eq!(fs::read(&appimage).unwrap(), b"\x7fELFexisting-content");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reconciliation_is_idempotent_and_rejects_unsafe_entries() {
        let root = test_root("safe");
        fs::create_dir_all(root.join("soundcraft")).unwrap();
        fs::write(root.join("soundcraft/soundcraft.AppImage"), b"\x7fELFapp").unwrap();
        fs::create_dir_all(root.join("INVALID ID")).unwrap();
        fs::write(root.join("INVALID ID/INVALID ID.AppImage"), b"\x7fELFapp").unwrap();

        let (first, changed) = reconcile_from_root(vec![], &root).unwrap();
        let (second, changed_again) = reconcile_from_root(first, &root).unwrap();
        assert!(changed);
        assert!(!changed_again);
        assert_eq!(
            second
                .iter()
                .map(|item| item.app_id.as_str())
                .collect::<Vec<_>>(),
            ["soundcraft"]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
