use crate::models::{CatalogApp, DirectoryStatus, InstalledApp, ReleaseInfo};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
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
pub fn read_installed() -> Result<Vec<InstalledApp>, String> {
    let path = state_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Estado local inválido: {e}"))
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
    let target = Path::new(path);
    if target.starts_with("/opt") {
        return DirectoryStatus{writable:false,reason:Some("A instalação em /opt está bloqueada até existir um helper polkit auditado. Escolha um diretório pessoal.".into())};
    }
    let Some(home) = dirs::home_dir() else {
        return DirectoryStatus {
            writable: false,
            reason: Some("Diretório pessoal indisponível".into()),
        };
    };
    if !target.is_absolute() || !target.starts_with(&home) {
        return DirectoryStatus {
            writable: false,
            reason: Some("Escolha um caminho absoluto dentro do seu diretório pessoal.".into()),
        };
    }
    let check = if target.exists() {
        target
    } else {
        target.ancestors().find(|x| x.exists()).unwrap_or(&home)
    };
    match fs::metadata(check) {
        Ok(m) => DirectoryStatus {
            writable: !m.permissions().readonly(),
            reason: m
                .permissions()
                .readonly()
                .then(|| "O diretório não permite escrita pelo usuário atual.".into()),
        },
        Err(e) => DirectoryStatus {
            writable: false,
            reason: Some(format!("Não foi possível acessar o diretório: {e}")),
        },
    }
}
fn desktop_escape(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}
fn desktop_path(id: &str) -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir())
        .ok_or("XDG_DATA_HOME indisponível")?;
    Ok(base
        .join("applications")
        .join(format!("suitecraft-{id}.desktop")))
}
fn shortcut(app: &CatalogApp, item: &InstalledApp) -> Result<(), String> {
    let path = desktop_path(&app.id)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let icon = app
        .icon_url
        .as_ref()
        .filter(|x| Path::new(x.as_str()).is_absolute())
        .cloned()
        .unwrap_or_else(|| "application-x-executable".into());
    let body=format!("[Desktop Entry]\nType=Application\nName={}\nComment=Installed by SuiteCraft\nExec={}\nIcon={}\nTerminal=false\nCategories=Utility;\n",app.name.replace('\n'," "),desktop_escape(Path::new(&item.executable_path)),icon.replace('\n',""));
    fs::write(path, body)
        .map_err(|e| format!("App instalado, mas o atalho não pôde ser criado: {e}"))
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
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Não foi possível criar o diretório: {e}"))?;
    let final_path = dir.join(format!("{}.AppImage", app.id));
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
    let item = InstalledApp {
        app_id: app.id.clone(),
        version: release.version,
        executable_path: final_path.to_string_lossy().into_owned(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    let mut items = read_installed()?;
    items.retain(|x| x.app_id != item.app_id);
    items.push(item.clone());
    write_installed(&items)?;
    shortcut(&app, &item)?;
    Ok(item)
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
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?
    }
    let desktop = desktop_path(&id)?;
    if desktop.exists() {
        fs::remove_file(desktop).map_err(|e| e.to_string())?
    }
    items.retain(|x| x.app_id != id);
    write_installed(&items)
}
