#!/usr/bin/env bash
# Compile FFmpeg et FFprobe statiques (GPLv3) pour macOS, avec les
# bibliothèques dont VERIFLOW a besoin. Chaque source est épinglée : commit
# git (les mêmes que les builds BtbN de Windows et Linux) ou archive avec son
# empreinte SHA-256. On sait ainsi exactement ce que contient le FFmpeg
# distribué, et on peut en fournir le code source complet (GPLv3 §6).
#
# Usage :
#   scripts/build-ffmpeg-macos.sh build <dossier de sortie>
#       compile pour l'architecture de la machine (arm64 : macOS 11 et plus ;
#       x86_64 : macOS 10.15 et plus) ; écrit ffmpeg, ffprobe et
#       BUILDINFO.txt (sources, options) dans le dossier de sortie.
#   scripts/build-ffmpeg-macos.sh sources <dossier de sortie>
#       télécharge seulement les sources et les range en archives .tar.xz
#       (archive des sources jointe aux Releases).
#   FFBUILD_SOURCES=<dossier de ces archives> ... build <dossier>
#       recompile à partir de ces archives, sans rien télécharger.
#
# Sous Linux, « build » compile la même chose sans VideoToolbox ni CoreText :
# sert à vérifier le script hors d'un Mac.
set -euo pipefail

MODE="${1:?mode manquant : build ou sources}"
OUT="$(mkdir -p "${2:?dossier de sortie manquant}" && cd "$2" && pwd)"
SCRIPT="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
WORK="${FFBUILD_WORK:-$(mktemp -d)}"
SRC="$WORK/src"
PREFIX="$WORK/prefix"
mkdir -p "$SRC" "$PREFIX"

FFMPEG_VERSION="9.0.2"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_SHA="8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e"

# nom|dépôt|commit (ceux de BtbN/FFmpeg-Builds, autobuild-2026-09-30-13-08).
GIT_SOURCES="
freetype|https://gitlab.freedesktop.org/freetype/freetype.git|d333439633039de426f943f28a2926c7f97b5ae5
fribidi|https://github.com/fribidi/fribidi.git|4c914a92e94a9fe4f30ae83a1130099841566448
harfbuzz|https://github.com/harfbuzz/harfbuzz.git|b3bab62307017934dd7f532c52a696fefb12b3ad
libass|https://github.com/libass/libass.git|f61db567e6593df3470e91594bcd4ad2d0473aff
x264|https://code.videolan.org/videolan/x264.git|0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee
x265|https://github.com/Multicorewareinc/x265.git|020d7054dbecfd3be8e62efe33ff8a305d41856e
libvpx|https://chromium.googlesource.com/webm/libvpx|5e680f30801d03c21078f8c4b772464752516211
svtav1|https://gitlab.com/AOMediaCodec/SVT-AV1.git|5ed9ffbcb5086da23f30fe9bc5b00145358462da
opus|https://github.com/xiph/opus.git|503d81b138d76621aae4b12786e90de48aa8db3a
ogg|https://github.com/xiph/ogg.git|06a5e0262cdc28aa4ae6797627a783b5010440f0
vorbis|https://github.com/xiph/vorbis.git|1b75110b5a2754ba1931d82dd83cb822b266a21d
vmaf|https://github.com/Netflix/vmaf.git|86da14d0306a138fd3f01319860b905169746516
snappy|https://github.com/google/snappy.git|9c28114a38866f6deeaa826db918293bc28ae410
libwebp|https://chromium.googlesource.com/webm/libwebp|5c168cd23b1872969c38e34b01b04dcf29849288
"
# LAME : dépôt Subversion, révision épinglée (comme BtbN).
LAME_REPO="https://svn.code.sf.net/p/lame/svn/trunk/lame"
LAME_REV="6835"

OS="$(uname -s)"
ARCH="$(uname -m)"
if [ "$OS" = "Darwin" ]; then
  if [ "$ARCH" = "arm64" ]; then MIN="11.0"; else MIN="10.15"; fi
  export MACOSX_DEPLOYMENT_TARGET="$MIN"
  JOBS="$(sysctl -n hw.ncpu)"
  CXXLIB="-lc++"
else
  JOBS="$(nproc)"
  CXXLIB="-lstdc++"
fi
export CFLAGS="-O2 -I$PREFIX/include ${MIN:+-mmacosx-version-min=$MIN}"
export CXXFLAGS="$CFLAGS"
export LDFLAGS="-L$PREFIX/lib ${MIN:+-mmacosx-version-min=$MIN}"
# Seules nos bibliothèques sont visibles : rien de Homebrew ni du système
# (sinon le FFmpeg dépendrait de bibliothèques absentes chez l'utilisateur).
export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig"
export PKG_CONFIG_PATH=""
export PATH="$PREFIX/bin:$PATH"

log() { echo "== $*"; }
fail() {
  echo "ERREUR : $*" >&2
  [ -n "${GITHUB_ACTIONS:-}" ] && echo "::error title=FFmpeg macOS::$*"
  exit 1
}

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# Exécute une étape ; en cas d'échec, les dernières lignes du journal sont
# remontées en annotation (lisibles dans le résumé du build GitHub).
step() {
  local name="$1"
  shift
  log "$name"
  # Sous-shell avec -e : la moindre commande en échec arrête l'étape (dans un
  # « if », -e serait ignoré).
  local rc=0
  set +e
  (
    set -e
    "$@"
  ) >"$WORK/$name.log" 2>&1
  rc=$?
  set -e
  if [ "$rc" -ne 0 ]; then
    tail -n 60 "$WORK/$name.log" >&2
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
      echo "::error title=FFmpeg macOS : $name::$(tail -n 25 "$WORK/$name.log" | tr '\n' ' ' | cut -c1-3000)"
      # Configuration de FFmpeg : la cause est dans config.log.
      if [ "$name" = "ffmpeg" ] && [ -f "$SRC/ffmpeg/ffbuild/config.log" ]; then
        echo "::error title=FFmpeg macOS : config.log::$(tail -n 50 "$SRC/ffmpeg/ffbuild/config.log" | tr '\n' ' ' | cut -c1-3500)"
      fi
    fi
    exit 1
  fi
}

fetch_git() { # nom dépôt commit
  local dir="$SRC/$1"
  [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$3" ] && return 0
  rm -rf "$dir"
  mkdir -p "$dir"
  git -C "$dir" init -q
  git -C "$dir" remote add origin "$2"
  # Commit seul ; sinon historique complet (serveur qui refuse ce mode).
  if ! git -C "$dir" fetch -q --depth 1 origin "$3" 2>/dev/null; then
    git -C "$dir" fetch -q origin
  fi
  git -C "$dir" -c advice.detachedHead=false checkout -q "$3"
  [ "$(git -C "$dir" rev-parse HEAD)" = "$3" ] || fail "$1 : commit $3 introuvable"
}

# LAME vient du Subversion de SourceForge, parfois injoignable : délai court
# et quatre tentatives.
svn_get() { # checkout|export dossier
  local i
  for i in 1 2 3 4; do
    rm -rf "$2"
    svn "$1" -q --config-option servers:global:http-timeout=60 "$LAME_REPO@$LAME_REV" "$2" && return 0
    echo "svn $1 : tentative $i échouée" >&2
    sleep $((i * 15))
  done
  fail "LAME r$LAME_REV : Subversion injoignable ($LAME_REPO)"
}

fetch_all() {
  local name repo commit
  # Recompilation depuis l'archive des sources d'une Release :
  # FFBUILD_SOURCES=<dossier des .tar.xz>, aucun téléchargement.
  if [ -n "${FFBUILD_SOURCES:-}" ]; then
    for line in $GIT_SOURCES; do
      IFS='|' read -r name repo commit <<<"$line"
      [ -d "$SRC/$name" ] && continue
      mkdir -p "$SRC/$name"
      tar -xJf "$FFBUILD_SOURCES/$name-$commit.tar.xz" -C "$SRC/$name" --strip-components 1
    done
    [ -d "$SRC/lame" ] || { mkdir -p "$SRC/lame" && tar -xJf "$FFBUILD_SOURCES/lame-r$LAME_REV.tar.xz" -C "$SRC/lame" --strip-components 1; }
    [ -d "$SRC/ffmpeg" ] || { mkdir -p "$SRC/ffmpeg" && tar -xJf "$FFBUILD_SOURCES/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$SRC/ffmpeg" --strip-components 1; }
    return 0
  fi
  for line in $GIT_SOURCES; do
    IFS='|' read -r name repo commit <<<"$line"
    log "source $name ($commit)"
    fetch_git "$name" "$repo" "$commit"
  done
  if [ ! -d "$SRC/lame" ]; then
    log "source lame (r$LAME_REV)"
    svn_get checkout "$SRC/lame"
  fi
  if [ ! -d "$SRC/ffmpeg" ]; then
    log "source ffmpeg $FFMPEG_VERSION"
    curl -fsSL --retry 4 -o "$WORK/ffmpeg.tar.xz" "$FFMPEG_URL"
    local got
    got="$(sha256 "$WORK/ffmpeg.tar.xz")"
    [ "$got" = "$FFMPEG_SHA" ] || fail "ffmpeg-$FFMPEG_VERSION.tar.xz : empreinte $got, attendue $FFMPEG_SHA"
    mkdir -p "$SRC/ffmpeg"
    tar -xJf "$WORK/ffmpeg.tar.xz" -C "$SRC/ffmpeg" --strip-components 1
  fi
}

cmake_build() { # dossier source, options...
  local src="$1"
  shift
  rm -rf "$src/_build"
  cmake -S "$src" -B "$src/_build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" -DCMAKE_PREFIX_PATH="$PREFIX" \
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DBUILD_SHARED_LIBS=OFF \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON "$@"
  cmake --build "$src/_build" -j "$JOBS"
  cmake --install "$src/_build"
}

meson_build() { # dossier source, options...
  local src="$1"
  shift
  rm -rf "$src/_build"
  meson setup "$src/_build" "$src" --prefix="$PREFIX" --libdir=lib --buildtype=release \
    --default-library=static "$@"
  ninja -C "$src/_build" -j "$JOBS"
  ninja -C "$src/_build" install
}

b_freetype() {
  cmake_build "$SRC/freetype" -DFT_DISABLE_BZIP2=ON -DFT_DISABLE_PNG=ON \
    -DFT_DISABLE_HARFBUZZ=ON -DFT_DISABLE_BROTLI=ON -DFT_DISABLE_ZLIB=ON -DFT_DISABLE_HVF=ON
}
b_fribidi() { meson_build "$SRC/fribidi" -Ddocs=false -Dbin=false -Dtests=false; }
b_harfbuzz() {
  meson_build "$SRC/harfbuzz" -Dfreetype=enabled -Dglib=disabled -Dgobject=disabled \
    -Dcairo=disabled -Dchafa=disabled -Dicu=disabled -Dtests=disabled -Dintrospection=disabled \
    -Ddocs=disabled -Dutilities=disabled
}
b_libass() {
  local font=(-Dfontconfig=disabled)
  [ "$OS" = "Darwin" ] && font+=(-Dcoretext=enabled)
  meson_build "$SRC/libass" -Dtest=disabled -Dcompare=disabled -Dprofile=disabled \
    -Dfuzz=disabled -Dcheckasm=disabled -Dlibunibreak=disabled -Drequire-system-font-provider=false "${font[@]}"
}
b_x264() {
  cd "$SRC/x264"
  ./configure --prefix="$PREFIX" --enable-static --enable-pic --disable-cli --disable-lavf \
    --disable-swscale
  make -j "$JOBS"
  make install
}
b_x265() {
  # 8, 10 et 12 bits dans une seule bibliothèque (comme BtbN).
  local s="$SRC/x265/source" b="$SRC/x265/_build"
  local common=(-G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$PREFIX"
    -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DENABLE_SHARED=OFF -DENABLE_CLI=OFF
    -DCMAKE_ASM_NASM_FLAGS=-w-macro-params-legacy)
  rm -rf "$b"
  # Version lue dans x265Version.txt (fourni par x265) : le dossier .git,
  # sans étiquettes ici, est masqué le temps de la configuration (sans
  # version, x265 n'installe pas x265.pc).
  [ -d "$SRC/x265/.git" ] && mv "$SRC/x265/.git" "$SRC/x265/.git-off"
  cmake "${common[@]}" -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF -DMAIN12=ON -S "$s" -B "$b/12"
  cmake "${common[@]}" -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF -S "$s" -B "$b/10"
  cmake --build "$b/12" -j "$JOBS"
  cmake --build "$b/10" -j "$JOBS"
  cp "$b/12/libx265.a" "$b/libx265_main12.a"
  cp "$b/10/libx265.a" "$b/libx265_main10.a"
  cmake "${common[@]}" -DEXTRA_LIB="x265_main10.a;x265_main12.a" -DEXTRA_LINK_FLAGS="-L$b" \
    -DLINKED_10BIT=ON -DLINKED_12BIT=ON -S "$s" -B "$b/8"
  [ -d "$SRC/x265/.git-off" ] && mv "$SRC/x265/.git-off" "$SRC/x265/.git"
  cmake --build "$b/8" -j "$JOBS"
  mv "$b/8/libx265.a" "$b/libx265_main.a"
  if [ "$OS" = "Darwin" ]; then
    libtool -static -o "$b/8/libx265.a" "$b/libx265_main.a" "$b/libx265_main10.a" "$b/libx265_main12.a"
  else
    printf 'CREATE %s\nADDLIB %s\nADDLIB %s\nADDLIB %s\nSAVE\nEND\n' "$b/8/libx265.a" \
      "$b/libx265_main.a" "$b/libx265_main10.a" "$b/libx265_main12.a" | ar -M
  fi
  cmake --install "$b/8"
  echo "Libs.private: $CXXLIB" >>"$PREFIX/lib/pkgconfig/x265.pc"
}
b_libvpx() {
  cd "$SRC/libvpx"
  local target
  case "$OS-$ARCH" in
    Darwin-arm64) target="arm64-darwin20-gcc" ;;
    Darwin-x86_64) target="x86_64-darwin19-gcc" ;;
    *) target="x86_64-linux-gcc" ;;
  esac
  ./configure --prefix="$PREFIX" --target="$target" --disable-shared --enable-static --enable-pic \
    --disable-examples --disable-tools --disable-docs --disable-unit-tests --enable-vp9-highbitdepth
  make -j "$JOBS"
  make install
}
b_svtav1() { cmake_build "$SRC/svtav1" -DBUILD_TESTING=OFF -DBUILD_APPS=OFF -DSVT_AV1_LTO=OFF; }
b_opus() { cmake_build "$SRC/opus" -DOPUS_BUILD_PROGRAMS=OFF -DOPUS_BUILD_TESTING=OFF; }
b_lame() {
  cd "$SRC/lame"
  # Dépôt Subversion : scripts de configuration à régénérer.
  autoreconf -fi
  CFLAGS="$CFLAGS -DNDEBUG" ./configure --prefix="$PREFIX" --disable-shared --enable-static \
    --disable-cpml --disable-frontend --disable-decoder
  make -j "$JOBS"
  make install
}
b_ogg() { cmake_build "$SRC/ogg" -DINSTALL_DOCS=OFF -DBUILD_TESTING=OFF; }
b_vorbis() { cmake_build "$SRC/vorbis" -DBUILD_TESTING=OFF; }
b_vmaf() {
  # Outils inutiles (et parfois cassés) retirés, comme BtbN.
  echo >"$SRC/vmaf/libvmaf/tools/meson.build"
  meson_build "$SRC/vmaf/libvmaf" -Dbuilt_in_models=true -Denable_tests=false -Denable_docs=false \
    -Denable_float=true
  # Bibliothèque C++ ajoutée à l'édition de liens (sed compatible BSD et GNU).
  local pc="$PREFIX/lib/pkgconfig/libvmaf.pc"
  if grep -q '^Libs.private:' "$pc"; then
    sed -i.bak "s/^Libs.private:/Libs.private: $CXXLIB/" "$pc"
  else
    echo "Libs.private: $CXXLIB" >>"$pc"
  fi
}
b_snappy() {
  cmake_build "$SRC/snappy" -DSNAPPY_BUILD_TESTS=OFF -DSNAPPY_BUILD_BENCHMARKS=OFF \
    -DSNAPPY_FUZZING_BUILD=OFF -DSNAPPY_REQUIRE_AVX=OFF -DSNAPPY_REQUIRE_AVX2=OFF
}
b_libwebp() {
  cmake_build "$SRC/libwebp" -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF \
    -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF -DWEBP_BUILD_WEBPINFO=OFF \
    -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF
}

FFMPEG_CONF=(
  --prefix="$PREFIX" --pkg-config-flags=--static
  --enable-gpl --enable-version3 --disable-nonfree
  --enable-static --disable-shared --disable-ffplay --disable-doc --disable-debug
  --enable-libx264 --enable-libx265 --enable-libvpx --enable-libsvtav1
  --enable-libopus --enable-libmp3lame --enable-libvorbis
  --enable-libfreetype --enable-libfribidi --enable-libharfbuzz --enable-libass
  --enable-libvmaf --enable-libsnappy --enable-libwebp
)
b_ffmpeg() {
  cd "$SRC/ffmpeg"
  local extra=()
  if [ "$OS" = "Darwin" ]; then
    # Sur le runner Intel, /usr/local (Homebrew) est dans les chemins par
    # défaut : FFmpeg y détecterait liblzma, X11, SDL2... et en dépendrait.
    # Aucune détection automatique ; seules les fonctions du système voulues.
    extra+=(--disable-autodetect --enable-zlib --enable-bzlib --enable-iconv
      --enable-videotoolbox --enable-audiotoolbox)
  fi
  ./configure "${FFMPEG_CONF[@]}" "${extra[@]}" \
    --extra-cflags="$CFLAGS" --extra-ldflags="$LDFLAGS" --extra-libs="$CXXLIB -lpthread -lm"
  make -j "$JOBS"
  make install
}

# Le FFmpeg produit ne doit dépendre que du système : aucune bibliothèque
# de Homebrew ou du dossier de compilation.
check_deps() {
  local bin="$1" bad
  if [ "$OS" = "Darwin" ]; then
    bad="$(otool -L "$bin" | tail -n +2 | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/Library/)' || true)"
  else
    bad="$(ldd "$bin" 2>/dev/null | grep -E "$PREFIX|/home/linuxbrew" || true)"
  fi
  [ -z "$bad" ] || fail "$(basename "$bin") dépend de : $bad"
}

case "$MODE" in
  sources)
    # Sources telles que publiées, sans aucun fichier de compilation :
    # dépôts git au commit épinglé (git archive), archive officielle de
    # FFmpeg, export Subversion de LAME.
    fetch_all
    for line in $GIT_SOURCES; do
      IFS='|' read -r name repo commit <<<"$line"
      log "archive $name"
      git -C "$SRC/$name" archive --format=tar --prefix="$name-$commit/" "$commit" | xz -T0 >"$OUT/$name-$commit.tar.xz"
    done
    log "archive lame"
    rm -rf "$WORK/lame-export"
    svn_get export "$WORK/lame-export/lame-r$LAME_REV"
    tar -C "$WORK/lame-export" -cJf "$OUT/lame-r$LAME_REV.tar.xz" "lame-r$LAME_REV"
    [ -f "$WORK/ffmpeg.tar.xz" ] || curl -fsSL --retry 4 -o "$WORK/ffmpeg.tar.xz" "$FFMPEG_URL"
    [ "$(sha256 "$WORK/ffmpeg.tar.xz")" = "$FFMPEG_SHA" ] || fail "ffmpeg-$FFMPEG_VERSION.tar.xz : empreinte incorrecte"
    cp "$WORK/ffmpeg.tar.xz" "$OUT/ffmpeg-$FFMPEG_VERSION.tar.xz"
    cp "$SCRIPT" "$OUT/"
    ;;
  build)
    fetch_all
    # FFBUILD_STEPS : étapes à refaire seulement (essais du script).
    for lib in ${FFBUILD_STEPS:-freetype fribidi harfbuzz libass x264 x265 libvpx svtav1 opus lame ogg vorbis vmaf snappy libwebp ffmpeg}; do
      step "$lib" "b_$lib"
    done
    for tool in ffmpeg ffprobe; do
      cp "$PREFIX/bin/$tool" "$OUT/$tool"
      check_deps "$OUT/$tool"
    done
    {
      echo "FFmpeg $FFMPEG_VERSION pour $OS $ARCH${MIN:+ (macOS $MIN et plus)}, compilé par VERIFLOW."
      echo "Script : scripts/build-ffmpeg-macos.sh ; sources : archive ffmpeg-sources de la Release."
      echo
      echo "ffmpeg-$FFMPEG_VERSION.tar.xz sha256 $FFMPEG_SHA"
      for line in $GIT_SOURCES; do echo "$line"; done
      echo "lame|$LAME_REPO|r$LAME_REV"
      echo
      "$OUT/ffmpeg" -hide_banner -buildconf
    } >"$OUT/BUILDINFO.txt"
    "$OUT/ffmpeg" -hide_banner -buildconf | grep -q -- --enable-nonfree && fail "--enable-nonfree présent"
    log "FFmpeg prêt : $OUT"
    ;;
  *)
    fail "mode inconnu : $MODE (build ou sources)"
    ;;
esac
