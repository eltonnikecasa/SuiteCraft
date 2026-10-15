export type LauncherAction = "install" | "open" | "update" | "refresh" | "configure" | "remove";

export function launcherActions(installed: boolean, installable: boolean, updateAvailable: boolean): LauncherAction[] {
  if (!installed) return [...(installable ? ["install" as const] : []), "refresh", "configure"];
  return ["open", ...(updateAvailable ? ["update" as const] : []), "refresh", "configure", "remove"];
}

export function fallbackLabel(name: string): string {
  return name.trim().split(/\s+/).map(part => part[0]).join("").slice(0, 2).toUpperCase() || "?";
}

export function isContextMenuKey(key: string, shiftKey: boolean): boolean {
  return key === "ContextMenu" || (key === "F10" && shiftKey);
}
