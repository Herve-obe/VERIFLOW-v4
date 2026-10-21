// Gestion centralisée du clavier : associe les touches aux actions.
import { DEFAULT_SHORTCUTS, PLAYER_SHORTCUTS, type PlayerAction, type Shortcut, type ShortcutAction } from "./defaults";
import { effectiveShortcut } from "../stores/prefs.svelte";

export function matches(e: KeyboardEvent, s: Shortcut | undefined): boolean {
  if (!s) return false;
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  return (
    key === s.key &&
    !!s.ctrl === (e.ctrlKey || e.metaKey) &&
    !!s.shift === e.shiftKey &&
    !!s.alt === e.altKey
  );
}

const NOT_TEXT = ["range", "checkbox", "radio", "button"];

/** Vrai si l'utilisateur est en train de saisir du texte. */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  // Barre de défilement, case à cocher... : pas de saisie, les raccourcis
  // (Espace pour lire) restent actifs après un clic dessus.
  if (target instanceof HTMLInputElement && NOT_TEXT.includes(target.type)) return false;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

/** Raccourci en vigueur (préférences, sinon défaut) d'une action d'onglet ou de mode. */
export const appShortcut = (a: ShortcutAction) => effectiveShortcut(a, DEFAULT_SHORTCUTS[a]);
/** Raccourci en vigueur d'une action de lecture. */
export const playerShortcut = (a: PlayerAction) => effectiveShortcut(a, PLAYER_SHORTCUTS[a]);

/** Retourne l'action correspondant à l'événement, ou null. */
export function resolveAction(e: KeyboardEvent): ShortcutAction | null {
  // Ordre important : la version forcée (Ctrl+Tab) est testée avant Tab seul.
  if (matches(e, appShortcut("mode.toggle.force"))) return "mode.toggle.force";
  if (matches(e, appShortcut("mode.toggle"))) {
    return isTyping(e.target) ? null : "mode.toggle";
  }
  for (const action of Object.keys(DEFAULT_SHORTCUTS) as ShortcutAction[]) {
    if (action.startsWith("mode.")) continue;
    if (matches(e, appShortcut(action))) return action;
  }
  return null;
}

/** Action de lecture correspondant à l'événement (hors saisie de texte), ou null. */
export function resolvePlayerAction(e: KeyboardEvent): PlayerAction | null {
  if (isTyping(e.target)) return null;
  for (const action of Object.keys(PLAYER_SHORTCUTS) as PlayerAction[]) {
    if (matches(e, playerShortcut(action))) return action;
  }
  return null;
}

/** Événement de fenêtre portant une action de lecture choisie dans le menu Lecture. */
export const PLAYER_ACTION_EVENT = "veriflow-player-action";

export function dispatchPlayerAction(action: PlayerAction) {
  window.dispatchEvent(new CustomEvent<PlayerAction>(PLAYER_ACTION_EVENT, { detail: action }));
}
