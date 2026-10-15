import en from "../i18n/en.json";
import ptBR from "../i18n/pt-BR.json";

export type SupportedLocale = "en" | "pt-BR";
export type LanguagePreference = "system" | SupportedLocale;
export type TranslationKey = keyof typeof en;
type Catalog = Record<TranslationKey, string>;

export const catalogs: Record<SupportedLocale, Catalog> = { en, "pt-BR": ptBR };

export function resolveLocale(preference: LanguagePreference, systemLocale: string): SupportedLocale {
  if (preference !== "system") return preference;
  return systemLocale.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

export function translate(locale: SupportedLocale, key: TranslationKey, values: Record<string, string | number> = {}): string {
  const template = catalogs[locale][key] ?? catalogs.en[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? `{${name}}`));
}

export function hasCompleteCatalog(locale: SupportedLocale): boolean {
  const reference = Object.keys(catalogs.en);
  const candidate = Object.keys(catalogs[locale]);
  return reference.length === candidate.length && reference.every(key => candidate.includes(key));
}
