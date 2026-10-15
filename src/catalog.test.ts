import { describe, expect, it } from "vitest";
import { cardState, embeddedCatalog, hasCatalogConflict, mergeCatalog, overrideDefaultEntry, removeManualEntry, setCatalogDisabled } from "./catalog";
import { emptyCatalogState, parseCatalogState } from "./storage";
import type { CatalogApp, InstalledApp, ReleaseInfo } from "./types";

const manual: CatalogApp = { ...embeddedCatalog[0], id: "custom-acme-tool", name: "Tool", repositoryUrl: "https://github.com/acme/tool", origin: "manual" };
const installed: InstalledApp = { appId: manual.id, version: "1.0.0", executablePath: "/tmp/tool.AppImage", installedAt: "now" };
const release = (version: string, reason?: string): ReleaseInfo => ({ version, releaseUrl: "", cached: false, reason, asset: version ? { name: "tool-x86_64.AppImage", url: "https://github.com/a", size: 1 } : undefined });

describe("unified catalog", () => {
  it("contains only the thirteen API-validated default repositories", () => {
    expect(embeddedCatalog).toHaveLength(13);
    expect(embeddedCatalog.every(app => app.repositoryUrl === `https://github.com/${app.id === "solidcraft" ? "bherbruck" : "storytold"}/${app.id}`)).toBe(true);
  });
  it("merges defaults and manual entries and persists default edits/disabled state", () => {
    const state = { manual: [manual], defaultOverrides: { deckcraft: { name: "Deck edited", repositoryUrl: "https://github.com/acme/deck" } }, disabledIds: ["deckcraft", manual.id] };
    const apps = mergeCatalog(state);
    expect(apps).toHaveLength(embeddedCatalog.length + 1);
    expect(apps.find(app => app.id === "deckcraft")).toMatchObject({ name: "Deck edited", disabled: true, origin: "default" });
    expect(apps.find(app => app.id === manual.id)).toMatchObject({ disabled: true, origin: "manual" });
    expect(parseCatalogState(JSON.stringify(state))).toEqual(state);
  });
  it("migrates the legacy manual list and detects duplicate IDs or repositories", () => {
    const state = parseCatalogState(null, JSON.stringify([manual]));
    expect(state.manual[0].origin).toBe("manual");
    expect(hasCatalogConflict([...embeddedCatalog, manual], "another", "https://github.com/acme/tool")).toBe(true);
    expect(hasCatalogConflict([...embeddedCatalog, manual], "deckcraft", undefined)).toBe(true);
    expect(emptyCatalogState().manual).toEqual([]);
    expect(mergeCatalog({ ...emptyCatalogState(), manual: [manual, { ...manual, id: "duplicate-repository" }] }).filter(app => app.origin === "manual")).toHaveLength(1);
  });
  it("edits/disables defaults and removes manual entries without touching unrelated state", () => {
    const initial = { ...emptyCatalogState(), manual: [manual] };
    const edited = overrideDefaultEntry(initial, "deckcraft", "Deck 2", "https://github.com/acme/deck");
    const disabled = setCatalogDisabled(edited, "deckcraft", true);
    expect(mergeCatalog(disabled).find(app => app.id === "deckcraft")).toMatchObject({ name: "Deck 2", disabled: true });
    const removed = removeManualEntry(setCatalogDisabled(disabled, manual.id, true), manual.id);
    expect(removed.manual).toEqual([]);
    expect(removed.disabledIds).toEqual(["deckcraft"]);
  });
});

describe("card actions", () => {
  it("maps real release and installation state to the correct action", () => {
    expect(cardState(undefined, undefined, true)).toBe("checking");
    expect(cardState(undefined, release("", "network failed"), false)).toBe("retry");
    expect(cardState(undefined, release("1.0.0"), false)).toBe("install");
    expect(cardState(installed, release("1.0.0"), false)).toBe("open");
    expect(cardState(installed, release("1.1.0"), false)).toBe("update");
  });
});
