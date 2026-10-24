// Textes juridiques et FFmpeg livré (voir src-tauri/src/legal.rs).
import { invoke } from "@tauri-apps/api/core";

export type LegalDoc = "gpl" | "cluf" | "cgu" | "tiers" | "ffmpeg" | "third_party";

export const legalText = (name: LegalDoc) => invoke<string>("legal_text", { name });

export interface FfmpegInfo {
  version: string;
  buildconf: string;
}
export const ffmpegInfo = () => invoke<FfmpegInfo>("ffmpeg_info");
