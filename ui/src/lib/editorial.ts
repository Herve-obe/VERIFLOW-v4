// Export des clips vers le montage (voir src-tauri/src/editorial.rs).
import { invoke } from "@tauri-apps/api/core";

export type EditFormat = "fcpxml" | "xmeml" | "edl" | "ale" | "otio" | "resolve_csv";
export type EditTarget = "fcp" | "premiere" | "resolve" | "avid" | "other";
export type FcpxmlVersion = "1.9" | "1.10" | "1.11";

export const EDIT_FORMATS: { id: EditFormat; label: string }[] = [
  { id: "fcpxml", label: "FCPXML" },
  { id: "xmeml", label: "XMEML" },
  { id: "edl", label: "EDL" },
  { id: "ale", label: "ALE" },
  { id: "otio", label: "OTIO" },
  { id: "resolve_csv", label: "CSV Resolve" },
];

export const EDIT_TARGETS: EditTarget[] = ["fcp", "premiere", "resolve", "avid", "other"];

/** Formats cochés par défaut selon le logiciel de montage. */
export const TARGET_FORMATS: Record<EditTarget, EditFormat[]> = {
  fcp: ["fcpxml"],
  premiere: ["xmeml"],
  resolve: ["fcpxml", "resolve_csv"],
  avid: ["ale", "edl"],
  other: ["otio", "edl"],
};

/** Version FCPXML conseillée selon le logiciel. */
export const TARGET_FCPXML: Record<EditTarget, FcpxmlVersion> = {
  fcp: "1.11",
  premiere: "1.10",
  resolve: "1.10",
  avid: "1.10",
  other: "1.10",
};

export interface EditOptions {
  title: string;
  paths: "absolute" | "relative";
  /** Null : le cœur prend le dossier du fichier écrit. */
  base: string | null;
  markers: boolean;
  fcpxml_version: FcpxmlVersion;
  ale_ascii: boolean;
}

export interface EditWarning {
  code: string;
  detail: string;
}

export interface EditResult {
  written: string[];
  warnings: EditWarning[];
}

/** Les chemins relatifs n'existent pas pour FCPXML et XMEML. */
export const relativeAllowed = (formats: EditFormat[]) =>
  !formats.some((f) => f === "fcpxml" || f === "xmeml");

export const editorialExport = (formats: EditFormat[], paths: string[], options: EditOptions, out: string) =>
  invoke<EditResult>("editorial_export", { formats, paths, options, out });
