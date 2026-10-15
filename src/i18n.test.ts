import { describe, expect, it } from "vitest";
import { catalogs, hasCompleteCatalog, resolveLocale, translate } from "./i18n";

describe("localization catalogs", () => {
  it("contain the same non-empty keys", () => {
    expect(hasCompleteCatalog("en")).toBe(true);
    expect(hasCompleteCatalog("pt-BR")).toBe(true);
    for (const catalog of Object.values(catalogs)) expect(Object.values(catalog).every(Boolean)).toBe(true);
  });
  it("detects Portuguese and falls back to English", () => {
    expect(resolveLocale("system", "pt-BR")).toBe("pt-BR");
    expect(resolveLocale("system", "pt-PT")).toBe("pt-BR");
    expect(resolveLocale("system", "fr-FR")).toBe("en");
    expect(resolveLocale("en", "pt-BR")).toBe("en");
  });
  it("interpolates values", () => expect(translate("en", "status.installed", { name: "DeckCraft" })).toContain("DeckCraft"));
});
