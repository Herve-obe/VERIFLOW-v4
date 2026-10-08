// État des copies (file d'attente et progression), alimenté par les événements du cœur.
import { onNotice, type FileResult, type JobResult, type Notice, type OffloadRequest } from "../lib/offload";

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
});

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
