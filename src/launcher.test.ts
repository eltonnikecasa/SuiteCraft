import { describe, expect, it } from "vitest";
import { fallbackLabel, isContextMenuKey, launcherActions, safeLauncherActions } from "./launcher";

describe("launcher actions", () => {
  it("offers only valid actions for non-installed applications", () => {
    expect(launcherActions(false, true, false)).toEqual(["install", "refresh", "configure"]);
    expect(launcherActions(false, false, false)).toEqual(["refresh", "configure"]);
  });

  it("only offers update when a newer compatible release is confirmed", () => {
    expect(launcherActions(true, true, false)).toEqual(["open", "refresh", "configure", "remove"]);
    expect(launcherActions(true, true, true)).toEqual(["open", "update", "refresh", "configure", "remove"]);
  });

  it("does not infer availability while installed state is loading or failed", () => {
    expect(safeLauncherActions("loading", false, true, false)).toEqual(["configure"]);
    expect(safeLauncherActions("error", false, true, false)).toEqual(["configure"]);
    expect(safeLauncherActions("ready", true, true, false)).toContain("open");
  });

  it("provides a neutral fallback and recognizes keyboard context-menu gestures", () => {
    expect(fallbackLabel("Photo Craft")).toBe("PC");
    expect(fallbackLabel(" ")).toBe("?");
    expect(isContextMenuKey("ContextMenu", false)).toBe(true);
    expect(isContextMenuKey("F10", true)).toBe(true);
    expect(isContextMenuKey("F10", false)).toBe(false);
  });
});
