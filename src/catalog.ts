import type { CatalogApp } from "./types";

const names = ["DeckCraft","SoundCraft","GridCraft","FilmCraft","WordCraft","PhotoCraft","VectorCraft","LightCraft","EffectCraft","DesignCraft","PdfCraft","CADCraft"];
const base = (name:string, repositoryUrl?:string): CatalogApp => ({ schemaVersion:1, id:name.toLowerCase(), name, description:name.toLowerCase(), category:name === "SolidCraft" || name === "CADCraft" ? "engineering" : "creativity", repositoryUrl, platform:"linux-x86_64", packageType:"appimage", assetRule:{ extensions:[".appimage"], include:["x86_64","amd64"], exclude:["arm","aarch64","zsync","sha256","debug"], architecture:"x86_64" } });
export const embeddedCatalog: CatalogApp[] = [...names.map(n=>base(n)), base("SolidCraft","https://github.com/bherbruck/solidcraft")];
export const normalizeGithubUrl = (url:string) => {
  const match = url.trim().match(/^https:\/\/github\.com\/([\w.-]+)\/([\w.-]+?)(?:\.git)?\/?$/i);
  return match ? `https://github.com/${match[1]}/${match[2]}` : null;
};
