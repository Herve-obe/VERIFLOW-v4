// Gestion centralisée du clavier : associe les touches aux actions.
import { DEFAULT_SHORTCUTS, type Shortcut, type ShortcutAction } from "./defaults";

function matches(e: KeyboardEvent, s: Shortcut): boolean {
  return (
    e.key === s.key &&
    !!s.ctrl === (e.ctrlKey || e.metaKey) &&
    !!s.shift === e.shiftKey &&
    !!s.alt === e.altKey
  );
}

/** Vrai si l'utilisateur est en train de saisir du texte. */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable;
}

/** Retourne l'action correspondant à l'événement, ou null. */
export function resolveAction(e: KeyboardEvent): ShortcutAction | null {
  // Ordre important : la version forcée (Ctrl+Tab) est testée avant Tab seul.
  if (matches(e, DEFAULT_SHORTCUTS["mode.toggle.force"])) return "mode.toggle.force";
  if (matches(e, DEFAULT_SHORTCUTS["mode.toggle"])) {
    return isTyping(e.target) ? null : "mode.toggle";
  }
  for (const [action, shortcut] of Object.entries(DEFAULT_SHORTCUTS)) {
    if (action.startsWith("mode.")) continue;
    if (matches(e, shortcut)) return action as ShortcutAction;
  }
  return null;
}
