import type { CatalogApp, CatalogState } from "./types";
const KEY = "suitecraft.catalog.v2";
const LEGACY_KEY = "suitecraft.custom-catalog.v1";
export const emptyCatalogState = (): CatalogState => ({ manual: [], defaultOverrides: {}, disabledIds: [] });
export function parseCatalogState(raw: string | null, legacyRaw: string | null = null): CatalogState {
  try {
    const value = JSON.parse(raw ?? "null") as Partial<CatalogState> | null;
    if (value && Array.isArray(value.manual) && value.defaultOverrides && Array.isArray(value.disabledIds)) return { manual: value.manual, defaultOverrides: value.defaultOverrides, disabledIds: [...new Set(value.disabledIds)] };
  } catch { /* Fall through to the legacy catalog. */ }
  try {
    const legacy = JSON.parse(legacyRaw ?? "[]") as CatalogApp[];
    if (Array.isArray(legacy)) return { ...emptyCatalogState(), manual: legacy.map(app => ({ ...app, origin: "manual" })) };
  } catch { /* Use an empty, valid state. */ }
  return emptyCatalogState();
}
export function loadCatalogState(): CatalogState { return parseCatalogState(localStorage.getItem(KEY), localStorage.getItem(LEGACY_KEY)); }
export function saveCatalogState(state: CatalogState) { localStorage.setItem(KEY, JSON.stringify(state)); }
