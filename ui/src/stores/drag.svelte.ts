// Glisser-déposer de dossiers vers les zones de dépôt (OFFLOAD).
//
// Deux sources :
// - l'explorateur de VERIFLOW : glisser géré au pointeur, car le glisser HTML5
//   est bloqué sous Windows (WebView2) quand Tauri intercepte les dépôts de
//   fichiers du système ;
// - l'Explorateur Windows, le Finder ou le gestionnaire de fichiers : dépôt
//   natif transmis par Tauri, avec les chemins complets.
import { getCurrentWebview } from "@tauri-apps/api/webview";

type Zone = { name: string; accept: (paths: string[]) => void };

export const drag = $state<{ path: string | null; active: boolean; x: number; y: number; over: string | null }>({
  path: null,
  active: false,
  x: 0,
  y: 0,
  over: null,
});

const zones = new Map<HTMLElement, Zone>();
const THRESHOLD = 5; // px avant de considérer que le geste est un glisser

function zoneAt(x: number, y: number): Zone | null {
  let el = document.elementFromPoint(x, y) as HTMLElement | null;
  while (el) {
    const z = zones.get(el);
    if (z) return z;
    el = el.parentElement;
  }
  return null;
}

/** Action Svelte : déclare un élément comme zone de dépôt. */
export function dropZone(node: HTMLElement, zone: Zone) {
  zones.set(node, zone);
  return {
    update(z: Zone) {
      zones.set(node, z);
    },
    destroy() {
      zones.delete(node);
    },
  };
}

/** Démarre un glisser potentiel depuis un dossier de l'explorateur. */
export function beginDrag(e: PointerEvent, path: string) {
  if (e.button !== 0) return;
  const x0 = e.clientX;
  const y0 = e.clientY;
  const move = (m: PointerEvent) => {
    if (!drag.active) {
      if (Math.hypot(m.clientX - x0, m.clientY - y0) < THRESHOLD) return;
      drag.path = path;
      drag.active = true;
      document.body.classList.add("vf-dragging");
    }
    // Le bouton enfoncé étend sinon une sélection de texte sur la page.
    window.getSelection()?.removeAllRanges();
    drag.x = m.clientX;
    drag.y = m.clientY;
    drag.over = zoneAt(m.clientX, m.clientY)?.name ?? null;
  };
  const end = (u: PointerEvent) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", end);
    window.removeEventListener("pointercancel", end);
    if (drag.active && u.type === "pointerup") zoneAt(u.clientX, u.clientY)?.accept([path]);
    drag.active = false;
    drag.path = null;
    drag.over = null;
    document.body.classList.remove("vf-dragging");
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", end);
  window.addEventListener("pointercancel", end);
}

let nativeStarted = false;

/** Écoute les dépôts de dossiers venant du système (une seule fois). */
export function startNativeDrop() {
  if (nativeStarted) return;
  nativeStarted = true;
  // Tauri donne des coordonnées physiques ; le DOM attend des pixels CSS.
  const css = (p: { x: number; y: number }) => ({ x: p.x / devicePixelRatio, y: p.y / devicePixelRatio });
  getCurrentWebview()
    .onDragDropEvent((ev) => {
      const p = ev.payload;
      if (p.type === "enter" || p.type === "over") {
        const { x, y } = css(p.position);
        drag.over = zoneAt(x, y)?.name ?? null;
      } else if (p.type === "drop") {
        const { x, y } = css(p.position);
        drag.over = null;
        if (p.paths.length) zoneAt(x, y)?.accept(p.paths);
      } else {
        drag.over = null;
      }
    })
    .catch(() => {
      nativeStarted = false;
    });
}
