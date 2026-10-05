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

# Compatible bash 3.2 (macOS) : pas de tableaux associatifs.
mr_path() {
  case "$1" in
    macos-arm64) echo "macos/arm64/1789931890_${VERSION}" ;;
    macos-amd64) echo "macos/amd64/1789931006_${VERSION}" ;;
    linux-amd64) echo "linux/amd64/1789931100_${VERSION}" ;;
  esac
}
mr_sha() {
  case "$1" in
    macos-arm64-ffmpeg) echo "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924" ;;
    macos-arm64-ffprobe) echo "fcbe839537485eaee7a7a8bc5cbc0f90d53617e80943e8a5b2e31cb851197ea6" ;;
    macos-amd64-ffmpeg) echo "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4" ;;
    macos-amd64-ffprobe) echo "2322438ed2f6319a691291b247d09c69dcaa3a982460d1f269a7e1af335cfdfd" ;;
    linux-amd64-ffmpeg) echo "fa8ecf4abbd290d98f7d188b8649cc6b391ae209a98452be955a15aab1909d7f" ;;
    linux-amd64-ffprobe) echo "3f428c49070be3d24ec338602b76d412e401ffcb8a5641ef0e729181a232fc32" ;;
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
    for tool in ffmpeg ffprobe; do fetch_mr macos-amd64 "$tool" "$DEST/veriflow-$tool-$TARGET"; done
    ;;
  universal-apple-darwin)
    # Binaire universel (Apple Silicon + Intel) assemblé avec lipo, puis signé ad hoc.
    for tool in ffmpeg ffprobe; do
      fetch_mr macos-arm64 "$tool" "$WORK/$tool-arm64"
      fetch_mr macos-amd64 "$tool" "$WORK/$tool-x86_64"
      lipo -create "$WORK/$tool-arm64" "$WORK/$tool-x86_64" -output "$DEST/veriflow-$tool-$TARGET"
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
ls -l "$DEST"/veriflow-*-"$TARGET"*
