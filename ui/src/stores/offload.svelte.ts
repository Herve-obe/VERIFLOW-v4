// État des copies (file d'attente et progression), alimenté par les événements du cœur.
import { t } from "../i18n/index.svelte";
import { onNotice, powerStatus, type FileResult, type JobResult, type Notice, type OffloadRequest } from "../lib/offload";

export type JobState = "queued" | "running" | "done" | "failed";

export interface Job {
  id: number;
  source: string;
  state: JobState;
  files: number;
  totalBytes: number;
  roots: string[];
  done: number;
  total: number;
  rate: number;
  current: string;
  verified: number;
  failed: FileResult[];
  recent: FileResult[];
  result: JobResult | null;
  error: string | null;
  ejected: string | null;
  startedAt: number;
}

const RECENT = 200; // lignes affichées dans le tableau d'avancement

export const offload = $state({
  jobs: [] as Job[],
  /** Demande de chaque copie (reprise après interruption). */
  requests: {} as Record<number, OffloadRequest>,
  /** Copie à reprendre : le formulaire est rempli avec cette demande. */
  resume: null as OffloadRequest | null,
  /** Alerte batterie pendant une copie (texte déjà traduit). */
  batteryAlert: null as string | null,
});

export function dismissBatteryAlert() {
  offload.batteryAlert = null;
}

const LOW_PERCENT = 20;
let timer: ReturnType<typeof setInterval> | null = null;
let wasOnBattery: boolean | null = null;
let alerted = false;

function fill(key: string, percent: number | null, left: number | null): string {
  return t(key)
    .replace("{percent}", String(percent ?? ""))
    .replace("{left}", String(left ?? ""));
}

async function pollPower() {
  let p;
  try {
    p = await powerStatus();
  } catch {
    return;
  }
  if (!p) return;
  const before = wasOnBattery;
  wasOnBattery = p.on_battery;
  if (!p.on_battery) {
    alerted = false;
    offload.batteryAlert = null;
    return;
  }
  if (alerted) return;
  const unplugged = before === false;
  const low = p.percent !== null && p.percent <= LOW_PERCENT;
  if (unplugged || low) {
    const key = p.percent === null ? "offload.battery.unplugged_nopercent" : "offload.battery.unplugged";
    offload.batteryAlert = fill(key, p.percent, p.minutes_left);
    alerted = true;
    return;
  }
  if (p.minutes_left !== null) {
    const left = p.minutes_left;
    const short = offload.jobs.some((j) => j.state === "running" && j.rate > 0 && (j.total - j.done) / j.rate / 60 > left);
    if (short) {
      offload.batteryAlert = fill("offload.battery.running_short", p.percent, left);
      alerted = true;
    }
  }
}

/** Interroge l'alimentation tant qu'une copie tourne, et seulement alors. */
function syncPowerWatch() {
  const running = offload.jobs.some((j) => j.state === "running");
  if (running && !timer) {
    wasOnBattery = null;
    alerted = false;
    void pollPower();
    timer = setInterval(pollPower, 60_000);
  } else if (!running && timer) {
    clearInterval(timer);
    timer = null;
    wasOnBattery = null;
    alerted = false;
    offload.batteryAlert = null;
  }
}

/** Retire une fiche de la file (les fichiers sur les disques ne sont pas touchés). */
export function removeJob(id: number) {
  offload.jobs = offload.jobs.filter((j) => j.id !== id);
  delete offload.requests[id];
}

/** Vrai si la copie s'est arrêtée avant d'être complète et vérifiée. */
export function interrupted(j: Job): boolean {
  if (j.state === "failed") return true;
  if (j.state !== "done" || !j.result) return false;
  return j.result.summary.cancelled || j.result.summary.failed_files > 0;
}

function job(id: number): Job | undefined {
  return offload.jobs.find((j) => j.id === id);
}

function apply(n: Notice) {
  applyNotice(n);
  syncPowerWatch();
}

function applyNotice(n: Notice) {
  if (n.kind === "queued") {
    offload.jobs.unshift({
      id: n.job,
      source: n.source,
      state: "queued",
      files: 0,
      totalBytes: 0,
      roots: [],
      done: 0,
      total: 0,
      rate: 0,
      current: "",
      verified: 0,
      failed: [],
      recent: [],
      result: null,
      error: null,
      ejected: null,
      startedAt: 0,
    });
    return;
  }
  const j = job(n.job);
  if (!j) return;
  switch (n.kind) {
    case "running":
      Object.assign(j, { state: "running", files: n.files, totalBytes: n.total_bytes, roots: n.roots, startedAt: Date.now() });
      break;
    case "engine": {
      const e = n.event;
      if (e.type === "progress") Object.assign(j, { done: e.done, total: e.total, rate: e.rate });
      else if (e.type === "file_started") j.current = e.rel;
      else {
        const ok = e.result.destinations.every((d) => d.state !== "failed");
        if (ok) j.verified++;
        else j.failed.push(e.result);
        j.recent.unshift(e.result);
        if (j.recent.length > RECENT) j.recent.pop();
      }
      break;
    }
    case "done":
      j.state = "done";
      j.result = n.result;
      j.current = "";
      if (n.ejected && "Ok" in n.ejected) j.ejected = "ok";
      else if (n.ejected && "Err" in n.ejected) j.ejected = n.ejected.Err;
      break;
    case "failed":
      j.state = "failed";
      j.error = n.error;
      break;
  }
}

let started = false;
/** À appeler une fois au démarrage de l'interface. */
export function listenOffload() {
  if (started) return;
  started = true;
  onNotice(apply).catch(() => {
    started = false; // hors de Tauri (aperçu navigateur)
  });
}
