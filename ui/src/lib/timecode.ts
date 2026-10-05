// Timecode SMPTE côté interface (même algorithme que core/src/player/timecode.rs).
export interface FrameRate {
  num: number;
  den: number;
}

export const fps = (r: FrameRate) => r.num / r.den;
export const nominal = (r: FrameRate) => Math.round(fps(r));
export const supportsDropFrame = (r: FrameRate) => r.den === 1001 && nominal(r) % 30 === 0;

const pad = (n: number) => String(n).padStart(2, "0");

/** Convertit un nombre d'images en "HH:MM:SS:FF" (";" avant les images en drop-frame). */
export function framesToTc(frames: number, rate: FrameRate, dropFrame = false): string {
  const fpsN = nominal(rate);
  const df = dropFrame && supportsDropFrame(rate);
  const day = 24 * 3600 * fpsN;
  let n = ((Math.trunc(frames) % day) + day) % day;
  if (df) {
    const drop = (fpsN / 30) * 2;
    const per10 = fpsN * 600 - drop * 9;
    const perMin = fpsN * 60 - drop;
    const tens = Math.floor(n / per10);
    const rem = n % per10;
    n += drop * 9 * tens;
    if (rem > drop) n += drop * Math.floor((rem - drop) / perMin);
  }
  const h = Math.floor(n / (fpsN * 3600));
  const m = Math.floor(n / (fpsN * 60)) % 60;
  const s = Math.floor(n / fpsN) % 60;
  const f = n % fpsN;
  return `${pad(h)}:${pad(m)}:${pad(s)}${df ? ";" : ":"}${pad(f)}`;
}

/** Durée en secondes vers "HH:MM:SS.mmm" (affichage audio sans cadence image). */
export function secondsToClock(seconds: number): string {
  const t = Math.max(0, seconds);
  const h = Math.floor(t / 3600);
  const m = Math.floor(t / 60) % 60;
  const s = Math.floor(t) % 60;
  const ms = Math.floor((t % 1) * 1000);
  return `${pad(h)}:${pad(m)}:${pad(s)}.${String(ms).padStart(3, "0")}`;
}
