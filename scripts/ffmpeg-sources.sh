#!/usr/bin/env bash
# Rassemble le code source correspondant des FFmpeg livrés avec VERIFLOW
# (GPLv3 §6d : sources accessibles au même endroit que les installeurs).
# Lancé par le build (job « sources ») sur Linux avec Docker ; produit deux
# archives jointes à la Release :
#   ffmpeg-sources-<version>-windows-linux.tar : FFmpeg au commit des builds
#     BtbN, les scripts BtbN de l'autobuild épinglé et la source de chaque
#     bibliothèque, téléchargée par leur propre outil (download.sh) ;
#   ffmpeg-sources-<version>-macos.tar : FFmpeg 9.0.2, chaque bibliothèque au
#     commit épinglé et le script de compilation de VERIFLOW.
# Chaque archive contient un LISEZMOI.txt et les empreintes SHA256SUMS.
#
# Usage : scripts/ffmpeg-sources.sh <dossier de sortie>
set -euo pipefail

OUT="$(mkdir -p "${1:?dossier de sortie manquant}" && cd "$1" && pwd)"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

VERSION="9.0.2"
# Doivent suivre scripts/fetch-ffmpeg.sh.
BTBN_REPO="https://github.com/BtbN/FFmpeg-Builds.git"
BTBN_TAG="autobuild-2026-09-30-13-08"
FFMPEG_REPO="https://github.com/FFmpeg/FFmpeg.git"
FFMPEG_BRANCH="release/9.0"
FFMPEG_COMMIT="2a571b6068"

log() { echo "== $*"; }

sums() { # dossier
  (cd "$1" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS)
}

# --- Windows et Linux (BtbN) ---------------------------------------------
WL="$WORK/ffmpeg-sources-$VERSION-windows-linux"
mkdir -p "$WL"

log "FFmpeg $FFMPEG_BRANCH au commit $FFMPEG_COMMIT"
git init -q "$WORK/ffmpeg"
git -C "$WORK/ffmpeg" fetch -q --depth 200 "$FFMPEG_REPO" "$FFMPEG_BRANCH"
full="$(git -C "$WORK/ffmpeg" rev-parse "$FFMPEG_COMMIT^{commit}")"
git -C "$WORK/ffmpeg" archive --format=tar --prefix="ffmpeg-$full/" "$full" | xz -T0 >"$WL/ffmpeg-$full.tar.xz"

log "Scripts BtbN ($BTBN_TAG)"
git init -q "$WORK/btbn"
git -C "$WORK/btbn" fetch -q --depth 1 "$BTBN_REPO" "refs/tags/$BTBN_TAG:refs/tags/$BTBN_TAG"
git -C "$WORK/btbn" -c advice.detachedHead=false checkout -q "$BTBN_TAG"
btbn_commit="$(git -C "$WORK/btbn" rev-parse HEAD)"
git -C "$WORK/btbn" archive --format=tar --prefix="FFmpeg-Builds-$BTBN_TAG/" HEAD | xz -T0 >"$WL/FFmpeg-Builds-$BTBN_TAG.tar.xz"

log "Sources des bibliothèques (download.sh de BtbN, Docker)"
# Image Docker de BtbN (outils de téléchargement seulement) : sans cela,
# download.sh prend le nom du dépôt courant (GITHUB_REPOSITORY) dans la CI.
(cd "$WORK/btbn" && GITHUB_REPOSITORY=BtbN/FFmpeg-Builds ./download.sh)
mkdir -p "$WL/bibliotheques"
# Les liens symboliques <nom>.tar.xz pointent vers <nom>_<empreinte>.tar.xz.
for f in "$WORK/btbn/.cache/downloads"/*.tar.xz; do
  [ -L "$f" ] && continue
  cp "$f" "$WL/bibliotheques/"
done

cat >"$WL/LISEZMOI.txt" <<EOF
Code source correspondant des FFmpeg Windows et Linux livrés avec VERIFLOW
=========================================================================

Binaires : builds statiques GPL de BtbN/FFmpeg-Builds, autobuild $BTBN_TAG
(commit $btbn_commit), fichiers :
  ffmpeg-n9.0.2-17-g2a571b6068-win64-gpl-9.0.zip
  ffmpeg-n9.0.2-17-g2a571b6068-linux64-gpl-9.0.tar.xz

Contenu :
  ffmpeg-$full.tar.xz
      FFmpeg, branche $FFMPEG_BRANCH, commit $full.
  FFmpeg-Builds-$BTBN_TAG.tar.xz
      Scripts de compilation de BtbN à cet autobuild (scripts.d : commit
      épinglé de chaque bibliothèque et options de compilation ; variants :
      options de FFmpeg, --enable-gpl --enable-version3).
  bibliotheques/
      Source de chaque bibliothèque, telle que téléchargée par download.sh
      de BtbN (un fichier par script de scripts.d, nommé d'après lui).
  SHA256SUMS
      Empreintes de tous les fichiers.

Recompiler : décompresser FFmpeg-Builds, placer le contenu de bibliotheques/
dans .cache/downloads/, puis lancer « ./build.sh win64 gpl 9.0 » ou
« ./build.sh linux64 gpl 9.0 » (Docker requis, voir le README de BtbN).

Licence : FFmpeg et ces binaires sont sous GNU GPL version 3 ; chaque
bibliothèque garde sa propre licence (fichiers COPYING ou LICENSE dans sa
source).
EOF
sums "$WL"

# --- macOS (compilé par VERIFLOW) ----------------------------------------
MAC="$WORK/ffmpeg-sources-$VERSION-macos"
mkdir -p "$MAC"
log "Sources macOS (scripts/build-ffmpeg-macos.sh sources)"
FFBUILD_WORK="$WORK/macbuild" bash "$ROOT/scripts/build-ffmpeg-macos.sh" sources "$MAC"
cat >"$MAC/LISEZMOI.txt" <<EOF
Code source correspondant des FFmpeg macOS livrés avec VERIFLOW
==============================================================

Binaires : FFmpeg $VERSION compilé par VERIFLOW avec build-ffmpeg-macos.sh
(inclus), pour Apple Silicon (macOS 11 et plus) et Intel (macOS 10.15 et
plus), assemblés en binaire universel.

Contenu : une archive .tar.xz par composant (ffmpeg, et chaque bibliothèque
au commit indiqué dans le script), le script de compilation, et SHA256SUMS.

Recompiler : sur un Mac avec les outils de compilation (Xcode, Homebrew :
nasm meson ninja cmake pkg-config autoconf automake libtool), depuis ce
dossier : « FFBUILD_SOURCES=\$PWD bash build-ffmpeg-macos.sh build sortie ».

Licence : FFmpeg et ces binaires sont sous GNU GPL version 3 ; chaque
bibliothèque garde sa propre licence (fichiers COPYING ou LICENSE dans sa
source).
EOF
sums "$MAC"

log "Archives finales"
# GitHub refuse les fichiers de Release de 2 Gio ou plus : une archive plus
# grosse est découpée en morceaux de 1 900 Mio (.part00, .part01...).
PART=$((1900 * 1024 * 1024))
for d in "$WL" "$MAC"; do
  tar="$OUT/$(basename "$d").tar"
  tar -C "$WORK" -cf "$tar" "$(basename "$d")"
  if [ "$(stat -c %s "$tar")" -gt "$PART" ]; then
    split -b "$PART" -d -a 2 "$tar" "$tar.part"
    rm "$tar"
  fi
done
cat >"$OUT/ffmpeg-sources-$VERSION-LISEZMOI.txt" <<EOF
Code source correspondant des FFmpeg livrés avec VERIFLOW (GPLv3 §6 d)
=====================================================================

ffmpeg-sources-$VERSION-windows-linux.tar : FFmpeg Windows et Linux (BtbN).
ffmpeg-sources-$VERSION-macos.tar : FFmpeg macOS (compilé par VERIFLOW).
ffmpeg-sources-$VERSION.sha256 : empreintes SHA-256 des fichiers publiés.

Une archive de plus de 1 900 Mio est publiée en morceaux (.part00,
.part01...), car GitHub limite chaque fichier à 2 Gio. Pour la reconstituer :
  Linux, macOS : cat <archive>.tar.part* > <archive>.tar
  Windows      : copy /b <archive>.tar.part00 + <archive>.tar.part01 <archive>.tar
Chaque archive contient un LISEZMOI.txt qui explique comment recompiler.
EOF
(cd "$OUT" && sha256sum ffmpeg-sources-*.tar* "ffmpeg-sources-$VERSION-LISEZMOI.txt" >"ffmpeg-sources-$VERSION.sha256")
ls -l "$OUT"
