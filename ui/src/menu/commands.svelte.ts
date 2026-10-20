// Barre de menus : liste unique des commandes et des menus. Elle sert à la
// barre dessinée de Windows et de Linux (MenuBar.svelte) et au menu natif de
// macOS (src-tauri/src/menu.rs), avec les mêmes libellés et raccourcis.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { app, toggleMode, type Tab } from "../stores/app.svelte";
import { explorer, setExplorerCollapsed } from "../stores/explorer.svelte";
import { t } from "../i18n/index.svelte";
import { inTauri } from "../lib/api";
import {
  clearRecent,
  closeProject,
  createProject,
  openProject,
  openRecent,
  recent,
  saveProject,
  saveProjectAs,
} from "../lib/project.svelte";
import { dispatchPlayerAction } from "../shortcuts";
import { PLAYER_SHORTCUTS, type PlayerAction, type Shortcut } from "../shortcuts/defaults";

export type Platform = "macos" | "windows" | "linux";

function detect(): Platform {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Mac/i.test(ua)) return "macos";
  if (/Windows/i.test(ua)) return "windows";
  return "linux";
}

export const platform: Platform = detect();
export const isMac = platform === "macos";

/** Fenêtre ouverte depuis les menus. */
export type Dialog = "about" | "shortcuts" | "licenses" | "prefs";
export const menuState = $state({ dialog: null as Dialog | null });

export interface Command {
  id: string;
  label: () => string;
  shortcut?: Shortcut;
  /** Raccourci traité ici. Faux : déjà géré ailleurs (lecteur, onglets, champs
   *  de texte), le menu ne fait que l'afficher. */
  bind?: boolean;
  /** Entrée du système sous macOS (copier, coller...). */
  native?: string;
  enabled?: () => boolean;
  run: () => void;
}

export type Entry = string | "-" | { submenu: () => string; items: () => Entry[] };
export interface MenuDef {
  id: string;
  label: () => string;
  items: () => Entry[];
}

const hasProject = () => app.project !== null;
const show = (d: Dialog) => () => (menuState.dialog = d);

// Commandes d'édition sur le champ qui a le focus (la barre ne le lui prend
// pas). Le presse-papiers passe par Tauri : le moteur web refuse « coller »
// depuis un menu, et parfois « copier ».
function field(): HTMLInputElement | HTMLTextAreaElement | null {
  const el = document.activeElement;
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement ? el : null;
}

function selectedText(): string {
  const f = field();
  if (f) return f.value.slice(f.selectionStart ?? 0, f.selectionEnd ?? 0);
  return window.getSelection()?.toString() ?? "";
}

function edit(command: "undo" | "redo" | "cut" | "copy" | "paste" | "selectAll") {
  return async () => {
    try {
      if (command === "copy" || command === "cut") {
        const text = selectedText();
        if (!text) return;
        await (inTauri ? writeText(text) : navigator.clipboard.writeText(text));
        if (command === "cut" && field() && !field()!.readOnly) document.execCommand("delete");
      } else if (command === "paste") {
        const f = field();
        if (!f || f.readOnly || f.disabled) return;
        const text = await (inTauri ? readText() : navigator.clipboard.readText());
        if (text) document.execCommand("insertText", false, text);
      } else {
        document.execCommand(command);
      }
    } catch {
      app.status = t("menu.paste.blocked");
    }
  };
}

async function toggleFullscreen() {
  if (!inTauri) return;
  const w = getCurrentWindow();
  await w.setFullscreen(!(await w.isFullscreen()));
}

async function quit() {
  if (inTauri) await getCurrentWindow().close();
}

const tab = (id: Tab, key: string): Command => ({
  id: `view.tab.${id}`,
  label: () => id.toUpperCase(),
  shortcut: { key, alt: true },
  bind: false, // App.svelte
  run: () => (app.tab = id),
});

const play = (action: PlayerAction, label: string): Command => ({
  id: `play.${action}`,
  label: () => t(label as never),
  shortcut: PLAYER_SHORTCUTS[action],
  bind: false, // VideoView, AudioView
  run: () => dispatchPlayerAction(action),
});

const list: Command[] = [
  { id: "file.new", label: () => t("menu.file.new"), shortcut: { key: "n", ctrl: true }, run: () => createProject() },
  { id: "file.open", label: () => t("menu.file.open"), shortcut: { key: "o", ctrl: true }, run: () => openProject() },
  { id: "file.recent.clear", label: () => t("menu.file.recent.clear"), enabled: () => recent.paths.length > 0, run: clearRecent },
  { id: "file.save", label: () => t("menu.file.save"), shortcut: { key: "s", ctrl: true }, enabled: hasProject, run: () => saveProject() },
  {
    id: "file.saveas",
    label: () => t("menu.file.saveas"),
    shortcut: { key: "s", ctrl: true, shift: true },
    enabled: hasProject,
    run: () => saveProjectAs(),
  },
  { id: "file.close", label: () => t("menu.file.close"), shortcut: { key: "w", ctrl: true }, enabled: hasProject, run: () => closeProject() },
  { id: "file.quit", label: () => t("menu.file.quit"), shortcut: { key: "q", ctrl: true }, native: "quit", run: quit },

  { id: "edit.undo", label: () => t("menu.edit.undo"), shortcut: { key: "z", ctrl: true }, bind: false, native: "undo", run: edit("undo") },
  {
    id: "edit.redo",
    label: () => t("menu.edit.redo"),
    shortcut: isMac ? { key: "z", ctrl: true, shift: true } : { key: "y", ctrl: true },
    bind: false,
    native: "redo",
    run: edit("redo"),
  },
  { id: "edit.cut", label: () => t("menu.edit.cut"), shortcut: { key: "x", ctrl: true }, bind: false, native: "cut", run: edit("cut") },
  { id: "edit.copy", label: () => t("menu.edit.copy"), shortcut: { key: "c", ctrl: true }, bind: false, native: "copy", run: edit("copy") },
  { id: "edit.paste", label: () => t("menu.edit.paste"), shortcut: { key: "v", ctrl: true }, bind: false, native: "paste", run: edit("paste") },
  {
    id: "edit.selectall",
    label: () => t("menu.edit.selectall"),
    shortcut: { key: "a", ctrl: true },
    bind: false,
    native: "select_all",
    run: edit("selectAll"),
  },
  { id: "edit.prefs", label: () => t("menu.edit.prefs"), shortcut: { key: ",", ctrl: true }, run: show("prefs") },

  tab("offload", "1"),
  tab("media", "2"),
  tab("player", "3"),
  tab("sync", "4"),
  tab("transcode", "5"),
  tab("report", "6"),
  { id: "view.mode", label: () => t("menu.view.mode"), shortcut: { key: "Tab" }, bind: false, run: toggleMode },
  {
    id: "view.explorer",
    label: () => (explorer.collapsed ? t("menu.view.explorer.show") : t("menu.view.explorer.hide")),
    shortcut: { key: "b", ctrl: true },
    run: () => setExplorerCollapsed(!explorer.collapsed),
  },
  { id: "view.fullscreen", label: () => t("menu.view.fullscreen"), shortcut: { key: "F11" }, native: "fullscreen", run: toggleFullscreen },

  play("play.toggle", "menu.play.toggle"),
  play("play.stop", "menu.play.stop"),
  play("shuttle.back", "menu.play.back"),
  play("shuttle.pause", "menu.play.pause"),
  play("shuttle.forward", "menu.play.forward"),
  play("step.back", "menu.play.step.back"),
  play("step.forward", "menu.play.step.forward"),
  play("mark.in", "menu.play.in"),
  play("mark.out", "menu.play.out"),
  play("mark.add", "menu.play.marker"),
  play("report.edit", "menu.play.report"),

  { id: "help.shortcuts", label: () => t("menu.help.shortcuts"), shortcut: { key: "F1" }, run: show("shortcuts") },
  { id: "help.licenses", label: () => t("menu.help.licenses"), run: show("licenses") },
  { id: "help.about", label: () => t("menu.help.about"), run: show("about") },
];

export const COMMANDS: Record<string, Command> = Object.fromEntries(list.map((c) => [c.id, c]));

const RECENT_PREFIX = "file.recent:";

/** Commande d'un identifiant (y compris un projet récent). */
export function command(id: string): Command | null {
  if (id.startsWith(RECENT_PREFIX)) {
    const path = id.slice(RECENT_PREFIX.length);
    return { id, label: () => path, run: () => openRecent(path) };
  }
  return COMMANDS[id] ?? null;
}

export function runCommand(id: string) {
  const c = command(id);
  if (c && (c.enabled?.() ?? true)) c.run();
}

const recentEntries = (): Entry[] => [
  ...recent.paths.map((p) => RECENT_PREFIX + p),
  ...(recent.paths.length ? ["-"] : []),
  "file.recent.clear",
];

/** Les menus, dans l'ordre de la barre. Sous macOS, le menu de l'application
 *  (À propos, Préférences, Quitter) est ajouté par native.ts. */
export const MENUS: MenuDef[] = [
  {
    id: "file",
    label: () => t("menu.file"),
    items: () => [
      "file.new",
      "file.open",
      { submenu: () => t("menu.file.recent"), items: recentEntries },
      "-",
      "file.save",
      "file.saveas",
      "-",
      "file.close",
      ...(isMac ? [] : ["-", "file.quit"]),
    ],
  },
  {
    id: "edit",
    label: () => t("menu.edit"),
    items: () => [
      "edit.undo",
      "edit.redo",
      "-",
      "edit.cut",
      "edit.copy",
      "edit.paste",
      "edit.selectall",
      ...(isMac ? [] : ["-", "edit.prefs"]),
    ],
  },
  {
    id: "view",
    label: () => t("menu.view"),
    items: () => [
      "view.tab.offload",
      "view.tab.media",
      "view.tab.player",
      "view.tab.sync",
      "view.tab.transcode",
      "view.tab.report",
      "-",
      "view.mode",
      "view.explorer",
      "-",
      "view.fullscreen",
    ],
  },
  {
    id: "play",
    label: () => t("menu.play"),
    items: () => [
      "play.play.toggle",
      "play.play.stop",
      "-",
      "play.shuttle.back",
      "play.shuttle.pause",
      "play.shuttle.forward",
      "play.step.back",
      "play.step.forward",
      "-",
      "play.mark.in",
      "play.mark.out",
      "play.mark.add",
      "play.report.edit",
    ],
  },
  {
    id: "help",
    label: () => t("menu.help"),
    items: () => ["help.shortcuts", "help.licenses", ...(isMac ? [] : ["-", "help.about"])],
  },
];

// Noms des touches à l'écran.
function keyName(key: string): string {
  switch (key) {
    case " ":
      return t("key.space");
    case "Enter":
      return isMac ? "↩" : t("key.enter");
    case "ArrowLeft":
      return "←";
    case "ArrowRight":
      return "→";
    case "Tab":
      return isMac ? "⇥" : "Tab";
    default:
      return key.length === 1 ? key.toUpperCase() : key;
  }
}

/** Raccourci lisible : « Ctrl+Maj+S » (Windows, Linux) ou « ⇧⌘S » (macOS). */
export function shortcutLabel(s?: Shortcut): string {
  if (!s) return "";
  if (isMac) return `${s.alt ? "⌥" : ""}${s.shift ? "⇧" : ""}${s.ctrl ? "⌘" : ""}${keyName(s.key)}`;
  const parts = [s.ctrl ? "Ctrl" : "", s.alt ? "Alt" : "", s.shift ? t("key.shift") : "", keyName(s.key)];
  return parts.filter(Boolean).join("+");
}

/** Raccourci au format des accélérateurs de Tauri (menu natif). */
export function accelerator(s: Shortcut): string {
  const key = s.key === "," ? "Comma" : s.key.length === 1 ? s.key.toUpperCase() : s.key;
  return [s.ctrl ? "CmdOrCtrl" : "", s.alt ? "Alt" : "", s.shift ? "Shift" : "", key].filter(Boolean).join("+");
}

function matches(e: KeyboardEvent, s: Shortcut): boolean {
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  return key === s.key && !!s.ctrl === (e.ctrlKey || e.metaKey) && !!s.shift === e.shiftKey && !!s.alt === e.altKey;
}

/** Raccourcis des menus (Windows, Linux ; sous macOS, le menu natif s'en
 *  charge). Vrai si l'événement a été traité. */
export function handleMenuKey(e: KeyboardEvent): boolean {
  if (isMac || e.repeat) return false;
  for (const c of list) {
    if (!c.shortcut || c.bind === false || !matches(e, c.shortcut)) continue;
    e.preventDefault();
    if (c.enabled?.() ?? true) c.run();
    return true;
  }
  return false;
}

/** Description du menu natif de macOS, envoyée au pont Tauri. */
type NativeItem =
  | { kind: "command"; id: string; label: string; accelerator: string | null; enabled: boolean }
  | { kind: "native"; which: string }
  | { kind: "separator" }
  | { kind: "submenu"; label: string; items: NativeItem[] };

// Raccourcis d'une seule touche (Espace, J, K, L, Tab...) : affichés dans la
// barre dessinée mais pas donnés au menu natif, qui les prendrait aux champs
// de texte.
const nativeAccel = (s?: Shortcut) => (s && (s.ctrl || s.alt || /^F\d+$/.test(s.key)) ? accelerator(s) : null);

function toNative(entries: Entry[]): NativeItem[] {
  return entries.map((e): NativeItem => {
    if (e === "-") return { kind: "separator" };
    if (typeof e !== "string") return { kind: "submenu", label: e.submenu(), items: toNative(e.items()) };
    const c = command(e)!;
    if (c.native && c.native !== "fullscreen") return { kind: "native", which: c.native };
    return { kind: "command", id: c.id, label: c.label(), accelerator: nativeAccel(c.shortcut), enabled: c.enabled?.() ?? true };
  });
}

export function nativeMenus() {
  const appMenu = {
    label: "VERIFLOW",
    items: [
      ...toNative(["help.about", "-", "edit.prefs", "-"]),
      { kind: "native", which: "hide" },
      { kind: "native", which: "hide_others" },
      { kind: "native", which: "show_all" },
      { kind: "separator" },
      { kind: "native", which: "quit" },
    ] as NativeItem[],
  };
  const windowMenu = {
    label: t("menu.window"),
    items: [
      { kind: "native", which: "minimize" },
      { kind: "native", which: "fullscreen" },
    ] as NativeItem[],
  };
  const menus = MENUS.map((m) => ({
    label: m.label(),
    // Le plein écran de macOS est dans le menu Fenêtre.
    items: toNative(m.items().filter((e) => e !== "view.fullscreen")),
  }));
  menus.splice(menus.length - 1, 0, windowMenu);
  return [appMenu, ...menus];
}

/** Menu natif de macOS : envoyé au pont, clics relayés par l'événement « menu ». */
export function startNativeMenu(): () => void {
  if (!isMac || !inTauri) return () => {};
  const stop = listen<string>("menu", (e) => runCommand(e.payload));
  return () => void stop.then((u) => u());
}

export const setNativeMenu = () => (isMac && inTauri ? invoke("menu_set", { menus: nativeMenus() }) : Promise.resolve());
