import type { CatalogApp, CatalogOverride, CatalogState, InstalledApp, ReleaseInfo } from "./types";

const defaultRepositories: Record<string, string> = {
  deckcraft: "https://github.com/storytold/deckcraft",
  soundcraft: "https://github.com/storytold/soundcraft",
  gridcraft: "https://github.com/storytold/gridcraft",
  filmcraft: "https://github.com/storytold/filmcraft",
  wordcraft: "https://github.com/storytold/wordcraft",
  photocraft: "https://github.com/storytold/photocraft",
  vectorcraft: "https://github.com/storytold/vectorcraft",
  lightcraft: "https://github.com/storytold/lightcraft",
  effectcraft: "https://github.com/storytold/effectcraft",
  designcraft: "https://github.com/storytold/designcraft",
  pdfcraft: "https://github.com/storytold/pdfcraft",
  cadcraft: "https://github.com/storytold/cadcraft",
  solidcraft: "https://github.com/bherbruck/solidcraft"
};
const names = ["DeckCraft", "SoundCraft", "GridCraft", "FilmCraft", "WordCraft", "PhotoCraft", "VectorCraft", "LightCraft", "EffectCraft", "DesignCraft", "PdfCraft", "CADCraft", "SolidCraft"];
const base = (name: string, repositoryUrl?: string): CatalogApp => ({
  schemaVersion: 1, id: name.toLowerCase(), name, description: name.toLowerCase(),
  category: name === "SolidCraft" || name === "CADCraft" ? "engineering" : "creativity",
  repositoryUrl, platform: "linux-x86_64", packageType: "appimage",
  assetRule: { extensions: [".appimage"], include: ["x86_64", "amd64"], exclude: ["arm", "aarch64", "zsync", "sha256", "debug"], architecture: "x86_64" },
  origin: "default"
});
export const embeddedCatalog: CatalogApp[] = names.map(name => base(name, defaultRepositories[name.toLowerCase()]));

export const normalizeGithubUrl = (url: string) => {
  const match = url.trim().match(/^https:\/\/github\.com\/([\w.-]+)\/([\w.-]+?)(?:\.git)?\/?$/i);
  return match ? `https://github.com/${match[1]}/${match[2]}` : null;
};

export const manualId = (repositoryUrl: string) => {
  const parts = repositoryUrl.split("/");
  return `custom-${parts.at(-2)}-${parts.at(-1)}`.toLowerCase().replace(/[^a-z0-9-]/g, "-");
};

export function mergeCatalog(state: CatalogState): CatalogApp[] {
  const defaults = embeddedCatalog.map(app => {
    const override: CatalogOverride | undefined = state.defaultOverrides[app.id];
    return { ...app, ...override, repositoryUrl: override && "repositoryUrl" in override ? override.repositoryUrl ?? undefined : app.repositoryUrl, id: app.id, origin: "default" as const, disabled: state.disabledIds.includes(app.id) };
  });
  const ids = new Set(defaults.map(app => app.id));
  const repositories = new Set(defaults.map(app => app.repositoryUrl && normalizeGithubUrl(app.repositoryUrl)?.toLowerCase()).filter(Boolean));
  const manual = state.manual.filter(app => {
    const repository = app.repositoryUrl && normalizeGithubUrl(app.repositoryUrl)?.toLowerCase();
    if (ids.has(app.id) || Boolean(repository && repositories.has(repository))) return false;
    ids.add(app.id); if (repository) repositories.add(repository); return true;
  });
  return [...defaults, ...manual.map(app => ({ ...app, origin: "manual" as const, disabled: state.disabledIds.includes(app.id) }))];
}

export function hasCatalogConflict(apps: CatalogApp[], id: string, repositoryUrl: string | undefined, editingId?: string) {
  const normalized = repositoryUrl ? normalizeGithubUrl(repositoryUrl)?.toLowerCase() : undefined;
  return apps.some(app => app.id !== editingId && (app.id === id || Boolean(normalized && app.repositoryUrl && normalizeGithubUrl(app.repositoryUrl)?.toLowerCase() === normalized)));
}

export function setCatalogDisabled(state: CatalogState, id: string, disabled: boolean): CatalogState {
  return { ...state, disabledIds: disabled ? [...new Set([...state.disabledIds, id])] : state.disabledIds.filter(value => value !== id) };
}

export function removeManualEntry(state: CatalogState, id: string): CatalogState {
  return { ...state, manual: state.manual.filter(app => app.id !== id), disabledIds: state.disabledIds.filter(value => value !== id) };
}

export function overrideDefaultEntry(state: CatalogState, id: string, name: string, repositoryUrl: string | null): CatalogState {
  return { ...state, defaultOverrides: { ...state.defaultOverrides, [id]: { name, repositoryUrl } } };
}

export type CardState = "checking" | "retry" | "install" | "open" | "update";
export function cardState(installed: InstalledApp | undefined, release: ReleaseInfo | undefined, checking: boolean): CardState {
  if (checking) return "checking";
  if (release?.reason && !release.version) return "retry";
  if (!installed) return "install";
  if (release?.asset && newer(release.version, installed.version)) return "update";
  return "open";
}

const versionParts = (value: string) => value.replace(/^v/i, "").split(/[.-]/).map(part => /^\d+$/.test(part) ? Number(part) : part);
export function newer(remote: string, local: string) {
  const a = versionParts(remote), b = versionParts(local);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const x = a[i] ?? 0, y = b[i] ?? 0;
    if (x === y) continue;
    if (typeof x === "number" && typeof y === "number") return x > y;
    return String(x).localeCompare(String(y), undefined, { numeric: true }) > 0;
  }
  return false;
}
