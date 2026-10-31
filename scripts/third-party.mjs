#!/usr/bin/env node
// Avis de licences des composants tiers compilés dans VERIFLOW : crates Rust
// de la cible et paquets JavaScript de l'interface, avec le texte de licence
// fourni par chacun (MIT, BSD, Apache... demandent de le reproduire).
// Écrit src-tauri/licenses/THIRD_PARTY.txt, embarqué dans l'application.
//
// Usage : node scripts/third-party.mjs [cible Rust, ex. x86_64-pc-windows-msvc]
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const target = process.argv[2] ?? execFileSync("rustc", ["-vV"]).toString().match(/host: (\S+)/)[1];
const out = join(root, "src-tauri/licenses/THIRD_PARTY.txt");

/** Fichiers de licence d'un dossier de paquet (LICENSE, COPYING, NOTICE...). */
function licenseTexts(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => /^(licen[cs]e|copying|notice|unlicense|copyright)/i.test(f))
    .sort()
    .flatMap((f) => {
      try {
        return [{ file: f, text: readFileSync(join(dir, f), "utf8").trim() }];
      } catch {
        return []; // dossier (LICENSES/) ou fichier illisible
      }
    });
}

// --- Rust : paquets atteints par des dépendances normales (pas de dev ni de
// build), pour la cible demandée.
const meta = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", target], {
    cwd: root,
    maxBuffer: 1 << 28,
  }).toString(),
);
const byId = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const workspace = new Set(meta.workspace_members);
const seen = new Set();
const stack = [...workspace];
while (stack.length) {
  const id = stack.pop();
  if (seen.has(id)) continue;
  seen.add(id);
  for (const d of nodes.get(id)?.deps ?? []) {
    if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
  }
}
const crates = [...seen]
  .filter((id) => !workspace.has(id))
  .map((id) => byId.get(id))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

// --- JavaScript : dépendances de production de l'interface, et Svelte
// (son exécution est incluse dans l'interface compilée).
const ui = join(root, "ui");
const uiPkg = JSON.parse(readFileSync(join(ui, "package.json"), "utf8"));
const npm = new Map();
const todo = [...Object.keys(uiPkg.dependencies ?? {}), "svelte"];
const resolvePkg = (name) =>
  [join(ui, "node_modules", name), join(root, "node_modules", name)].find((d) => existsSync(join(d, "package.json")));
while (todo.length) {
  const name = todo.pop();
  if (npm.has(name)) continue;
  const dir = resolvePkg(name);
  if (!dir) continue;
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  npm.set(name, { pkg, dir });
  todo.push(...Object.keys(pkg.dependencies ?? {}));
}

const lines = [];
const rule = "=".repeat(78);
lines.push(
  "Composants tiers de VERIFLOW et leurs licences",
  rule,
  "",
  `Généré par scripts/third-party.mjs pour ${target}.`,
  "VERIFLOW est sous GNU GPL version 3 ou ultérieure ; ces composants gardent",
  "leur propre licence, toutes compatibles avec la GPLv3. Les programmes ffmpeg",
  "et ffprobe sont décrits dans FFMPEG.txt.",
  "",
  `Crates Rust (${crates.length}) et paquets JavaScript (${npm.size}) :`,
  "",
);
for (const c of crates) lines.push(`  ${c.name} ${c.version} : ${c.license ?? c.license_file ?? "?"}`);
for (const [name, { pkg }] of [...npm].sort()) lines.push(`  ${name} ${pkg.version} (npm) : ${pkg.license ?? "?"}`);
lines.push("");

const missing = [];
const section = (title, license, texts) => {
  lines.push(rule, title, `Licence : ${license}`, rule, "");
  if (texts.length === 0) {
    missing.push(title);
    lines.push(`(Aucun fichier de licence dans le paquet : texte standard de la licence ${license}.)`, "");
  }
  for (const t of texts) lines.push(`--- ${t.file} ---`, t.text, "");
};
for (const c of crates) section(`${c.name} ${c.version}`, c.license ?? "?", licenseTexts(dirname(c.manifest_path)));
for (const [name, { pkg, dir }] of [...npm].sort()) section(`${name} ${pkg.version} (npm)`, pkg.license ?? "?", licenseTexts(dir));

// Textes standard des licences courantes, pour les paquets qui ne
// fournissent pas leur propre fichier (repris d'un paquet qui les contient).
const standard = [
  ["Apache-2.0", /^licen[cs]e-apache/i],
  ["MIT", /^licen[cs]e-mit/i],
];
lines.push(rule, "Textes standard des licences", rule, "");
for (const [id, re] of standard) {
  const src = crates.find((c) => licenseTexts(dirname(c.manifest_path)).some((t) => re.test(t.file)));
  const text = src && licenseTexts(dirname(src.manifest_path)).find((t) => re.test(t.file));
  if (text) lines.push(`--- ${id} (texte repris de ${src.name}) ---`, text.text, "");
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, lines.join("\n") + "\n");
console.log(`${out} : ${crates.length} crates, ${npm.size} paquets npm, ${missing.length} sans fichier de licence`);
if (missing.length) console.log(`Sans fichier de licence : ${missing.join(", ")}`);
// Licences refusées : incompatibles avec la GPLv3 ou non libres.
const bad = [...crates.map((c) => [c.name, c.license ?? ""]), ...[...npm].map(([n, { pkg }]) => [n, pkg.license ?? ""])].filter(
  ([, l]) => /\b(GPL-2\.0-only|SSPL|BUSL|Commons-Clause|proprietary|UNLICENSED)\b/i.test(l) && !/\bOR\b/.test(l),
);
if (bad.length) {
  console.error(`ERREUR : licences incompatibles : ${bad.map(([n, l]) => `${n} (${l})`).join(", ")}`);
  process.exit(1);
}
