// Traductions de l'interface. Ajouter une langue = ajouter un fichier JSON ici.
import fr from "./fr.json";
import en from "./en.json";

export const LANGUAGES = { fr, en } as const;
export type Language = keyof typeof LANGUAGES;
type Key = keyof typeof fr;

const STORAGE_KEY = "veriflow.language";

function initialLanguage(): Language {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && saved in LANGUAGES) return saved as Language;
  } catch {
    /* stockage indisponible : on retombe sur la langue du système */
  }
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export const i18n = $state({ lang: initialLanguage() });

export function setLanguage(lang: Language) {
  i18n.lang = lang;
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    /* sans effet si le stockage est indisponible */
  }
}

/** Traduit une clé ; renvoie la clé elle-même si elle est absente. */
export function t(key: Key | string): string {
  const table = LANGUAGES[i18n.lang] as Record<string, string>;
  return table[key] ?? (fr as Record<string, string>)[key] ?? key;
}
