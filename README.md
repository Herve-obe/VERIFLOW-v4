# VERIFLOW

Application de bureau locale et légère pour la gestion des rushes audio et vidéo après le tournage :
copie sécurisée avec checksums et ASC MHL, visionnage, logs, synchronisation, transcodage et rapports.

Windows 10/11, macOS 12+ (Apple Silicon et Intel), Linux.

**Statut : développement (phase 1 : OFFLOAD).** La charte du projet est dans [`docs/01_CHARTE.md`](docs/01_CHARTE.md).

## Onglets

Deux modes, **VIDEO** et **AUDIO** (bascule avec `Tab`), contenant chacun :
`OFFLOAD` `MEDIA` `PLAYER` `SYNC` `TRANSCODE` `REPORT`.

## Développement

Prérequis : Node.js 22, Rust stable, FFmpeg (ffmpeg et ffprobe dans le PATH) et les [dépendances système de Tauri 2](https://v2.tauri.app/start/prerequisites/).

Les installeurs embarquent FFmpeg 9.0.2 (GPL). Pour produire un installeur complet en local :

```bash
bash scripts/fetch-ffmpeg.sh x86_64-unknown-linux-gnu   # ou x86_64-pc-windows-msvc, universal-apple-darwin
npm run tauri build -- --config src-tauri/tauri.ffmpeg.conf.json
```

```bash
npm ci                    # dépendances
npm run tauri dev         # lancer l'application en développement
cargo test --workspace    # tests
npm run check             # vérification de l'interface
npm run tauri build       # installeur pour le système courant
```

## Organisation

| Dossier | Contenu |
|---|---|
| `ui/` | Interface graphique (Svelte 5 + TypeScript). Visuel centralisé dans `ui/src/theme/tokens.css` |
| `core/` | Cœur métier en Rust, indépendant de l'interface |
| `src-tauri/` | Pont Tauri 2 entre l'interface et le cœur |
| `docs/` | Charte et documentation |

## Licence

GPL-3.0-or-later. Voir [`LICENSE`](LICENSE). Le nom et le logo VERIFLOW ne sont pas couverts par cette licence.
