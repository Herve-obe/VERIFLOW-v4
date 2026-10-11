// État des conversions (file d'attente et progression), alimenté par les événements du cœur.
import { defaultSettings } from "../lib/transcode";
import {
  onNotice,
  type FileResult,
  type Notice,
  type TranscodeRequest,
} from "../lib/transcode";

export type JobState = "queued" | "running" | "done" | "failed";

export interface Job {
  id: number;
  request: TranscodeRequest;
  label: string;
  state: JobState;
  files: number;
  index: number;
  current: string;
  fraction: number;
  speed: number;
  results: FileResult[];
  cancelled: boolean;
  error: string | null;
  startedAt: number;
  seconds: number;
  report: string | null;
}

export const transcode = $state({
  jobs: [] as Job[],
  /** Libellé et demande de chaque lot, connus avant le premier événement. */
  pending: {} as Record<number, { request: TranscodeRequest; label: string }>,
});

const emptyRequest = (): TranscodeRequest => ({
  sources: [],
  settings: defaultSettings(""),
  dest: null,
  prefix: "",
  suffix: "",
  replace_from: "",
  replace_to: "",
  numbering: false,
  number_start: 1,
  number_digits: 3,
  existing: "rename",
  checksum: false,
  report: false,
});

export function removeJob(id: number) {
  transcode.jobs = transcode.jobs.filter((j) => j.id !== id);
}

/** Avancement global du lot (0 à 1). */
export const progressOf = (j: Job) =>
  j.files > 0
    ? Math.min(
        1,
        (j.results.length + (j.state === "running" ? j.fraction : 0)) / j.files,
      )
    : 0;

const job = (id: number) => transcode.jobs.find((j) => j.id === id);

function apply(n: Notice) {
  if (n.kind === "queued") {
    if (job(n.job)) return;
    const p = transcode.pending[n.job];
    transcode.jobs.unshift({
      id: n.job,
      request: p?.request ?? emptyRequest(),
      label: p?.label ?? "",
      state: "queued",
      files: n.files,
      index: 0,
      current: "",
      fraction: 0,
      speed: 0,
      results: [],
      cancelled: false,
      error: null,
      startedAt: 0,
      seconds: 0,
      report: null,
    });
    return;
  }
  const j = job(n.job);
  if (!j) return;
  switch (n.kind) {
    case "running":
      j.state = "running";
      j.startedAt = Date.now();
      break;
    case "engine": {
      const e = n.event;
      if (e.type === "file_started")
        Object.assign(j, {
          index: e.index,
          current: e.source,
          fraction: 0,
          speed: 0,
        });
      else if (e.type === "progress")
        Object.assign(j, { fraction: e.fraction, speed: e.speed });
      else j.results.push(e.result);
      break;
    }
    case "done":
      Object.assign(j, {
        state: "done",
        current: "",
        results: n.summary.files,
        cancelled: n.summary.cancelled,
        seconds: n.summary.seconds,
        report: n.summary.report,
      });
      break;
    case "failed":
      Object.assign(j, { state: "failed", error: n.error });
      break;
  }
}

let started = false;
/** À appeler une fois au démarrage de l'onglet. */
export function listenTranscode() {
  if (started) return;
  started = true;
  onNotice(apply).catch(() => {
    started = false; // hors de Tauri (aperçu navigateur)
  });
}
