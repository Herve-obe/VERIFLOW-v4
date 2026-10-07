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

# Sources épinglées (version + empreinte) : une compilation donne toujours le même FFmpeg.
WIN_URL="https://github.com/GyanD/codexffmpeg/releases/download/${VERSION}/ffmpeg-${VERSION}-essentials_build.zip"
WIN_SHA="60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"
MR="https://ffmpeg.martin-riedl.de/download"
# macOS Intel : builds statiques GPL d'evermeet.cx, compilés pour macOS 10.13
# et plus (ceux de martin-riedl.de exigent macOS 12). Permet Catalina (10.15).
EV="https://evermeet.cx/ffmpeg"
# Version minimale de macOS acceptée pour les binaires Intel embarqués.
MACOS_INTEL_MIN="10.15"

# Compatible bash 3.2 (macOS) : pas de tableaux associatifs.
mr_path() {
  case "$1" in
    macos-arm64) echo "macos/arm64/1789931890_${VERSION}" ;;
    linux-amd64) echo "linux/amd64/1789931100_${VERSION}" ;;
  esac
}
mr_sha() {
  case "$1" in
    macos-arm64-ffmpeg) echo "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924" ;;
    macos-arm64-ffprobe) echo "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6" ;;
    linux-amd64-ffmpeg) echo "fa8ecf4abbd290d98f7d188b8649cc6b391ae209a98452be955a15aab1909d7f" ;;
    linux-amd64-ffprobe) echo "3f428c49070be3d24ec338602b76d412e401ffcb8a5641ef0e729181a232fc32" ;;
  esac
}

ev_sha() {
  case "$1" in
    ffmpeg) echo "4acc0be580f9b2788029eb7bd4d645ff87968911b0a62aeeb3940d42d54558d5" ;;
    ffprobe) echo "24a9c968cd4da72d99c7245e914b921815835eb6dff01d99868031aebaf1d439" ;;
  esac
}

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# download <url> <fichier> <empreinte attendue>
download() {
  echo "Téléchargement : $1"
  curl -fsSL --retry 4 --retry-delay 5 -o "$2" "$1"
  local got
  got="$(sha256 "$2")"
  if [ -z "$3" ]; then
    # Source pas encore épinglée : l'empreinte est affichée pour être recopiée ici.
    echo "AVERTISSEMENT : empreinte à épingler pour $1 : $got" >&2
    return 0
  fi
  if [ "$got" != "$3" ]; then
    echo "ERREUR : empreinte SHA-256 incorrecte pour $1" >&2
    echo "  attendue : $3" >&2
    echo "  obtenue  : $got" >&2
    exit 1
  fi
}

# Récupère un outil depuis martin-riedl.de : fetch_mr <plateforme> <outil> <destination>
fetch_mr() {
  download "$MR/$(mr_path "$1")/$2.zip" "$WORK/$1-$2.zip" "$(mr_sha "$1-$2")"
  unzip -o -q "$WORK/$1-$2.zip" -d "$WORK/$1-$2"
  cp "$WORK/$1-$2/$2" "$3"
  chmod +x "$3"
}

# Récupère un outil depuis evermeet.cx (macOS Intel) : fetch_ev <outil> <destination>
fetch_ev() {
  download "$EV/$1-${VERSION}.zip" "$WORK/ev-$1.zip" "$(ev_sha "$1")"
  unzip -o -q "$WORK/ev-$1.zip" -d "$WORK/ev-$1"
  cp "$WORK/ev-$1/$1" "$2"
  chmod +x "$2"
  check_macos_min "$2" "$MACOS_INTEL_MIN"
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
    download "$WIN_URL" "$WORK/win.zip" "$WIN_SHA"
    unzip -o -q "$WORK/win.zip" -d "$WORK/win"
    for tool in ffmpeg ffprobe; do
      cp "$WORK/win/ffmpeg-${VERSION}-essentials_build/bin/$tool.exe" "$DEST/veriflow-$tool-$TARGET.exe"
    done
    ;;
  x86_64-unknown-linux-gnu)
    for tool in ffmpeg ffprobe; do fetch_mr linux-amd64 "$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  aarch64-apple-darwin)
    for tool in ffmpeg ffprobe; do fetch_mr macos-arm64 "$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  x86_64-apple-darwin)
    for tool in ffmpeg ffprobe; do fetch_ev "$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  universal-apple-darwin)
    # Binaire universel (Apple Silicon + Intel) assemblé avec lipo, puis signé ad hoc.
    # Tauri compile aussi chaque architecture séparément : il lui faut les
    # versions arm64 et x86_64 en plus de la version universelle.
    for tool in ffmpeg ffprobe; do
      fetch_mr macos-arm64 "$tool" "$DEST/veriflow-$tool-aarch64-apple-darwin"
      fetch_ev "$tool" "$DEST/veriflow-$tool-x86_64-apple-darwin"
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
