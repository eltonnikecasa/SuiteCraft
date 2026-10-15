import { invoke } from "@tauri-apps/api/core";
import type { CatalogApp, InstalledApp, ReleaseInfo } from "./types";
import type { LanguagePreference } from "./i18n";
export const isTauri = () => "__TAURI_INTERNALS__" in window;
export const api = {
  releases: (apps:CatalogApp[], force=false) => invoke<Record<string,ReleaseInfo>>("check_releases", { apps, force }),
  installed: () => invoke<InstalledApp[]>("list_installed"),
  install: (app:CatalogApp, release:ReleaseInfo, installRoot:string) => invoke<InstalledApp>("install_app", { app, release, installRoot }),
  launch: (appId:string) => invoke<void>("launch_app", { appId }),
  remove: (appId:string) => invoke<void>("remove_app", { appId }),
  inspectDirectory: (path:string) => invoke<{writable:boolean; reason?:string}>("inspect_install_directory", { path }),
  homeInstallRoot: () => invoke<string>("suggest_user_install_root"),
  appIcon: (appId:string) => invoke<string | null>("get_app_icon", { appId }),
  languagePreference: () => invoke<LanguagePreference>("get_language_preference"),
  setLanguagePreference: (language:LanguagePreference) => invoke<void>("set_language_preference", { language })
};
