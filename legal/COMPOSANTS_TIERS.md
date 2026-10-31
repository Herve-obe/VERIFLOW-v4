# Composants tiers livrés avec VERIFLOW

Version du 20/10/2026. Les licences sont données à titre indicatif ; le texte qui fait foi est celui de chaque composant :
- dans sa source, pour FFmpeg et ses bibliothèques (archives `ffmpeg-sources-*` des versions publiées) ;
- dans `THIRD_PARTY.txt`, pour les bibliothèques Rust et JavaScript.

## FFmpeg (ffmpeg et ffprobe 9.0.2)

FFmpeg est sous GNU LGPL 2.1 ou ultérieure. Il est compilé ici avec `--enable-gpl --enable-version3` et sans `--enable-nonfree`, ce qui place les binaires livrés sous **GNU GPL version 3**. Ils sont installés à côté de VERIFLOW (`veriflow-ffmpeg`, `veriflow-ffprobe`) et exécutés comme des programmes séparés.

| Système | Origine | Code source correspondant |
|---|---|---|
| Windows x64 | BtbN/FFmpeg-Builds, autobuild `autobuild-2026-09-30-13-08`, `ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-9.0.zip` | `ffmpeg-sources-9.0.2-windows-linux.tar` |
| Linux x64 | même autobuild, `ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz` | `ffmpeg-sources-9.0.2-windows-linux.tar` |
| macOS (Apple Silicon et Intel) | compilé par VERIFLOW, `scripts/build-ffmpeg-macos.sh` | `ffmpeg-sources-9.0.2-macos.tar` |

### Bibliothèques compilées dans FFmpeg

`W` : Windows, `L` : Linux, `M` : macOS.

| Bibliothèque | Licence principale | W | L | M |
|---|---|---|---|---|
| x264 | GPL-2.0-or-later | x | x | x |
| x265 | GPL-2.0-or-later | x | x | x |
| libvpx | BSD-3-Clause | x | x | x |
| SVT-AV1 | BSD-3-Clause-Clear et licence de brevets AOMedia 1.0 | x | x | x |
| libaom | BSD-2-Clause et licence de brevets AOMedia 1.0 | x | x | |
| dav1d | BSD-2-Clause | x | x | |
| rav1e | BSD-2-Clause et licence de brevets AOMedia 1.0 | x | x | |
| vvenc | BSD-3-Clause-Clear | x | x | |
| kvazaar | BSD-3-Clause | x | x | |
| OpenH264 | BSD-2-Clause | x | x | |
| xvid | GPL-2.0-or-later | x | x | |
| davs2, xavs2 | GPL-2.0-or-later | x | x | |
| uavs3d | BSD-3-Clause (à vérifier) | x | x | |
| OpenAPV (liboapv) | BSD-3-Clause | x | x | |
| LCEVCdec | BSD-3-Clause-Clear (à vérifier) | x | x | |
| OpenJPEG | BSD-2-Clause | x | x | |
| libjxl | BSD-3-Clause | x | x | |
| libwebp | BSD-3-Clause | x | x | x |
| snappy (HAP) | BSD-3-Clause | x | x | x |
| libtheora | BSD-3-Clause | x | x | |
| Opus | BSD-3-Clause | x | x | x |
| libogg, libvorbis | BSD-3-Clause | x | x | x |
| LAME | LGPL-2.0-or-later | x | x | x |
| TwoLAME | LGPL-2.1-or-later | x | x | |
| opencore-amr | Apache-2.0 | x | x | |
| libopenmpt | BSD-3-Clause | x | x | |
| Game Music Emu | LGPL-2.1-or-later | x | x | |
| soxr | LGPL-2.1-or-later | x | x | |
| Rubber Band | GPL-2.0-or-later | x | x | |
| Chromaprint | LGPL-2.1-or-later | x | x | |
| libvmaf | BSD-2-Clause-Patent | x | x | x |
| vid.stab | GPL-2.0-or-later | x | x | |
| frei0r | GPL-2.0-or-later | x | x | |
| zimg | WTFPL | x | x | |
| libplacebo | LGPL-2.1-or-later | x | x | |
| libass | ISC | x | x | x |
| FreeType | FreeType License ou GPL-2.0-or-later | x | x | x |
| FriBidi | LGPL-2.1-or-later | x | x | x |
| HarfBuzz | MIT | x | x | x |
| Fontconfig | MIT (variante) | x | x | |
| librsvg | LGPL-2.1-or-later | x | x | |
| libaribb24 | LGPL-3.0 (à vérifier) | x | x | |
| libaribcaption | MIT | x | x | |
| libzvbi | GPL-2.0-or-later et LGPL selon les fichiers (à vérifier) | x | x | |
| libbluray | LGPL-2.1-or-later | x | x | |
| libdvdread, libdvdnav | GPL-2.0-or-later | x | x | |
| SRT | MPL-2.0 | x | x | |
| librist | BSD-2-Clause | x | x | |
| libssh | LGPL-2.1 | x | x | |
| ZeroMQ | MPL-2.0 | x | x | |
| libxml2 | MIT | x | x | |
| OpenSSL | Apache-2.0 | | x | |
| OpenAL Soft | LGPL-2.0-or-later | x | x | |
| SDL2 | Zlib | x | x | |
| LV2, lilv | ISC | x | x | |
| GMP | LGPL-3.0-or-later ou GPL-2.0-or-later | x | x | |
| libiconv | LGPL-2.1-or-later | x | x | |
| xz (lzma) | 0BSD et domaine public | x | x | |
| zlib | Zlib | x | x | système |
| Vulkan (en-têtes, chargeur) | Apache-2.0 | x | x | |
| OpenCL (chargeur ICD) | Apache-2.0 | x | x | |
| libvpl (Intel) | MIT | x | x | |
| en-têtes AMF, nv-codec-headers | MIT | x | x | |
| libva, libdrm, libxcb, Xlib | MIT | | x | |
| PulseAudio (client) | LGPL-2.1-or-later | | x | |
| AviSynth+ (en-têtes, chargé s'il est installé) | GPL-2.0-or-later | x | x | |
| exécution mingw-w64 et GCC | ZPL-2.1, domaine public ; GPL-3.0 avec exception d'exécution GCC | x | | |

S'y ajoutent les dépendances de ces bibliothèques (brotli, libpng, libjpeg-turbo, pcre2, libffi, fftw3, libsamplerate, libunibreak, libudfread et d'autres). Leurs sources sont dans l'archive ; leurs commits et options sont dans `scripts.d` des scripts BtbN.

Fonctions du système utilisées sous macOS : VideoToolbox, AudioToolbox, CoreText. Sous Windows : Media Foundation, Schannel, Direct3D.

## Bibliothèques Rust et JavaScript

Environ 520 crates Rust et 24 paquets JavaScript, dont :
- Tauri (MIT ou Apache-2.0) ;
- Svelte (MIT) ;
- SQLite par rusqlite (domaine public, MIT) ;
- Typst (Apache-2.0) ;
- cpal, rubato, realfft (MIT ou Apache-2.0) ;
- pdf-writer (MIT ou Apache-2.0) ;
- rust_xlsxwriter (MIT ou Apache-2.0) ;
- ureq (MIT ou Apache-2.0).

Liste complète, versions et textes de licence : `THIRD_PARTY.txt`, généré à chaque compilation par `scripts/third-party.mjs`. La génération échoue si une licence incompatible avec la GPLv3 apparaît.

## Autres

- Polices **Inter** et **JetBrains Mono** : SIL Open Font License 1.1.
- **SDK ASIO** de Steinberg (Windows) : GNU GPL version 3 (double licence proposée par Steinberg depuis octobre 2025).
