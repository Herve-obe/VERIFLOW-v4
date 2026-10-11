// Formulaire de l'onglet TRANSCODE : réglages, destination et nommage,
// mémorisés sur le poste pour chaque mode (VIDEO, AUDIO).
import { completeSettings, defaultSettings, LOUDNESS_TARGETS, type Existing, type PresetView, type Settings, type Trim, type TranscodeRequest } from "./transcode";

export interface Form {
  settings: Settings;
  /** "none", "ebu", "atsc", "stream", "podcast" ou "custom". */
  loudness: string;
  customI: number;
  customTp: number;
  dest: string;
  nextToSource: boolean;
  prefix: string;
  suffix: string;
  replaceFrom: string;
  replaceTo: string;
  numbering: boolean;
  numberStart: number;
  numberDigits: number;
  existing: Existing;
  checksum: boolean;
  report: boolean;
}

export function defaultForm(mode: string): Form {
  return {
    settings: defaultSettings(mode === "audio" ? "wav" : "prores_422"),
    loudness: "none",
    customI: -23,
    customTp: -1,
    dest: "",
    nextToSource: false,
    prefix: "",
    suffix: "",
    replaceFrom: "",
    replaceTo: "",
    numbering: false,
    numberStart: 1,
    numberDigits: 3,
    existing: "rename",
    checksum: false,
    report: false,
  };
}

/** Complète un formulaire enregistré (version plus ancienne, préréglage importé). */
export function completeForm(partial: Partial<Form> | null | undefined, mode: string): Form {
  const d = defaultForm(mode);
  if (!partial) return d;
  return { ...d, ...partial, settings: completeSettings(partial.settings, d.settings.preset) };
}

const key = (mode: string) => `veriflow.transcode.form.${mode}`;

export function loadForm(mode: string): Form {
  try {
    return completeForm(JSON.parse(localStorage.getItem(key(mode)) ?? "null"), mode);
  } catch {
    return defaultForm(mode);
  }
}

export function saveForm(mode: string, form: Form) {
  try {
    localStorage.setItem(key(mode), JSON.stringify(form));
  } catch {
    /* stockage indisponible */
  }
}

export function loudnessTarget(f: Form) {
  if (f.loudness === "none") return null;
  if (f.loudness === "custom") return { integrated: Number(f.customI), true_peak: Number(f.customTp) };
  return LOUDNESS_TARGETS.find((l) => l.id === f.loudness)?.target ?? null;
}

/** Demande envoyée au cœur : seuls les réglages utiles au préréglage. */
export function toRequest(f: Form, preset: PresetView, sources: string[], trims: Record<string, Trim>): TranscodeRequest {
  const s = $state.snapshot(f.settings) as Settings;
  const settings: Settings = {
    ...s,
    preset: preset.id,
    loudness: preset.audio || preset.video || preset.analysis ? loudnessTarget(f) : null,
    trims,
  };
  if (!preset.audio) {
    settings.sample_rate = null;
    settings.bit_depth = null;
  }
  if (!preset.bit_depths.includes(settings.bit_depth as never)) settings.bit_depth = null;
  if (!preset.audio_bitrates.length) settings.audio_kbps = null;
  if (!preset.video_bitrate) settings.video_mbps = null;
  if (!preset.encoder_choice) settings.software = false;
  if (!preset.scale_free) settings.scale = null;
  const writes = !preset.analysis || ["cut_detect", "black_detect", "offline_detect", "silence_detect", "framemd5"].includes(preset.kind);
  return {
    sources,
    settings,
    dest: !writes || f.nextToSource || !f.dest ? null : f.dest,
    prefix: f.prefix,
    suffix: f.suffix || preset.suffix,
    replace_from: f.replaceFrom,
    replace_to: f.replaceTo,
    numbering: f.numbering,
    number_start: Number(f.numberStart) || 1,
    number_digits: Number(f.numberDigits) || 3,
    existing: f.existing,
    checksum: f.checksum && !preset.analysis,
    report: f.report,
  };
}

// Préréglages personnels : réglages complets nommés, mémorisés sur le poste,
// exportables en fichier .vfpreset pour les partager.
export interface UserPreset {
  name: string;
  mode: string;
  form: Form;
}

const USER_KEY = "veriflow.transcode.user";

export function loadUserPresets(): UserPreset[] {
  try {
    const list = JSON.parse(localStorage.getItem(USER_KEY) ?? "[]");
    return Array.isArray(list) ? list : [];
  } catch {
    return [];
  }
}

export function saveUserPresets(list: UserPreset[]) {
  try {
    localStorage.setItem(USER_KEY, JSON.stringify(list));
  } catch {
    /* stockage indisponible */
  }
}

/** Contenu d'un fichier .vfpreset (sans dossier de sortie ni fichiers du poste). */
export function exportPreset(p: UserPreset): string {
  const form = JSON.parse(JSON.stringify(p.form)) as Form;
  form.dest = "";
  form.settings.trims = {};
  form.settings.audio_files = [];
  form.settings.insert_file = null;
  return JSON.stringify({ veriflow: "transcode-preset", version: 1, name: p.name, mode: p.mode, form }, null, 2);
}

export function importPreset(text: string): UserPreset {
  const data = JSON.parse(text);
  if (data?.veriflow !== "transcode-preset" || !data.form) throw new Error("fichier de préréglage VERIFLOW invalide");
  const mode = data.mode === "audio" ? "audio" : "video";
  return { name: String(data.name ?? "Préréglage"), mode, form: completeForm(data.form, mode) };
}
