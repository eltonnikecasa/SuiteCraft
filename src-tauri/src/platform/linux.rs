use crate::models::{DirectoryStatus, IntegrationArtifact};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DESKTOP_BYTES: u64 = 256 * 1024;

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
fn applications_dir() -> Result<PathBuf, String> {
    Ok(data_home()?.join("applications"))
}
fn icons_dir() -> Result<PathBuf, String> {
    Ok(data_home()?.join("icons"))
}
fn desktop_path(id: &str) -> Result<PathBuf, String> {
    Ok(applications_dir()?.join(format!("suitecraft-{id}.desktop")))
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
fn desktop_roots() -> Result<Vec<(PathBuf, bool)>, String> {
    Ok(vec![
        (applications_dir()?, true),
        (PathBuf::from("/usr/local/share/applications"), false),
        (PathBuf::from("/usr/share/applications"), false),
    ])
}
fn parse_desktop(body: &str) -> BTreeMap<&str, &str> {
    let mut fields = BTreeMap::new();
    let mut in_desktop_entry = false;
    for line in body.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if in_desktop_entry && line.starts_with('[') {
            break;
        }
        if in_desktop_entry {
            if let Some((key, value)) = line.split_once('=') {
                fields.insert(key.trim(), value.trim());
            }
        }
    }
    fields
}
// Exec is shell-like but is never passed to a shell. Only its first token is inspected.
fn exec_program(value: &str) -> Option<PathBuf> {
    let mut token = String::new();
    let mut chars = value.trim().chars().peekable();
    let mut quote = None;
    while let Some(character) = chars.next() {
        match (quote, character) {
            (Some(expected), current) if current == expected => quote = None,
            (Some('"'), '\\') => token.push(chars.next()?),
            (Some(_), current) => token.push(current),
            (None, '"' | '\'') => quote = Some(character),
            (None, '\\') => token.push(chars.next()?),
            (None, current) if current.is_whitespace() => break,
            (None, current) => token.push(current),
        }
    }
    (!token.is_empty() && quote.is_none()).then(|| PathBuf::from(token))
}

#[derive(Clone)]
struct DesktopMatch {
    path: PathBuf,
    user: bool,
    suitecraft_owned: bool,
    icon: Option<String>,
}
fn regular_file(path: &Path, max_size: u64) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_file() && metadata.len() <= max_size)
}
fn matching_desktops(id: &str, executable: &Path) -> Result<Vec<DesktopMatch>, String> {
    matching_desktops_in(id, executable, desktop_roots()?)
}
fn matching_desktops_in(
    id: &str,
    executable: &Path,
    roots: Vec<(PathBuf, bool)>,
) -> Result<Vec<DesktopMatch>, String> {
    let expected = fs::canonicalize(executable)
        .map_err(|error| format!("Não foi possível validar o executável instalado: {error}"))?;
    let mut matches = vec![];
    for (root, user) in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten().take(4096) {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop")
                || !regular_file(&path, MAX_DESKTOP_BYTES)
            {
                continue;
            }
            let Ok(body) = fs::read_to_string(&path) else {
                continue;
            };
            let fields = parse_desktop(&body);
            let Some(program) = fields.get("Exec").and_then(|value| exec_program(value)) else {
                continue;
            };
            if fs::canonicalize(program).ok().as_ref() != Some(&expected) {
                continue;
            }
            matches.push(DesktopMatch {
                path,
                user,
                suitecraft_owned: fields.get("X-SuiteCraft-AppId") == Some(&id),
                icon: fields.get("Icon").map(|value| value.to_string()),
            });
        }
    }
    Ok(matches)
}
fn theme_icon(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    let roots = [
        icons_dir().ok()?,
        PathBuf::from("/usr/local/share/icons"),
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
    ];
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
fn resolve_icon(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    (if path.is_absolute() {
        Some(path)
    } else {
        theme_icon(value)
    })
    .filter(|candidate| read_valid_icon(candidate).ok().flatten().is_some())
}
fn icon_references_in(roots: Vec<PathBuf>) -> BTreeMap<PathBuf, usize> {
    let mut references = BTreeMap::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten().take(4096) {
            let path = entry.path();
            if !regular_file(&path, MAX_DESKTOP_BYTES) {
                continue;
            }
            let Ok(body) = fs::read_to_string(path) else {
                continue;
            };
            if let Some(icon) = parse_desktop(&body)
                .get("Icon")
                .and_then(|value| resolve_icon(value))
            {
                if let Ok(canonical) = fs::canonicalize(icon) {
                    *references.entry(canonical).or_insert(0) += 1;
                }
            }
        }
    }
    references
}
fn all_icon_references() -> Result<BTreeMap<PathBuf, usize>, String> {
    Ok(icon_references_in(
        desktop_roots()?.into_iter().map(|(path, _)| path).collect(),
    ))
}
fn removable_user_icon(path: &Path, reference_count: usize) -> bool {
    let Ok(root) = fs::canonicalize(icons_dir().unwrap_or_default()) else {
        return false;
    };
    regular_file(path, MAX_ICON_BYTES)
        && fs::canonicalize(path).is_ok_and(|canonical| canonical.starts_with(root))
        && reference_count <= 1
}

pub fn integration_artifacts(
    id: &str,
    executable: &Path,
) -> Result<Vec<IntegrationArtifact>, String> {
    let desktops = matching_desktops(id, executable)?;
    let references = all_icon_references()?;
    let mut artifacts = vec![];
    let mut icons_seen = BTreeSet::new();
    for desktop in &desktops {
        artifacts.push(IntegrationArtifact {
            path: desktop.path.to_string_lossy().into_owned(),
            kind: "desktop".into(),
            external: !desktop.suitecraft_owned,
            removable: desktop.user,
            detail: if desktop.user {
                "Exec aponta para o executável instalado"
            } else {
                "Integração global detectada; somente leitura"
            }
            .into(),
        });
        let Some(icon) = desktop.icon.as_deref().and_then(resolve_icon) else {
            continue;
        };
        let Ok(canonical) = fs::canonicalize(&icon) else {
            continue;
        };
        if !icons_seen.insert(canonical.clone()) {
            continue;
        }
        let count = references.get(&canonical).copied().unwrap_or(0);
        let removable = removable_user_icon(&icon, count);
        artifacts.push(IntegrationArtifact {
            path: icon.to_string_lossy().into_owned(),
            kind: "icon".into(),
            external: !desktop.suitecraft_owned,
            removable,
            detail: if count > 1 {
                "Ícone compartilhado; será preservado"
            } else if removable {
                "Ícone exclusivo desta integração"
            } else {
                "Ícone fora do diretório de usuário; somente leitura"
            }
            .into(),
        });
    }
    let cached = icons_dir()?
        .join("hicolor/256x256/apps")
        .join(format!("suitecraft-{id}.png"));
    if let Ok(canonical) = fs::canonicalize(&cached) {
        if icons_seen.insert(canonical.clone()) && read_valid_icon(&cached)?.is_some() {
            let count = references.get(&canonical).copied().unwrap_or(0);
            artifacts.push(IntegrationArtifact {
                path: cached.to_string_lossy().into_owned(),
                kind: "icon".into(),
                external: false,
                removable: removable_user_icon(&cached, count),
                detail: "Ícone gerenciado pelo SuiteCraft".into(),
            });
        }
    }
    Ok(artifacts)
}

pub fn integrate(id: &str, name: &str, executable: &Path) -> Result<Option<PathBuf>, String> {
    let matches = matching_desktops(id, executable)?;
    let source = matches
        .iter()
        .find_map(|entry| entry.icon.as_deref().and_then(resolve_icon));
    let cached = if let Some(source) = source {
        let bytes = read_valid_icon(&source)?
            .ok_or("Ícone deixou de ser válido durante a cópia")?
            .1;
        let dir = icons_dir()?.join("hicolor/256x256/apps");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let destination = dir.join(format!("suitecraft-{id}.png"));
        let temp = dir.join(format!(".suitecraft-{id}.png.tmp"));
        fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        fs::rename(temp, &destination).map_err(|e| e.to_string())?;
        Some(destination)
    } else {
        None
    };
    if matches.iter().any(|entry| !entry.suitecraft_owned)
        && !matches.iter().any(|entry| entry.suitecraft_owned)
    {
        return Ok(cached.or_else(|| {
            matches
                .iter()
                .find_map(|entry| entry.icon.as_deref().and_then(resolve_icon))
        }));
    }
    let path = desktop_path(id)?;
    fs::create_dir_all(path.parent().ok_or("Caminho de atalho inválido")?)
        .map_err(|e| e.to_string())?;
    let mut body = format!("[Desktop Entry]\nType=Application\nName={}\nComment=Installed by SuiteCraft\nX-SuiteCraft-AppId={id}\nExec={}\nTerminal=false\nCategories=Utility;\n", name.replace(['\n', '\r'], " "), quote_exec(executable));
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

pub fn remove_integrations(id: &str, executable: &Path, approved: &[String]) -> Result<(), String> {
    let approved: BTreeSet<&str> = approved.iter().map(String::as_str).collect();
    let artifacts = integration_artifacts(id, executable)?;
    for artifact in artifacts.iter().filter(|item| {
        item.kind == "desktop"
            && item.removable
            && (!item.external || approved.contains(item.path.as_str()))
    }) {
        let path = Path::new(&artifact.path);
        if regular_file(path, MAX_DESKTOP_BYTES) {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    let references = all_icon_references()?;
    for artifact in artifacts.iter().filter(|item| {
        item.kind == "icon"
            && item.removable
            && (!item.external || approved.contains(item.path.as_str()))
    }) {
        let path = Path::new(&artifact.path);
        let count = fs::canonicalize(path)
            .ok()
            .as_ref()
            .and_then(|value| references.get(value))
            .copied()
            .unwrap_or(0);
        if removable_user_icon(path, count) && count == 0 {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "suitecraft-integration-{name}-{}",
            std::process::id()
        ))
    }
    #[test]
    fn parses_quoted_exec_and_ignores_arguments() {
        assert_eq!(
            exec_program("\"/tmp/My App.AppImage\" %U --new-window"),
            Some(PathBuf::from("/tmp/My App.AppImage"))
        );
        assert_eq!(
            exec_program("/tmp/app.AppImage %F"),
            Some(PathBuf::from("/tmp/app.AppImage"))
        );
    }
    #[test]
    fn rejects_unterminated_exec_quote() {
        assert_eq!(exec_program("\"/tmp/app.AppImage %U"), None);
    }
    #[test]
    fn similarly_named_program_is_not_equal() {
        assert_ne!(
            exec_program("/tmp/deckcraft-helper.AppImage %F"),
            Some(PathBuf::from("/tmp/deckcraft.AppImage"))
        );
    }
    #[test]
    fn discovers_preexisting_duplicates_and_rejects_symlink_and_similar_exec() {
        let root = root("discovery");
        let apps = root.join("applications");
        fs::create_dir_all(&apps).unwrap();
        let executable = root.join("deck craft.AppImage");
        fs::write(&executable, b"\x7fELF").unwrap();
        let icon = root.join("deck.png");
        fs::write(&icon, b"\x89PNG\r\n\x1a\nimage").unwrap();
        fs::write(
            apps.join("external.desktop"),
            format!(
                "[Desktop Entry]\nExec=\"{}\" %U\nIcon={}\n",
                executable.display(),
                icon.display()
            ),
        )
        .unwrap();
        fs::write(
            apps.join("owned.desktop"),
            format!(
                "[Desktop Entry]\nX-SuiteCraft-AppId=deckcraft\nExec=\"{}\" %F\nIcon={}\n",
                executable.display(),
                icon.display()
            ),
        )
        .unwrap();
        fs::write(
            apps.join("similar.desktop"),
            format!("[Desktop Entry]\nExec={}x %F\n", executable.display()),
        )
        .unwrap();
        symlink(apps.join("external.desktop"), apps.join("linked.desktop")).unwrap();
        let found =
            matching_desktops_in("deckcraft", &executable, vec![(apps.clone(), true)]).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found.iter().filter(|item| item.suitecraft_owned).count(), 1);
        let references = icon_references_in(vec![apps]);
        assert_eq!(references.get(&fs::canonicalize(icon).unwrap()), Some(&2));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ignores_exec_from_desktop_action_sections() {
        let fields = parse_desktop(
            "[Desktop Entry]\nExec=/real/app\n[Desktop Action New]\nExec=/other/app\n",
        );
        assert_eq!(fields.get("Exec"), Some(&"/real/app"));
    }
}
