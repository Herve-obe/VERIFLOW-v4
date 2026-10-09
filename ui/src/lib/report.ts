// Rapports image et son (voir src-tauri/src/report.rs et core/src/report).
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { VIDEO_EXT, AUDIO_EXT } from "./player";

export type ReportKind = "image" | "sound";
export type ReportTemplate = "school" | "pro";
export type ReportLang = "fr" | "en";
export type ExportFormat = "pdf" | "csv" | "xlsx" | "html";

export interface ReportRow {
  clip: string | null;
  fields: Record<string, string>;
}

export interface Report {
  id: number;
  kind: ReportKind;
  template: ReportTemplate;
  number: number;
  header: Record<string, string>;
  rows: ReportRow[];
  tracks: number;
}

export interface ReportSummary {
  id: number;
  kind: ReportKind;
  number: number;
  date: string;
  title: string;
  rows: number;
  updated_at: string;
}

export interface HeaderField {
  key: string;
  label_fr: string;
  label_en: string;
  /** Cases du rapport papier (vide : texte libre). */
  options: string[];
  /** Valeurs supplémentaires proposées (« autre » sur papier). */
  extra: string[];
  unit: string;
}

export interface Column {
  key: string;
  label: string;
  width: number;
}

export interface ReportSchema {
  header: HeaderField[];
  columns: Column[];
}

export interface ReportBranding {
  logo: string | null;
  organization: string;
}

export const MAX_TRACKS = 32;
export const EXPORT_FORMATS: { id: ExportFormat; label: string }[] = [
  { id: "pdf", label: "PDF" },
  { id: "xlsx", label: "XLSX" },
  { id: "csv", label: "CSV" },
  { id: "html", label: "HTML" },
];

export const reportList = () => invoke<ReportSummary[]>("report_list");
export const reportGet = (id: number) => invoke<Report>("report_get", { id });
export const reportCreate = (kind: ReportKind, template: ReportTemplate) => invoke<Report>("report_create", { kind, template });
export const reportSave = (report: Report) => invoke<Report>("report_save", { report });
export const reportDelete = (id: number) => invoke<void>("report_delete", { id });
export const reportSchema = (kind: ReportKind, template: ReportTemplate, tracks: number, lang: ReportLang) =>
  invoke<ReportSchema>("report_schema", { kind, template, tracks, lang });
export const reportAddMedia = (report: Report, paths: string[]) => invoke<Report>("report_add_media", { report, paths });
export const brandingGet = () => invoke<ReportBranding>("report_branding_get");
export const brandingSet = (branding: ReportBranding) => invoke<void>("report_branding_set", { branding });
export const reportExport = (report: Report, format: ExportFormat, dest: string, lang: ReportLang) =>
  invoke<string>("report_export", { report, format, dest, lang });

/** Médias à ajouter : vidéos pour un rapport image, sons pour un rapport son. */
export async function pickMedia(kind: ReportKind, label: string): Promise<string[]> {
  const r = await open({ multiple: true, filters: [{ name: label, extensions: kind === "image" ? VIDEO_EXT : AUDIO_EXT }] });
  if (!r) return [];
  return Array.isArray(r) ? r : [r];
}

export async function pickLogo(label: string): Promise<string | null> {
  const r = await open({ multiple: false, filters: [{ name: label, extensions: ["png", "jpg", "jpeg", "svg"] }] });
  return typeof r === "string" ? r : null;
}

export async function pickExportPath(name: string, format: ExportFormat): Promise<string | null> {
  return save({ defaultPath: `${name}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
}

/** Type de rapport du mode courant. */
export const kindOfMode = (mode: "video" | "audio"): ReportKind => (mode === "video" ? "image" : "sound");
