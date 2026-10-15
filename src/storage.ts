import type { CatalogApp } from "./types";
const KEY = "suitecraft.custom-catalog.v1";
export function loadCustomCatalog(): CatalogApp[] { try { const value=JSON.parse(localStorage.getItem(KEY) ?? "[]"); return Array.isArray(value) ? value : []; } catch { return []; } }
export function saveCustomCatalog(apps:CatalogApp[]) { localStorage.setItem(KEY, JSON.stringify(apps)); }
