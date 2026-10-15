export type PackageType = "appimage";
export type Platform = "linux-x86_64";

export interface AssetRule { extensions: string[]; include: string[]; exclude: string[]; architecture: "x86_64" }
export interface CatalogApp {
  schemaVersion: 1; id: string; name: string; description: string; category: string;
  repositoryUrl?: string; iconUrl?: string; platform: Platform; packageType: PackageType;
  assetRule: AssetRule; custom?: boolean;
}
export interface ReleaseAsset { name: string; url: string; size: number; checksum?: string }
export interface ReleaseInfo { version: string; releaseUrl: string; publishedAt?: string; asset?: ReleaseAsset; reason?: string; cached: boolean }
export interface InstalledApp { appId: string; version: string; executablePath: string; installedAt: string }
export type Activity = { appId: string; kind: "loading" | "installing" | "error"; message?: string; progress?: number };
