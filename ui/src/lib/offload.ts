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

export type Notice =
  | { kind: "queued"; job: number; source: string }
  | { kind: "running"; job: number; files: number; total_bytes: number; roots: string[] }
  | { kind: "engine"; job: number; event: EngineEvent }
  | { kind: "done"; job: number; result: JobResult; ejected: { Ok: null } | { Err: string } | null }
  | { kind: "failed"; job: number; error: string };

export const volumes = () => invoke<Volume[]>("offload_volumes");
export const preflight = (request: OffloadRequest) => invoke<PreflightView>("offload_preflight", { request });
export const start = (request: OffloadRequest, eject: boolean, check: number | null = null) =>
  invoke<number>("offload_start", { request, eject, check });
export const templatePreview = (template: string, vars: OffloadRequest["vars"], card: string, date: string | null) =>
  invoke<string>("offload_template_preview", { template, vars, card, date });
export const checkExisting = (request: OffloadRequest) => invoke<CheckView>("offload_check_existing", { request });
export const checkCancel = () => invoke<void>("offload_check_cancel");
export const onCheckProgress = (cb: (p: { done: number; total: number }) => void): Promise<UnlistenFn> =>
  listen<{ done: number; total: number }>("offload-check", (e) => cb(e.payload));
export const cancel = (job: number) => invoke<void>("offload_cancel", { job });
export const eject = (mountPoint: string) => invoke<void>("offload_eject", { mountPoint });
export const reveal = (path: string) => invoke<void>("reveal", { path });
export const onNotice = (cb: (n: Notice) => void): Promise<UnlistenFn> => listen<Notice>("offload", (e) => cb(e.payload));
