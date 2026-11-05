// État des copies (file d'attente et progression), alimenté par les événements du cœur.
import { decidePower, onNotice, powerStatus, remainingSecs, type PowerReason, type Rates, progressSpeed, type WaitReason, type EjectCode, type ErrorCode, type FileResult, type JobKindName, type VerifySummary, type ReportFiles, type JobResult, type Notice, type OffloadRequest, type PendingView, cascadePending } from "../lib/offload";

export type JobState = "queued" | "waiting" | "starting" | "running" | "done" | "failed";

export interface Job {
  id: number;
  source: string;
  kind: JobKindName;
  state: JobState;
  /** Raison de l'attente (état waiting), null sinon. */
  waitReason: WaitReason | null;
  /** Retirée de la file par l'annulation avant d'avoir démarré. */
  dropped: boolean;
  files: number;
  totalBytes: number;
  roots: string[];
  done: number;
  total: number;
  rate: number;
  /** Derniers débits mesurés par disque et par phase. */
  rates: Rates | null;
  /** Vitesse d'avancement lissée sur ~8 s (octets/s), 0 si inconnue. */
  speed: number;
  /** Relevés récents de la vitesse (horodatage ms, octets/s). */
  speedHist: { t: number; v: number }[];
  current: string;
  verified: number;
  failed: FileResult[];
  recent: FileResult[];
  result: JobResult | null;
  verify: VerifySummary | null;
  verifyReports: ReportFiles | null;
  reportError: string | null;
  error: string | null;
  errorCode: ErrorCode | null;
  ejected: string | null;
  ejectCode: EjectCode | null;
  startedAt: number;
  /** Étape de la copie : 1 (immédiate) ou 2 (en cascade). */
  stage: 1 | 2;
  /** Étape 2 : numéro de la copie d'étape 1 d'origine. */
  cascadeOf: number | null;
  /** Étape 1 : numéro de la copie d'étape 2 mise en file. */
  cascadeJob: number | null;
  /** Étape 1 : la carte peut être libérée (null tant que la copie n'est pas finie). */
  cardFree: boolean | null;
}

/** Fenêtre de la moyenne glissante de la vitesse. */
const SMOOTH_MS = 8000;
const RECENT = 200; // lignes affichées dans le tableau d'avancement

export const offload = $state({
  jobs: [] as Job[],
  /** Demande de chaque copie (reprise après interruption). */
  requests: {} as Record<number, OffloadRequest>,
  /** Copie à reprendre : le formulaire est rempli avec cette demande. */
  resume: null as OffloadRequest | null,
  /** Alerte batterie pendant une copie (clé i18n et variables, traduites à l'affichage). */
  batteryAlert: null as { key: string; vars: Record<string, string> } | null,
  /** Copies en cascade enregistrées dont l'étape 2 reste à faire. */
  pending: [] as PendingView[],
});

/** Relit la liste des copies en cascade à reprendre. */
export async function refreshPending() {
  try {
    offload.pending = await cascadePending();
  } catch {
    offload.pending = []; // hors de Tauri
  }
}

export function dismissBatteryAlert() {
  offload.batteryAlert = null;
}

let timer: ReturnType<typeof setInterval> | null = null;
let wasOnBattery: boolean | null = null;
let lastReason: PowerReason | null = null;
let startedOnBattery = false;
/** Génération de la surveillance : un relevé d'une génération révolue est ignoré. */
let generation = 0;

/** Fin estimée (secondes) de la copie en cours la plus longue, null si inconnue. */
function runningEta(): number | null {
  let max: number | null = null;
  for (const j of offload.jobs) {
    if (j.state !== "running") continue;
    const s = remainingSecs(j.total, j.done, j.rate, j.kind === "verify" ? 0 : j.roots.length, j.speed);
    if (Number.isFinite(s) && (max === null || s > max)) max = s;
  }
  return max;
}

async function pollPower() {
  const gen = generation;
  let p;
  try {
    p = await powerStatus();
  } catch {
    return;
  }
  if (gen !== generation) return; // la surveillance a été arrêtée ou relancée pendant l'attente
  if (!p) return;
  const before = wasOnBattery;
  wasOnBattery = p.on_battery;
  if (!p.on_battery) {
    lastReason = null;
    startedOnBattery = false;
    offload.batteryAlert = null;
    return;
  }
  if (before === null) startedOnBattery = true;
  const alert = decidePower(before, p, runningEta(), lastReason, startedOnBattery);
  if (alert) {
    offload.batteryAlert = { key: alert.key, vars: alert.vars };
    lastReason = alert.reason;
  }
}

/** Interroge l'alimentation tant qu'une copie tourne, et seulement alors. */
function syncPowerWatch() {
  const running = offload.jobs.some((j) => j.state === "running");
  if (running && !timer) {
    generation++;
    wasOnBattery = null;
    lastReason = null;
    startedOnBattery = false;
    void pollPower();
    timer = setInterval(pollPower, 60_000);
  } else if (!running && timer) {
    clearInterval(timer);
    timer = null;
    generation++;
    wasOnBattery = null;
    lastReason = null;
    startedOnBattery = false;
    offload.batteryAlert = null;
  }
}

/** Travaux de vérification dont la notification de mise en file peut précéder le retour de la commande. */
const verifyIds = new Set<number>();

/** Signale que le travail `id` est une revérification (avant son premier `running`). */
export function markVerify(id: number) {
  verifyIds.add(id);
  const j = job(id);
  if (j) j.kind = "verify";
}

/** Retire une fiche de la file (les fichiers sur les disques ne sont pas touchés). */
export function removeJob(id: number) {
  offload.jobs = offload.jobs.filter((j) => j.id !== id);
  delete offload.requests[id];
}

/** Vrai si la copie s'est arrêtée avant d'être complète et vérifiée. */
export function interrupted(j: Job): boolean {
  if (j.state === "failed") return true;
  if (j.kind === "verify") return false;
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
      kind: verifyIds.has(n.job) ? "verify" : "offload",
      state: "queued",
      waitReason: null,
      dropped: false,
      files: 0,
      totalBytes: 0,
      roots: [],
      done: 0,
      total: 0,
      rate: 0,
      rates: null,
      speed: 0,
      speedHist: [],
      current: "",
      verified: 0,
      failed: [],
      recent: [],
      result: null,
      verify: null,
      verifyReports: null,
      reportError: null,
      error: null,
      errorCode: null,
      ejected: null,
      ejectCode: null,
      startedAt: 0,
      stage: n.cascade_of != null ? 2 : 1,
      cascadeOf: n.cascade_of ?? null,
      cascadeJob: null,
      cardFree: null,
    });
    return;
  }
  const j = job(n.job);
  if (!j) return;
  switch (n.kind) {
    case "waiting":
      if (j.state === "queued" || j.state === "waiting") Object.assign(j, { state: "waiting", waitReason: n.reason });
      break;
    case "dropped":
      if (j.state === "queued" || j.state === "waiting" || j.state === "starting") Object.assign(j, { state: "failed", waitReason: null, dropped: true });
      break;
    case "starting":
      if (j.state === "queued" || j.state === "waiting") Object.assign(j, { state: "starting", waitReason: null });
      break;
    case "running":
      Object.assign(j, { state: "running", kind: n.job_kind ?? "offload", waitReason: null, files: n.files, totalBytes: n.total_bytes, roots: n.roots, startedAt: Date.now() });
      break;
    case "engine": {
      const e = n.event;
      if (e.type === "progress") Object.assign(j, { done: e.done, total: e.total, rate: e.rate });
      else if (e.type === "rates") {
        j.rates = { source: e.source, write: e.write, verify: e.verify, hash_wait_ratio: e.hash_wait_ratio };
        const now = Date.now();
        j.speedHist.push({ t: now, v: progressSpeed(j.rates, j.roots.length) });
        while (j.speedHist.length > 1 && now - j.speedHist[0].t > SMOOTH_MS) j.speedHist.shift();
        j.speed = j.speedHist.reduce((a, h) => a + h.v, 0) / j.speedHist.length;
      }
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
      j.result = n.result ?? null;
      j.verify = n.verify ?? null;
      j.verifyReports = n.reports ?? null;
      j.reportError = n.report_error ?? null;
      if (n.job_kind) j.kind = n.job_kind;
      j.current = "";
      if (n.ejected && "Ok" in n.ejected) j.ejected = "ok";
      else if (n.ejected && "Err" in n.ejected) j.ejected = n.ejected.Err;
      j.ejectCode = n.eject_code ?? null;
      if (j.kind === "offload" && j.stage === 1) {
        j.cardFree = n.card_free ?? false;
        j.cascadeJob = n.cascade_job ?? null;
      }
      void refreshPending();
      break;
    case "failed":
      j.state = "failed";
      j.error = n.error;
      j.errorCode = n.error_code ?? null;
      void refreshPending();
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
