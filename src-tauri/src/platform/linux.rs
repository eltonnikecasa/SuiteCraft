use crate::models::DirectoryStatus;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

pub fn user_install_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|x| x.join(".local/opt/suitecraft/apps"))
        .ok_or("Diretório pessoal indisponível".into())
}

pub fn inspect_root(target: &Path) -> DirectoryStatus {
    if target.starts_with("/opt") {
        return DirectoryStatus { writable: false, reason: Some("A instalação global em /opt exige um helper administrativo auditado, ainda indisponível neste Alpha. Use o diretório pessoal.".into()) };
    }
    let Ok(root) = user_install_root() else {
        return DirectoryStatus {
            writable: false,
            reason: Some("Diretório pessoal indisponível".into()),
        };
    };
    if !target.is_absolute()
        || target
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || !target.starts_with(&root)
    {
        return DirectoryStatus {
            writable: false,
            reason: Some(format!("Use um caminho dentro de {}.", root.display())),
        };
    }
    let check = target.ancestors().find(|x| x.exists()).unwrap_or(&root);
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

fn data_home() -> Result<PathBuf, String> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|x| x.join(".local/share")))
        .ok_or("XDG_DATA_HOME indisponível".into())
}
fn desktop_path(id: &str) -> Result<PathBuf, String> {
    Ok(data_home()?
        .join("applications")
        .join(format!("suitecraft-{id}.desktop")))
}
fn quote_exec(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

pub fn read_valid_icon(path: &Path) -> Result<Option<(&'static str, Vec<u8>)>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(x) => x,
        Err(_) => return Ok(None),
    };
    if !meta.file_type().is_file() || meta.len() == 0 || meta.len() > MAX_ICON_BYTES {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else {
        return Ok(None);
    };
    Ok(Some((mime, bytes)))
}

fn theme_icon(name: &str) -> Option<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    let mut roots = vec![data_home().ok()?.join("icons")];
    roots.extend(
        [
            "/usr/local/share/icons",
            "/usr/share/icons",
            "/usr/share/pixmaps",
        ]
        .into_iter()
        .map(PathBuf::from),
    );
    for root in roots {
        for suffix in [
            format!("hicolor/256x256/apps/{name}.png"),
            format!("hicolor/128x128/apps/{name}.png"),
            format!("{name}.png"),
            format!("{name}.jpg"),
        ] {
            let candidate = root.join(suffix);
            if read_valid_icon(&candidate).ok().flatten().is_some() {
                return Some(candidate);
            }
        }
    }
    None
}

fn desktop_icon_for(executable: &Path) -> Option<PathBuf> {
    let expected = fs::canonicalize(executable).ok()?;
    let mut roots = vec![data_home().ok()?.join("applications")];
    roots.extend(
        ["/usr/local/share/applications", "/usr/share/applications"]
            .into_iter()
            .map(PathBuf::from),
    );
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten().take(4096) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.file_type().is_file() || meta.len() > 256 * 1024 {
                continue;
            }
            let Ok(body) = fs::read_to_string(path) else {
                continue;
            };
            let exec = body
                .lines()
                .find_map(|x| x.strip_prefix("Exec="))
                .and_then(exec_program);
            if exec.and_then(|x| fs::canonicalize(x).ok()).as_ref() != Some(&expected) {
                continue;
            }
            let Some(icon) = body
                .lines()
                .find_map(|x| x.strip_prefix("Icon="))
                .map(str::trim)
            else {
                continue;
            };
            let candidate = PathBuf::from(icon);
            let resolved = if candidate.is_absolute() {
                Some(candidate)
            } else {
                theme_icon(icon)
            };
            if let Some(found) = resolved.filter(|x| read_valid_icon(x).ok().flatten().is_some()) {
                return Some(found);
            }
        }
    }
    None
}

fn exec_program(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find('"')?;
        Some(PathBuf::from(&rest[..end]))
    } else {
        value.split_whitespace().next().map(PathBuf::from)
    }
}

pub fn integrate(id: &str, name: &str, executable: &Path) -> Result<Option<PathBuf>, String> {
    let source = desktop_icon_for(executable);
    let cached = if let Some(source) = source {
        let bytes = read_valid_icon(&source)?
            .ok_or("Ícone deixou de ser válido durante a cópia")?
            .1;
        let dir = data_home()?.join("icons/hicolor/256x256/apps");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let destination = dir.join(format!("suitecraft-{id}.png"));
        let temp = dir.join(format!(".suitecraft-{id}.png.tmp"));
        fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        fs::rename(temp, &destination).map_err(|e| e.to_string())?;
        Some(destination)
    } else {
        None
    };
    let path = desktop_path(id)?;
    fs::create_dir_all(path.parent().ok_or("Caminho de atalho inválido")?)
        .map_err(|e| e.to_string())?;
    let mut body = format!("[Desktop Entry]\nType=Application\nName={}\nComment=Installed by SuiteCraft\nExec={}\nTerminal=false\nCategories=Utility;\n", name.replace(['\n', '\r'], " "), quote_exec(executable));
    if let Some(icon) = &cached {
        body.push_str(&format!("Icon={}\n", icon.display()));
    }
    let temp = path.with_extension("desktop.tmp");
    fs::write(&temp, body)
        .map_err(|e| format!("App instalado, mas o atalho não pôde ser criado: {e}"))?;
    fs::rename(temp, path)
        .map_err(|e| format!("App instalado, mas o atalho não pôde ser criado: {e}"))?;
    Ok(cached)
}

pub fn remove_integration(id: &str) -> Result<(), String> {
    let desktop = desktop_path(id)?;
    if desktop.exists() {
        fs::remove_file(desktop).map_err(|e| e.to_string())?;
    }
    Ok(())
}
