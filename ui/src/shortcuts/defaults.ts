// Raccourcis clavier par défaut (charte §7.3). Reconfigurables plus tard via les réglages.
export type ShortcutAction =
  | "mode.toggle"
  | "mode.toggle.force"
  | "tab.offload"
  | "tab.media"
  | "tab.player"
  | "tab.sync"
  | "tab.transcode"
  | "tab.report";

export interface Shortcut {
  key: string; // valeur de KeyboardEvent.key
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export const DEFAULT_SHORTCUTS: Record<ShortcutAction, Shortcut> = {
  "mode.toggle": { key: "Tab" }, // ignoré quand un champ de saisie a le focus
  "mode.toggle.force": { key: "Tab", ctrl: true }, // fonctionne partout
  "tab.offload": { key: "1", alt: true },
  "tab.media": { key: "2", alt: true },
  "tab.player": { key: "3", alt: true },
  "tab.sync": { key: "4", alt: true },
  "tab.transcode": { key: "5", alt: true },
  "tab.report": { key: "6", alt: true },
};
