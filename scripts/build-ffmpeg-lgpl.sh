#!/usr/bin/env bash
# Build static LGPL ffmpeg/ffprobe for macOS sidecars (no GPL/nonfree, no Homebrew dylibs).
set -euo pipefail
TRIPLE="${1:-aarch64-apple-darwin}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/apps/desktop/src-tauri/binaries"
MANIFEST="$ROOT/scripts/sidecars.manifest.json"
VERSION="$(python3 - "$MANIFEST" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["ffmpeg"]["version"])
PY
)"
TARBALL_SHA="$(python3 - "$MANIFEST" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))["ffmpeg"]["sourceTarballSha256"])
PY
)"
BIN_CACHE="$BIN/.cache/ffmpeg-$VERSION"
SRC_TAR="$BIN_CACHE/ffmpeg-$VERSION.tar.xz"
SRC_DIR="$BIN_CACHE/ffmpeg-$VERSION"
# ffmpeg configure embeds --prefix unquoted in shell snippets; avoid spaces in prefix.
BUILD_ROOT="${WPE_FFMPEG_BUILD_ROOT:-$HOME/.cache/wpe-ffmpeg-build}/$VERSION"
OUT="$BUILD_ROOT/out-$TRIPLE"
mkdir -p "$BIN_CACHE" "$BUILD_ROOT" "$BIN"

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}';
  else sha256sum "$1" | awk '{print $1}'; fi
}

if [[ ! -f "$SRC_TAR" ]]; then
  echo "Downloading ffmpeg-$VERSION source…"
  curl -fsSL "https://ffmpeg.org/releases/ffmpeg-${VERSION}.tar.xz" -o "$SRC_TAR"
fi
got="$(sha256_file "$SRC_TAR")"
if [[ "$got" != "$TARBALL_SHA" ]]; then
  echo "ffmpeg source tarball checksum mismatch (expected $TARBALL_SHA, got $got)" >&2
  exit 1
fi

if [[ ! -d "$SRC_DIR" ]]; then
  tar -xf "$SRC_TAR" -C "$BIN_CACHE"
fi

# Run command native or under Rosetta for x86_64 on Apple Silicon hosts.
run_for_triple() {
  if [[ "$TRIPLE" == "x86_64-apple-darwin" && "$(uname -m)" == "arm64" ]]; then
    arch -x86_64 "$@"
  else
    "$@"
  fi
}

verify_static_macos() {
  local exe="$1"
  local bad=0
  while IFS= read -r line; do
    dep="$(echo "$line" | awk '{print $1}')"
    [[ -z "$dep" || "$dep" == *":"* ]] && continue
    case "$dep" in
      /usr/lib/*|/System/*|/Library/Developer/*) ;;
      @*) ;;
      *)
        echo "ERROR: $exe links external library: $dep" >&2
        bad=1
        ;;
    esac
  done < <(otool -L "$exe" | tail -n +2)
  return "$bad"
}

if [[ -f "$BIN/ffmpeg-$TRIPLE" && -f "$BIN/ffprobe-$TRIPLE" ]]; then
  if verify_static_macos "$BIN/ffmpeg-$TRIPLE" && verify_static_macos "$BIN/ffprobe-$TRIPLE"; then
    echo "ffmpeg $TRIPLE already built and verified"
    exit 0
  fi
  echo "Existing ffmpeg $TRIPLE failed otool check — rebuilding" >&2
  rm -f "$BIN/ffmpeg-$TRIPLE" "$BIN/ffprobe-$TRIPLE"
fi

rm -rf "$OUT"
mkdir -p "$OUT"

cd "$SRC_DIR"
make distclean 2>/dev/null || true

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"
export CFLAGS="${CFLAGS:-} -O2"
export LDFLAGS="${LDFLAGS:-}"

extra_configure=()
if [[ "$TRIPLE" == x86_64-* ]] && ! command -v nasm >/dev/null 2>&1 && ! command -v yasm >/dev/null 2>&1; then
  extra_configure+=(--disable-x86asm)
fi

run_for_triple ./configure \
  --prefix="$OUT" \
  --disable-gpl \
  --disable-nonfree \
  --disable-debug \
  --enable-static \
  --disable-shared \
  --disable-doc \
  --disable-network \
  --disable-autodetect \
  --disable-everything \
  --enable-protocol=file \
  --enable-demuxer=wav,mp3,mov,mp4,m4a,aiff,flac,mpegps,mpegts,ogg \
  --enable-decoder=pcm_s16le,pcm_s24le,pcm_f32le,mp3,aac,flac,vorbis,opus \
  --enable-muxer=wav,mp4 \
  --enable-encoder=pcm_s16le \
  --enable-filter=aresample \
  --enable-zlib \
  "${extra_configure[@]+"${extra_configure[@]}"}"

run_for_triple make -j"$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
run_for_triple make install

install -m 755 "$OUT/bin/ffmpeg" "$BIN/ffmpeg-$TRIPLE"
install -m 755 "$OUT/bin/ffprobe" "$BIN/ffprobe-$TRIPLE"

verify_static_macos "$BIN/ffmpeg-$TRIPLE"
verify_static_macos "$BIN/ffprobe-$TRIPLE"
echo "Installed verified static ffmpeg/ffprobe for $TRIPLE"
