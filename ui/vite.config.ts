// Configuration Vite de l'interface VERIFLOW (servie par Tauri).
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import oldWebkit from "./old-webkit.postcss";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  // Moteur web de macOS Catalina (niveau Safari 13) : voir old-webkit.postcss.ts.
  css: { postcss: { plugins: [oldWebkit()] } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
  },
  build: {
    target: ["safari13", "chrome105"],
    outDir: "dist",
    emptyOutDir: true,
  },
});
