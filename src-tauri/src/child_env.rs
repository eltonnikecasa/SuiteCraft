#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
const PATH_LIST_VARS: &[&str] = &[
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
    "XDG_DATA_DIRS",
    "GI_TYPELIB_PATH",
    "GTK_PATH",
    "QT_PLUGIN_PATH",
    "QML2_IMPORT_PATH",
    "QML_IMPORT_PATH",
];

#[cfg(target_os = "linux")]
const PATH_VALUE_VARS: &[&str] = &[
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GDK_PIXBUF_MODULE_FILE",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
    "QTWEBENGINEPROCESS_PATH",
];

#[cfg(target_os = "linux")]
fn belongs_to_mount(value: &OsStr, mount: &Path) -> bool {
    let path = Path::new(value);
    path.is_absolute() && path.starts_with(mount)
}

#[cfg(target_os = "linux")]
fn filtered_path_list(value: &OsStr, mount: &Path) -> Option<OsString> {
    let kept: Vec<PathBuf> = std::env::split_paths(value)
        .filter(|entry| !entry.as_os_str().is_empty() && !entry.starts_with(mount))
        .collect();
    (!kept.is_empty())
        .then(|| std::env::join_paths(kept).ok())
        .flatten()
}

#[cfg(target_os = "linux")]
fn trustworthy_original(name: &str, value: &OsStr, mount: &Path) -> bool {
    if value.is_empty() {
        return true;
    }
    if PATH_LIST_VARS.contains(&name) {
        return std::env::split_paths(value).all(|entry| !entry.starts_with(mount));
    }
    !belongs_to_mount(value, mount)
}

/// Produces an environment for a non-SuiteCraft process. Only AppImage runtime
/// additions belonging to the parent's mount are removed; session and user
/// variables pass through unchanged.
#[cfg(target_os = "linux")]
pub(crate) fn sanitized_environment<I>(environment: I) -> BTreeMap<OsString, OsString>
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let mut result: BTreeMap<OsString, OsString> = environment.into_iter().collect();
    let Some(mount) = result.get(OsStr::new("APPDIR")).map(PathBuf::from) else {
        return result;
    };
    if !mount.is_absolute() {
        return result;
    }

    let names: Vec<OsString> = result.keys().cloned().collect();
    for key in names {
        let Some(name) = key.to_str() else { continue };
        if let Some(original_name) = name.strip_prefix("APPIMAGE_ORIGINAL_") {
            if (PATH_LIST_VARS.contains(&original_name)
                || PATH_VALUE_VARS.contains(&original_name)
                || original_name == "LD_PRELOAD")
                && result
                    .get(&key)
                    .is_some_and(|value| trustworthy_original(original_name, value, &mount))
            {
                let value = result.get(&key).cloned().unwrap_or_default();
                if value.is_empty() {
                    result.remove(OsStr::new(original_name));
                } else {
                    result.insert(OsString::from(original_name), value);
                }
            }
            result.remove(&key);
        }
    }

    for name in PATH_LIST_VARS {
        if let Some(value) = result.get(OsStr::new(name)).cloned() {
            match filtered_path_list(&value, &mount) {
                Some(value) => {
                    result.insert(OsString::from(name), value);
                }
                None => {
                    result.remove(OsStr::new(name));
                }
            }
        }
    }
    for name in PATH_VALUE_VARS {
        if result
            .get(OsStr::new(name))
            .is_some_and(|value| belongs_to_mount(value, &mount))
        {
            result.remove(OsStr::new(name));
        }
    }
    if let Some(value) = result.get(OsStr::new("LD_PRELOAD")).cloned() {
        let kept = value
            .to_string_lossy()
            .split(|character: char| character == ':' || character.is_whitespace())
            .filter(|entry| !entry.is_empty() && !Path::new(entry).starts_with(&mount))
            .collect::<Vec<_>>()
            .join(":");
        if kept.is_empty() {
            result.remove(OsStr::new("LD_PRELOAD"));
        } else {
            result.insert(OsString::from("LD_PRELOAD"), OsString::from(kept));
        }
    }
    let parent_appimage = result.get(OsStr::new("APPIMAGE")).cloned();
    if result.get(OsStr::new("ARGV0")).is_some_and(|value| {
        belongs_to_mount(value, &mount)
            || parent_appimage
                .as_ref()
                .is_some_and(|appimage| Path::new(value) == Path::new(appimage))
    }) {
        result.remove(OsStr::new("ARGV0"));
    }
    result.remove(OsStr::new("APPDIR"));
    result.remove(OsStr::new("APPIMAGE"));
    result
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn configure(command: &mut tokio::process::Command) {
    let _ = command;
}

#[cfg(target_os = "linux")]
pub(crate) fn configure(command: &mut tokio::process::Command) {
    command
        .env_clear()
        .envs(sanitized_environment(std::env::vars_os()));
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn env(values: &[(&str, &str)]) -> BTreeMap<OsString, OsString> {
        sanitized_environment(
            values
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        )
    }

    #[test]
    fn removes_only_parent_appimage_paths() {
        let clean = env(&[
            ("APPDIR", "/tmp/.mount_SuiteC123"),
            ("APPIMAGE", "/downloads/SuiteCraft.AppImage"),
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_SuiteC123/usr/lib:/home/me/lib:/usr/lib",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_SuiteC123/usr/share:/home/me/.local/share:/usr/share",
            ),
            (
                "GTK_IM_MODULE_FILE",
                "/tmp/.mount_SuiteC123/usr/lib/gtk.immodules",
            ),
            ("QT_STYLE_OVERRIDE", "adwaita"),
        ]);
        assert_eq!(
            clean.get(OsStr::new("LD_LIBRARY_PATH")).unwrap(),
            "/home/me/lib:/usr/lib"
        );
        assert_eq!(
            clean.get(OsStr::new("XDG_DATA_DIRS")).unwrap(),
            "/home/me/.local/share:/usr/share"
        );
        assert!(!clean.contains_key(OsStr::new("GTK_IM_MODULE_FILE")));
        assert_eq!(
            clean.get(OsStr::new("QT_STYLE_OVERRIDE")).unwrap(),
            "adwaita"
        );
        assert!(!clean.contains_key(OsStr::new("APPDIR")));
    }

    #[test]
    fn clean_non_appimage_environment_is_unchanged() {
        let values = [
            (OsString::from("HOME"), OsString::from("/home/me")),
            (OsString::from("PATH"), OsString::from("/usr/bin")),
        ];
        assert_eq!(
            sanitized_environment(values.clone()),
            values.into_iter().collect()
        );
    }

    #[test]
    fn restores_trustworthy_original_values() {
        let clean = env(&[
            ("APPDIR", "/tmp/.mount_SuiteC123"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_SuiteC123/usr/lib"),
            ("APPIMAGE_ORIGINAL_LD_LIBRARY_PATH", "/home/me/lib:/usr/lib"),
            ("APPIMAGE_ORIGINAL_GTK_PATH", ""),
        ]);
        assert_eq!(
            clean.get(OsStr::new("LD_LIBRARY_PATH")).unwrap(),
            "/home/me/lib:/usr/lib"
        );
        assert!(!clean.contains_key(OsStr::new("GTK_PATH")));
        assert!(!clean.contains_key(OsStr::new("APPIMAGE_ORIGINAL_LD_LIBRARY_PATH")));
    }

    #[test]
    fn preserves_graphical_session_and_locale() {
        let clean = env(&[
            ("APPDIR", "/tmp/.mount_SuiteC123"),
            ("HOME", "/home/me"),
            ("PATH", "/usr/bin"),
            ("LANG", "pt_BR.UTF-8"),
            ("DISPLAY", ":0"),
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1000/bus"),
            ("XDG_CURRENT_DESKTOP", "GNOME"),
        ]);
        for key in [
            "HOME",
            "PATH",
            "LANG",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_RUNTIME_DIR",
            "DBUS_SESSION_BUS_ADDRESS",
            "XDG_CURRENT_DESKTOP",
        ] {
            assert!(clean.contains_key(OsStr::new(key)), "missing {key}");
        }
    }
}
