// Formats d'affichage partagés (tailles en unités SI et binaires, débits, durées).

/** 1 500 000 000 octets -> "1,50 Go". Mo = 1 000 000 octets (SI). */
export function bytes(n: number): string {
  if (n < 1e3) return `${n} o`;
  const units = ["ko", "Mo", "Go", "To"];
  let v = n;
  let i = -1;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2).replace(".", ",")} ${units[i]}`;
}

/** Équivalent binaire : 1 Gio = 1 073 741 824 octets. */
export function bytesBinary(n: number): string {
  const units = ["Kio", "Mio", "Gio", "Tio"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(2).replace(".", ",")} ${units[i]}`;
}

export const rate = (bytesPerSecond: number) => `${bytes(bytesPerSecond)}/s`;

export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "--:--";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  const sec = s % 60;
  const pad = (x: number) => String(x).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}
