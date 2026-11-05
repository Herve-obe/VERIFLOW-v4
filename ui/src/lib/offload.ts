// Accès aux commandes OFFLOAD (voir src-tauri/src/offload.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type HashAlgo = "xxh128" | "xxh64" | "xxh3" | "md5" | "sha1" | "sha256" | "c4";

export const ALGORITHMS: { id: HashAlgo; label: string; mhl: boolean }[] = [
  { id: "xxh128", label: "XXH128", mhl: true },
  { id: "xxh64", label: "XXH64", mhl: true },
  { id: "xxh3", label: "XXH3-64", mhl: true },
  { id: "md5", label: "MD5", mhl: true },
  { id: "sha1", label: "SHA-1", mhl: true },
  { id: "sha256", label: "SHA-256", mhl: false },
  { id: "c4", label: "C4", mhl: true },
];

export interface Volume {
  mount_point: string;
  name: string;
  file_system: string;
  total: number;
  available: number;
  removable: boolean;
  kind: "ssd" | "hdd" | "unknown";
}

export interface OffloadRequest {
  source: string;
  destinations: string[];
  template: string;
  vars: { projet: string | null; jour: string | null; camera: string | null };
  algorithms: HashAlgo[];
  operator: string | null;
  notes: string | null;
  /** Rushes déjà présents : revérifier (défaut), compléter ou tout recopier. */
  existing?: ExistingMode;
  /** Date du modèle {date} (reprise d'une copie : même dossier). */
  date?: string | null;
  /** Dossiers finals imposés (reprise d'une copie trouvée dans un autre dossier). */
  roots?: string[] | null;
}

export type ExistingMode = "verify" | "complete" | "replace";

export interface Present {
  files: number;
  bytes: number;
  partial: number;
}

export interface DestCheck {
  identical: number;
  identical_bytes: number;
  different: string[];
  missing: number;
}

export interface CheckView {
  id: number;
  roots: string[];
  destinations: DestCheck[];
}

export interface PreflightView {
  source_name: string;
  files: number;
  total_bytes: number;
  fingerprint: string;
  roots: string[];
  missing_space: (number | null)[];
  already_in_destination: boolean[];
  present: Present[];
  date: string;
  /** Par destination : copies de cette carte trouvées dans d'autres dossiers. */
  elsewhere: (Present & { root: string })[][];
  hdd: boolean[];
  source_hdd: boolean;
  previous: { finished_at: string; source_name: string; destinations: string[] }[];
  /** Durée estimée de la copie (secondes). */
  estimated_secs: number;
  battery: BatteryWarning | null;
}

export interface BatteryWarning {
  reason: "low" | "long" | "short";
  percent: number | null;
  minutes_left: number | null;
  estimated_minutes: number;
}

export interface PowerStatus {
  on_battery: boolean;
  percent: number | null;
  minutes_left: number | null;
}

export type DestStatus =
  | { state: "verified" }
  | { state: "resumed_verified" }
  | { state: "failed"; detail: string };

export interface FileResult {
  rel: string;
  size: number;
  modified: string;
  hashes: [HashAlgo, string][];
  destinations: DestStatus[];
}

export type EngineEvent =
  | { type: "file_started"; index: number; rel: string; size: number }
  | { type: "progress"; done: number; total: number; rate: number }
  | { type: "file_done"; index: number; result: FileResult };

export interface JobResult {
  summary: {
    files: FileResult[];
    total_bytes: number;
    started_at: string;
    finished_at: string;
    duration_s: number;
    cancelled: boolean;
    failed_files: number;
  };
  roots: string[];
  mhl: (string | null)[];
  reports: { csv: string; html: string; pdf: string; pdf_sha256: string }[];
  source_name: string;
  fingerprint: string;
}

export type JobKindName = "offload" | "verify";

export type VerifyState =
  | { state: "ok" }
  | { state: "different" }
  | { state: "missing" }
  | { state: "new" }
  | { state: "unreadable"; detail: string };

export interface VerifySummary {
  root: string;
  files: { rel: string; size: number; status: VerifyState }[];
  total_bytes: number;
  ok: number;
  different: number;
  missing: number;
  new: number;
  unreadable: number;
  started_at: string;
  finished_at: string;
  duration_s: number;
  cancelled: boolean;
  conform: boolean;
  mhl: string | null;
  write_error: string | null;
  chain: { entries: { seq: number; file: string; c4: string }[]; issues: { kind: "altered" | "missing"; name: string }[] };
  /** L'historique ne décrit aucun fichier : rien à comparer. */
  empty_history: boolean;
  /** Fichiers retirés de `files` (conformes, nouveaux au-delà de 200) : détail dans le CSV. */
  omitted: number;
}

export interface ReportFiles {
  csv: string;
  html: string;
  pdf: string;
  pdf_sha256: string;
}

export interface MhlInfo {
  root: string;
  generations: number;
  last_date: string | null;
  files: number;
  algos: HashAlgo[];
  chain_issues: { kind: "altered" | "missing"; name: string }[];
  histories: string[];
}

export interface MhlError {
  code: "no_history" | "mhl_v1" | "nested_history" | null;
  message: string;
}

export type WaitReason = "limit" | "source_busy" | "root_busy" | "source_in_use" | "hdd_busy" | "space";

/** Copies simultanées au maximum. Miroir de core/src/offload/schedule.rs : à garder identique. */
export const MAX_PARALLEL_JOBS = 3;

export type EjectCode = "source_in_use_eject" | "eject_not_removable";
export type ErrorCode = "no_space" | "job_panic" | "job_spawn" | "no_history" | "mhl_v1" | "nested_history";

export type Notice =
  | { kind: "queued"; job: number; source: string }
  | { kind: "waiting"; job: number; reason: WaitReason }
  | { kind: "dropped"; job: number }
  | { kind: "starting"; job: number }
  | { kind: "running"; job: number; job_kind?: JobKindName; files: number; total_bytes: number; roots: string[] }
  | { kind: "engine"; job: number; event: EngineEvent }
  | { kind: "done"; job: number; job_kind?: JobKindName; result?: JobResult; verify?: VerifySummary; reports?: ReportFiles; report_error?: string; ejected: { Ok: null } | { Err: string } | null; eject_code: EjectCode | null }
  | { kind: "failed"; job: number; error: string; error_code: ErrorCode | null };

export const volumes = () => invoke<Volume[]>("offload_volumes");
export const preflight = (request: OffloadRequest) => invoke<PreflightView>("offload_preflight", { request });
export const start = (request: OffloadRequest, eject: boolean, check: number | null = null, totalBytes: number | null = null) =>
  invoke<number>("offload_start", { request, eject, check, totalBytes });
export const templatePreview = (template: string, vars: OffloadRequest["vars"], card: string, date: string | null) =>
  invoke<string>("offload_template_preview", { template, vars, card, date });
export const checkExisting = (request: OffloadRequest) => invoke<CheckView>("offload_check_existing", { request });
export const checkCancel = () => invoke<void>("offload_check_cancel");
export const onCheckProgress = (cb: (p: { done: number; total: number }) => void): Promise<UnlistenFn> =>
  listen<{ done: number; total: number }>("offload-check", (e) => cb(e.payload));
export const mhlInfo = (root: string) => invoke<MhlInfo>("offload_mhl_info", { root });
export const mhlVerify = (root: string, operator: string | null = null) => invoke<number>("offload_mhl_verify", { root, operator });
export const cancel = (job: number) => invoke<void>("offload_cancel", { job });
export const eject = (mountPoint: string) => invoke<void>("offload_eject", { mountPoint });
export const reveal = (path: string) => invoke<void>("reveal", { path });
export const onNotice = (cb: (n: Notice) => void): Promise<UnlistenFn> => listen<Notice>("offload", (e) => cb(e.payload));
export const powerStatus = () => invoke<PowerStatus | null>("power_status");

/** Avertissement batterie recalculé à l'instant du lancement (null si rien à signaler). */
export const batteryWarning = (estimatedSecs: number) =>
  invoke<BatteryWarning | null>("battery_warning", { estimatedSecs });

/**
 * Secondes restantes d'une copie. `total` compte les octets lus et écrits
 * (octets × (1 + nb destinations)) alors que `rate` est le débit de lecture de la source.
 * NaN si le débit ou le total est inconnu.
 */
export function remainingSecs(total: number, done: number, rate: number, destinations: number): number {
  if (!(rate > 0) || !(total > 0)) return NaN;
  return (total - done) / (rate * (1 + destinations));
}

export type PowerReason = "unplugged" | "low" | "running_short" | "running_long";

export interface PowerAlert {
  key: string;
  vars: Record<string, string>;
  reason: PowerReason;
}

const LOW_PERCENT = 20;
const LONG_MINUTES = 10;
/** Gravité : une alerte ne réapparaît que si la raison s'aggrave. */
const RANK: Record<PowerReason, number> = { running_long: 1, unplugged: 1, low: 2, running_short: 3 };

export function powerRank(r: PowerReason | null): number {
  return r ? RANK[r] : 0;
}

/**
 * Décide s'il faut alerter pendant une copie (fonction pure).
 * `before` : sur batterie au relevé précédent (null au premier relevé).
 * `etaSecs` : fin estimée de la copie la plus longue (NaN/null si inconnue).
 * `last` : raison de la dernière alerte depuis le passage sur batterie.
 * `startedOnBattery` : la surveillance a démarré sur batterie (et n'est pas repassée sur secteur).
 * Retourne l'alerte à afficher, ou null.
 */
export function decidePower(before: boolean | null, p: PowerStatus, etaSecs: number | null, last: PowerReason | null, startedOnBattery: boolean): PowerAlert | null {
  if (!p.on_battery) return null;
  const eta = etaSecs !== null && Number.isFinite(etaSecs) ? etaSecs / 60 : null;
  let reason: PowerReason | null = null;
  if (p.minutes_left !== null && eta !== null && eta > p.minutes_left) reason = "running_short";
  else if (p.percent !== null && p.percent <= LOW_PERCENT) reason = "low";
  else if (before === false) reason = "unplugged";
  else if (startedOnBattery && eta !== null && eta > LONG_MINUTES) reason = "running_long";
  if (!reason || RANK[reason] <= powerRank(last)) return null;
  const np = p.percent === null ? "_nopercent" : "";
  const vars = {
    percent: String(p.percent ?? ""),
    left: String(p.minutes_left ?? ""),
    minutes: String(eta === null ? "" : Math.round(eta)),
  };
  const key =
    reason === "low"
      ? "offload.battery.running_low"
      : reason === "running_short"
        ? "offload.battery.running_short"
        : `offload.battery.${reason === "unplugged" ? "unplugged" : "running_long"}${np}`;
  return { key, vars, reason };
}
