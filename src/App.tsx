import { useEffect, useMemo, useState } from "react";
import {
  Box, Check, Download, ExternalLink, FolderCog, LoaderCircle, PackageOpen,
  Eye, EyeOff, Pencil, Play, Plus, RefreshCw, Search, Settings, Trash2, X
} from "lucide-react";
import { api, isTauri } from "./api";
import { cardState, hasCatalogConflict, manualId, mergeCatalog, newer, normalizeGithubUrl, overrideDefaultEntry, removeManualEntry, setCatalogDisabled } from "./catalog";
import { resolveLocale, translate, type LanguagePreference, type TranslationKey } from "./i18n";
import { loadCatalogState, saveCatalogState } from "./storage";
import type { Activity, CatalogApp, InstalledApp, ReleaseInfo } from "./types";
import "./icon.css";
import "./catalog.css";

type Filter = "all" | "installed" | "available" | "updates";
const filterKeys: Record<Filter, TranslationKey> = {
  all: "nav.all", installed: "nav.installed", available: "nav.available", updates: "nav.updates"
};
export default function App() {
  const [preference, setPreference] = useState<LanguagePreference>("system");
  const locale = resolveLocale(preference, navigator.language);
  const t = (key: TranslationKey, values?: Record<string, string | number>) => translate(locale, key, values);
  const [catalogState, setCatalogState] = useState(loadCatalogState);
  const [installed, setInstalled] = useState<InstalledApp[]>([]);
  const [installedIcons, setInstalledIcons] = useState<Record<string, string>>({});
  const [releases, setReleases] = useState<Record<string, ReleaseInfo>>({});
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [settings, setSettings] = useState(false);
  const [entryName, setEntryName] = useState("");
  const [repo, setRepo] = useState("");
  const [editingId, setEditingId] = useState<string>();
  const [root, setRoot] = useState(() => localStorage.getItem("suitecraft.install-root") || "");
  const [rootWritable, setRootWritable] = useState(false);
  const [activity, setActivity] = useState<Record<string, Activity>>({});
  const [notice, setNotice] = useState("");
  const allApps = useMemo(() => mergeCatalog(catalogState), [catalogState]);
  const apps = useMemo(() => allApps.filter(app => !app.disabled), [allApps]);
  const installedMap = useMemo(() => Object.fromEntries(installed.map(item => [item.appId, item])), [installed]);
  const hasUpdate = (id: string) => installedMap[id] && releases[id] && newer(releases[id].version, installedMap[id].version);
  const description = (app: CatalogApp) => t((app.origin === "manual" ? "catalog.custom.description" : `catalog.${app.id}.description`) as TranslationKey);
  const category = (app: CatalogApp) => t(app.origin === "manual" ? "catalog.category.custom" : (app.id === "solidcraft" || app.id === "cadcraft") ? "catalog.category.engineering" : "catalog.category.creativity");
  const visible = apps.filter(app => {
    const hit = `${app.name} ${description(app)}`.toLowerCase().includes(query.toLowerCase());
    return hit && (filter === "all" || (filter === "installed" && installedMap[app.id]) || (filter === "available" && !installedMap[app.id]) || (filter === "updates" && hasUpdate(app.id)));
  });

  const inspect = (value: string) => {
    if (!isTauri()) { setRootWritable(false); return; }
    api.inspectDirectory(value).then(result => setRootWritable(result.writable)).catch(() => setRootWritable(false));
  };
  const refreshIcons = async (items: InstalledApp[]) => {
    const entries = await Promise.all(items.map(async item => [item.appId, await api.appIcon(item.appId).catch(() => null)] as const));
    setInstalledIcons(Object.fromEntries(entries.filter((entry): entry is readonly [string, string] => Boolean(entry[1]))));
  };
  const refresh = async (force = false) => {
    if (!isTauri()) { setNotice(t("error.preview")); return; }
    const configured = apps.filter(app => app.repositoryUrl);
    configured.forEach(app => setActivity(state => ({ ...state, [app.id]: { appId: app.id, kind: "loading" } })));
    try { setReleases(await api.releases(configured, force)); }
    catch { setNotice(t("error.releases")); }
    finally { setActivity({}); }
  };
  const refreshOne = async (app: CatalogApp) => {
    if (!app.repositoryUrl || !isTauri()) return;
    setActivity(state => ({ ...state, [app.id]: { appId: app.id, kind: "loading" } }));
    try {
      const result = await api.releases([app], true);
      setReleases(state => ({ ...state, ...result }));
    } catch (error) { setNotice(String(error)); }
    finally { setActivity(state => { const next = { ...state }; delete next[app.id]; return next; }); }
  };
  useEffect(() => {
    if (isTauri()) {
      if (root) inspect(root);
      else api.homeInstallRoot().then(path => { setRoot(path); localStorage.setItem("suitecraft.install-root", path); inspect(path); }).catch(() => setRootWritable(false));
      api.languagePreference().then(value => setPreference(value)).catch(() => setPreference("system"));
      api.installed().then(items => { setInstalled(items); void refreshIcons(items); }).catch(() => undefined);
      api.releases(apps.filter(app => app.repositoryUrl)).then(setReleases).catch(() => undefined);
    }
  }, []); // Initial desktop state only.

  const changeLanguage = async (value: LanguagePreference) => {
    setPreference(value);
    if (isTauri()) {
      try { await api.setLanguagePreference(value); }
      catch { setNotice(translate(resolveLocale(value, navigator.language), "error.languageSave")); }
    }
  };
  const storeCatalog = (next: typeof catalogState) => { setCatalogState(next); saveCatalogState(next); };
  const resetEditor = () => { setEntryName(""); setRepo(""); setEditingId(undefined); };
  const saveEntry = () => {
    const editing = allApps.find(app => app.id === editingId);
    const normalized = repo.trim() ? normalizeGithubUrl(repo) : null;
    if (repo.trim() && !normalized) { setNotice(t("error.invalidRepository")); return; }
    if (!editing && !normalized) { setNotice(t("error.invalidRepository")); return; }
    const id = editing?.id ?? manualId(normalized!);
    if (hasCatalogConflict(allApps, id, normalized ?? undefined, editingId)) { setNotice(t("error.duplicateRepository")); return; }
    const name = entryName.trim() || editing?.name || normalized!.split("/").at(-1)!;
    if (editing?.origin === "default") {
      storeCatalog(overrideDefaultEntry(catalogState, id, name, normalized));
      resetEditor(); setNotice(t("status.catalogUpdated")); return;
    }
    const entry: CatalogApp = {
      schemaVersion: 1, id, name, description: "custom", category: "custom", repositoryUrl: normalized ?? undefined,
      platform: "linux-x86_64", packageType: "appimage",
      assetRule: { extensions: [".appimage"], include: ["x86_64", "amd64"], exclude: ["arm", "aarch64", "zsync", "sha256", "debug"], architecture: "x86_64" },
      origin: "manual"
    };
    storeCatalog({ ...catalogState, manual: editingId ? catalogState.manual.map(item => item.id === editingId ? entry : item) : [...catalogState.manual, entry] });
    resetEditor(); setNotice(t(editingId ? "status.catalogUpdated" : "status.customAdded"));
  };
  const editEntry = (app: CatalogApp) => { setEditingId(app.id); setEntryName(app.name); setRepo(app.repositoryUrl || ""); };
  const toggleEntry = (app: CatalogApp) => {
    storeCatalog(setCatalogDisabled(catalogState, app.id, !app.disabled));
  };
  const deleteManual = (app: CatalogApp) => {
    if (!window.confirm(t("confirm.removeCatalog", { name: app.name }))) return;
    storeCatalog(removeManualEntry(catalogState, app.id));
  };
  const install = async (app: CatalogApp) => {
    const release = releases[app.id];
    if (!release?.asset || !rootWritable) return;
    const updating = Boolean(installedMap[app.id]);
    if (!window.confirm(t(updating ? "confirm.update" : "confirm.install", { name: app.name }))) return;
    setActivity(state => ({ ...state, [app.id]: { appId: app.id, kind: "installing", message: t("status.downloading") } }));
    try {
      const item = await api.install(app, release, root);
      setInstalled(state => [...state.filter(value => value.appId !== app.id), item]);
      const icon = await api.appIcon(app.id).catch(() => null);
      setInstalledIcons(state => { const next = { ...state }; if (icon) next[app.id] = icon; else delete next[app.id]; return next; });
      setNotice(t(updating ? "status.updated" : "status.installed", { name: app.name }));
    } catch (error) { setNotice(`${t("error.install", { name: app.name })} ${String(error)}`); }
    finally { setActivity(state => { const next = { ...state }; delete next[app.id]; return next; }); }
  };
  const removeInstalled = async (app: CatalogApp) => {
    if (!window.confirm(t("confirm.remove", { name: app.name }))) return;
    try { await api.remove(app.id); setInstalled(state => state.filter(item => item.appId !== app.id)); setInstalledIcons(state => { const next = { ...state }; delete next[app.id]; return next; }); setNotice(t("status.removed", { name: app.name })); }
    catch { setNotice(t("error.remove", { name: app.name })); }
  };

  return <div className="shell">
    <aside>
      <div className="brand"><span className="logo"><Box /></span><div><strong>SuiteCraft</strong><small>{t("app.subtitle")}</small></div></div>
      <nav>{(Object.keys(filterKeys) as Filter[]).map(id =>
        <button className={filter === id ? "active" : ""} onClick={() => setFilter(id)} key={id}>
          {id === "all" ? <PackageOpen /> : id === "installed" ? <Check /> : id === "available" ? <Download /> : <RefreshCw />}
          <span>{t(filterKeys[id])}</span>{id === "updates" && apps.filter(app => hasUpdate(app.id)).length > 0 && <b>{apps.filter(app => hasUpdate(app.id)).length}</b>}
        </button>)}</nav>
      <div className="aside-note"><span></span><div><strong>Fedora · Linux</strong><small>AppImages x86_64</small></div></div>
    </aside>
    <main>
      <header><div><p>{t("header.catalog")}</p><h1>{t(filterKeys[filter])}</h1></div>
        <div className="header-actions">
          <label className="search"><Search /><input value={query} onChange={event => setQuery(event.target.value)} placeholder={t("search.placeholder")} aria-label={t("search.placeholder")} /></label>
          <button className="icon-button" title={t("a11y.refresh")} aria-label={t("a11y.refresh")} onClick={() => refresh(true)}><RefreshCw /></button>
          <button className="icon-button" title={t("a11y.settings")} aria-label={t("a11y.settings")} onClick={() => setSettings(true)}><Settings /></button>
        </div>
      </header>
      {notice && <div className="notice"><span>{notice}</span><button aria-label={t("a11y.close")} onClick={() => setNotice("")}><X /></button></div>}
      <section className="summary">
        <div><span>{t("summary.apps")}</span><strong>{apps.length}</strong></div>
        <div><span>{t("summary.installed")}</span><strong>{installed.length}</strong></div>
        <div><span>{t("summary.updates")}</span><strong>{apps.filter(app => hasUpdate(app.id)).length}</strong></div>
        <button onClick={() => refresh(true)}><RefreshCw /> {t("action.checkUpdates")}</button>
      </section>
      <section className="grid">{visible.map(app => {
        const local = installedMap[app.id], release = releases[app.id], busy = activity[app.id];
        const state = cardState(local, release, Boolean(busy));
        const reason = !app.repositoryUrl ? t("card.sourcePending") : release?.reason || (!release ? t("card.releasePending") : !release.asset ? t("card.assetUnavailable") : !rootWritable ? t("settings.directoryBlocked") : undefined);
        const stateLabel = !reason && state === "install" ? t("card.stateReady") : !reason && state === "open" ? t("card.stateCurrent") : !reason && state === "update" ? t("card.stateUpdate") : undefined;
        return <article className={!local ? "muted-app" : ""} key={app.id}>
          <div className="card-top"><div className="app-icon">{(installedIcons[app.id] || app.iconUrl) && <img src={installedIcons[app.id] || app.iconUrl} alt="" />}</div><span className="category">{category(app)}</span>{app.origin === "manual" && <span className="custom">{t("card.custom")}</span>}</div>
          <h2>{app.name}</h2><p>{description(app)}</p>
          <div className="versions"><span>{t("card.installedVersion")} <b>{local?.version || "—"}</b></span><span>{t("card.latestVersion")} <b>{release?.version || "—"}</b></span></div>
          {state === "checking" ? <button className="primary" disabled><LoaderCircle className="spin" /> {busy?.message || t("status.checking")}</button>
            : state === "retry" ? <button className="primary" onClick={() => refreshOne(app)}><RefreshCw />{t("action.retry")}</button>
            : state === "update" ? <div className="actions"><button className="primary" disabled={!rootWritable} title={!rootWritable ? t("settings.directoryBlocked") : undefined} onClick={() => install(app)}><Download />{t("action.update")}</button><button className="danger-icon" title={t("action.remove")} aria-label={t("action.remove")} onClick={() => removeInstalled(app)}><Trash2 /></button></div>
            : state === "open" ? <div className="actions"><button className="secondary" onClick={() => api.launch(app.id).catch(error => setNotice(`${t("error.launch", { name: app.name })} ${String(error)}`))}><Play />{t("action.open")}</button><button className="danger-icon" title={t("action.remove")} aria-label={t("action.remove")} onClick={() => removeInstalled(app)}><Trash2 /></button></div>
            : <button className="primary" disabled={!rootWritable || !release?.asset} title={reason} onClick={() => install(app)}><Download />{t("action.install")}</button>}
          {reason && <small className="reason">{reason}</small>}
          {stateLabel && <small className="reason">{stateLabel}</small>}
        </article>;
      })}
      {visible.length === 0 && <div className="empty"><PackageOpen /><h2>{t("empty.title")}</h2><p>{t("empty.description")}</p></div>}</section>
    </main>
    {settings && <div className="overlay" onMouseDown={event => event.target === event.currentTarget && setSettings(false)}>
      <section className="panel">
        <div className="panel-head"><div><p>{t("settings.eyebrow")}</p><h2>{t("settings.title")}</h2></div><button className="icon-button" aria-label={t("a11y.close")} onClick={() => setSettings(false)}><X /></button></div>
        <div className="setting"><div className="setting-title"><Settings /><div><strong>{t("settings.language")}</strong><small>{t("settings.languageHelp")}</small></div></div>
          <select className="language-select" value={preference} onChange={event => changeLanguage(event.target.value as LanguagePreference)} aria-label={t("settings.language")}>
            <option value="system">{t("language.system")}</option><option value="pt-BR">{t("language.pt-BR")}</option><option value="en">{t("language.en")}</option>
          </select>
        </div>
        <div className="setting"><div className="setting-title"><FolderCog /><div><strong>{t("settings.installDirectory")}</strong><small>{t("settings.optHelp")}</small></div></div>
          <div className="input-row"><input value={root} onChange={event => setRoot(event.target.value)} aria-label={t("settings.installDirectory")} /><button onClick={() => { localStorage.setItem("suitecraft.install-root", root); inspect(root); }}>{t("action.apply")}</button></div>
          <div className={rootWritable ? "ok" : "warning"}>{t(rootWritable ? "settings.directoryWritable" : "settings.directoryBlocked")}</div>
          <button className="link-button" onClick={async () => { if (isTauri()) { const path = await api.homeInstallRoot(); setRoot(path); localStorage.setItem("suitecraft.install-root", path); inspect(path); } }}>{t("settings.useUserDirectory")}</button>
        </div>
        <div className="setting"><div className="setting-title"><Plus /><div><strong>{t(editingId ? "settings.editRepository" : "settings.addRepository")}</strong><small>{t("settings.repositoryHelp")}</small></div></div>
          <div className="catalog-fields"><input value={entryName} onChange={event => setEntryName(event.target.value)} placeholder={t("settings.namePlaceholder")} aria-label={t("settings.namePlaceholder")} /><input value={repo} onChange={event => setRepo(event.target.value)} placeholder={t("settings.repositoryPlaceholder")} aria-label={t(editingId ? "settings.editRepository" : "settings.addRepository")} /><button onClick={saveEntry}>{t(editingId ? "action.save" : "action.add")}</button></div>
          {editingId && <button className="link-button" onClick={resetEditor}>{t("action.cancelEdit")}</button>}
        </div>
        <div className="custom-list"><strong>{t("settings.catalogEntries")}</strong>{allApps.map(app =>
          <div className={app.disabled ? "catalog-disabled" : ""} key={app.id}><span>{app.name}<small>{app.id} · {app.origin === "default" ? t("card.default") : t("card.custom")} · {app.repositoryUrl || t("card.sourceMissing")}</small></span>
            {app.repositoryUrl && <a href={app.repositoryUrl} target="_blank" rel="noreferrer" aria-label={t("a11y.openRepository", { name: app.name })} title={t("a11y.openRepository", { name: app.name })}><ExternalLink /></a>}
            <button title={t(app.disabled ? "action.enable" : "action.disable")} aria-label={t(app.disabled ? "action.enable" : "action.disable")} onClick={() => toggleEntry(app)}>{app.disabled ? <Eye /> : <EyeOff />}</button>
            <button title={t("action.edit")} aria-label={t("action.edit")} onClick={() => editEntry(app)}><Pencil /></button>
            {app.origin === "manual" && <button title={t("action.remove")} aria-label={t("action.remove")} onClick={() => deleteManual(app)}><Trash2 /></button>}
          </div>)}</div>
      </section>
    </div>}
  </div>;
}
