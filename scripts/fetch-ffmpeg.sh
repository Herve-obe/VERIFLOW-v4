#!/usr/bin/env bash
# Télécharge FFmpeg / FFprobe statiques (GPL) pour la plateforme cible, vérifie
# leur empreinte SHA-256 et les place dans src-tauri/binaries/ sous le nom
# attendu par Tauri : veriflow-ffmpeg-<cible>[.exe] (charte §5.2, décision du 05/10/2026).
#
# Usage : scripts/fetch-ffmpeg.sh <cible>
#   cibles : x86_64-pc-windows-msvc | x86_64-unknown-linux-gnu |
#            aarch64-apple-darwin | x86_64-apple-darwin | universal-apple-darwin
set -euo pipefail

TARGET="${1:?cible manquante (ex. x86_64-unknown-linux-gnu)}"
VERSION="9.0.2"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/binaries"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$DEST"

# Sources épinglées (version + empreinte) : une compilation donne toujours le
# même FFmpeg, dont on connaît exactement le contenu et les sources (GPLv3 §6).
# Windows et Linux : builds statiques GPL de BtbN (variantes win64-gpl et
# linux64-gpl, branche 9.0), sans --enable-nonfree, au même autobuild de fin
# de mois (BtbN conserve ceux-là durablement). Leurs scripts fixent le commit
# de chaque bibliothèque : l'archive des sources des Releases les reprend.
BTBN="https://github.com/BtbN/FFmpeg-Builds/releases/download"
BTBN_TAG="autobuild-2026-09-30-13-08"
BTBN_LINUX="ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz"
BTBN_LINUX_SHA="68ee646831adaae2495618346f3bba94ff207ff83bbd34d643e7004730d66269"
BTBN_WIN="ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-9.0.zip"
BTBN_WIN_SHA="a0e45723c72141975f51d8666302e614711745f3102b704ca3f82c897a58d278"
# macOS : FFmpeg compilé par VERIFLOW (scripts/build-ffmpeg-macos.sh, job
# « ffmpeg-macos » du build), une fois par architecture ; dossier passé dans
# FFMPEG_MACOS_DIR, avec un sous-dossier arm64 et un sous-dossier x86_64.
# Version minimale de macOS acceptée pour les binaires Intel embarqués.
MACOS_INTEL_MIN="10.15"

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# download <url> <fichier> <empreinte attendue>
download() {
  echo "Téléchargement : $1"
  if ! curl -fsSL --retry 4 --retry-delay 5 --retry-all-errors -o "$2" "$1"; then
    echo "ERREUR : téléchargement impossible : $1" >&2
    [ -n "${GITHUB_ACTIONS:-}" ] && echo "::error title=Téléchargement FFmpeg::$1"
    exit 1
  fi
  local got
  got="$(sha256 "$2")"
  if [ -z "$3" ]; then
    # Source pas encore épinglée : l'empreinte est affichée pour être recopiée ici.
    echo "AVERTISSEMENT : empreinte à épingler pour $1 : $got" >&2
    # Dans GitHub Actions : annotation visible dans le résumé du build.
    [ -n "${GITHUB_ACTIONS:-}" ] && echo "::warning title=Empreinte à épingler::$1 $got"
    return 0
  fi
  if [ "$got" != "$3" ]; then
    echo "ERREUR : empreinte SHA-256 incorrecte pour $1" >&2
    echo "  attendue : $3" >&2
    echo "  obtenue  : $got" >&2
    exit 1
  fi
}

# Récupère ffmpeg et ffprobe depuis BtbN : fetch_btbn <fichier> <empreinte> <destination sans suffixe> [.exe]
fetch_btbn() {
  local asset="$1" dir="${1%.tar.xz}"
  dir="${dir%.zip}"
  download "$BTBN/$BTBN_TAG/$asset" "$WORK/$asset" "$2"
  case "$asset" in
    *.zip) unzip -o -q "$WORK/$asset" -d "$WORK" ;;
    *) tar -xJf "$WORK/$asset" -C "$WORK" ;;
  esac
  for tool in ffmpeg ffprobe; do
    cp "$WORK/$dir/bin/$tool${4:-}" "$3-$tool${4:-}"
    chmod +x "$3-$tool${4:-}"
  done
}

# FFmpeg macOS compilé par VERIFLOW : copy_macos <arm64|x86_64> <outil> <destination>
copy_macos() {
  local src="${FFMPEG_MACOS_DIR:?FFMPEG_MACOS_DIR : dossier du FFmpeg macOS compilé par le job ffmpeg-macos}/$1/$2"
  [ -s "$src" ] || { echo "ERREUR : $src absent" >&2; exit 1; }
  cp "$src" "$3"
  chmod +x "$3"
}

# Version de macOS exigée par un binaire (LC_BUILD_VERSION ou LC_VERSION_MIN_MACOSX).
macos_min() {
  otool -l "$1" | awk '
    /LC_BUILD_VERSION/ { b = 1 } b && $1 == "minos" { print $2; exit }
    /LC_VERSION_MIN_MACOSX/ { v = 1 } v && $1 == "version" { print $2; exit }'
}

# Refuse un binaire qui exige un macOS plus récent que <max> (ex. 10.15).
check_macos_min() {
  local min
  min="$(macos_min "$1")"
  num() { echo "$1" | awk -F. '{ printf "%d", $1 * 100 + $2 }'; }
  echo "$(basename "$1") : macOS minimum $min ; bibliothèques :"
  otool -L "$1" | tail -n +2
  if [ -z "$min" ] || [ "$(num "$min")" -gt "$(num "$2")" ]; then
    echo "ERREUR : $(basename "$1") exige macOS ${min:-inconnu}, au-delà de $2" >&2
    exit 1
  fi
}

case "$TARGET" in
  x86_64-pc-windows-msvc)
    fetch_btbn "$BTBN_WIN" "$BTBN_WIN_SHA" "$WORK/veriflow" .exe
    for tool in ffmpeg ffprobe; do mv "$WORK/veriflow-$tool.exe" "$DEST/veriflow-$tool-$TARGET.exe"; done
    ;;
  x86_64-unknown-linux-gnu)
    fetch_btbn "$BTBN_LINUX" "$BTBN_LINUX_SHA" "$WORK/veriflow"
    for tool in ffmpeg ffprobe; do mv "$WORK/veriflow-$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  aarch64-apple-darwin)
    for tool in ffmpeg ffprobe; do copy_macos arm64 "$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  x86_64-apple-darwin)
    for tool in ffmpeg ffprobe; do
      copy_macos x86_64 "$tool" "$DEST/veriflow-$tool-$TARGET"
      check_macos_min "$DEST/veriflow-$tool-$TARGET" "$MACOS_INTEL_MIN"
    done
    ;;
  universal-apple-darwin)
    # Binaire universel (Apple Silicon + Intel) assemblé avec lipo, puis signé ad hoc.
    # Tauri compile aussi chaque architecture séparément : il lui faut les
    # versions arm64 et x86_64 en plus de la version universelle.
    for tool in ffmpeg ffprobe; do
      copy_macos arm64 "$tool" "$DEST/veriflow-$tool-aarch64-apple-darwin"
      copy_macos x86_64 "$tool" "$DEST/veriflow-$tool-x86_64-apple-darwin"
      check_macos_min "$DEST/veriflow-$tool-x86_64-apple-darwin" "$MACOS_INTEL_MIN"
      lipo -create "$DEST/veriflow-$tool-aarch64-apple-darwin" "$DEST/veriflow-$tool-x86_64-apple-darwin" \
        -output "$DEST/veriflow-$tool-$TARGET"
      codesign --force --sign - "$DEST/veriflow-$tool-$TARGET"
      chmod +x "$DEST/veriflow-$tool-$TARGET"
    done
    ;;
  *)
    echo "Cible non gérée : $TARGET" >&2
    exit 1
    ;;
esac

echo "FFmpeg $VERSION prêt pour $TARGET :"
ls -l "$DEST"/veriflow-*
