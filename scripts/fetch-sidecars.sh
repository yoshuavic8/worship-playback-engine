#!/usr/bin/env bash
# Fetch pinned uv + build LGPL static ffmpeg/ffprobe sidecars (not committed).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/apps/desktop/src-tauri/binaries"
MANIFEST="$ROOT/scripts/sidecars.manifest.json"
mkdir -p "$BIN"

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}';
  else sha256sum "$1" | awk '{print $1}'; fi
}

fetch_uv() {
  local triple="$1" url sha expect
  url="$(python3 - "$MANIFEST" "$triple" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
arch = sys.argv[2]
print(m["uv"]["archives"][arch]["url"])
PY
)"
  expect="$(python3 - "$MANIFEST" "$triple" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
print(m["uv"]["archives"][sys.argv[2]]["sha256"])
PY
)"
  local archive="$BIN/.cache/uv-$triple.tar.gz"
  mkdir -p "$BIN/.cache"
  echo "→ uv $triple"
  if [[ -f "$archive" ]] && [[ "$(sha256_file "$archive")" == "$expect" ]]; then
    echo "  using cached archive (sha256 ok)"
  else
    curl -fsSL "$url" -o "$archive"
  fi
  local got
  got="$(sha256_file "$archive")"
  if [[ "$got" != "$expect" ]]; then
    echo "uv checksum mismatch for $triple (got $got)" >&2
    exit 1
  fi
  tar -xzf "$archive" -C "$BIN/.cache"
  local extracted="$BIN/.cache/uv"
  [[ -f "$extracted" ]] || extracted="$BIN/.cache/uv-$triple/uv"
  install -m 755 "$extracted" "$BIN/uv-$triple"
}

fetch_ffmpeg() {
  local triple="$1"
  if [[ "${ALLOW_SYSTEM_FFMPEG_COPY:-0}" == "1" ]]; then
    echo "WARNING: ALLOW_SYSTEM_FFMPEG_COPY=1 — not for release builds" >&2
    if command -v ffmpeg >/dev/null && command -v ffprobe >/dev/null; then
      install -m 755 "$(command -v ffmpeg)" "$BIN/ffmpeg-$triple"
      install -m 755 "$(command -v ffprobe)" "$BIN/ffprobe-$triple"
      return
    fi
  fi
  echo "→ ffmpeg $triple (static LGPL build from ffmpeg.org source)"
  "$ROOT/scripts/build-ffmpeg-lgpl.sh" "$triple"
}

lipo_universal() {
  local name="$1"
  local arm="$BIN/${name}-aarch64-apple-darwin"
  local x86="$BIN/${name}-x86_64-apple-darwin"
  local out="$BIN/${name}-universal-apple-darwin"
  if [[ ! -f "$arm" || ! -f "$x86" ]]; then
    echo "missing arch sidecar for $name (need $arm and $x86)" >&2
    exit 1
  fi
  echo "→ lipo universal $name"
  lipo -create "$arm" "$x86" -output "$out"
  chmod 755 "$out"
  lipo -info "$out"
}

echo "Fetching sidecars into $BIN"
fetch_uv "aarch64-apple-darwin"
fetch_uv "x86_64-apple-darwin"
fetch_ffmpeg "aarch64-apple-darwin"
fetch_ffmpeg "x86_64-apple-darwin"

for tool in uv ffmpeg ffprobe; do
  lipo_universal "$tool"
done

echo "Done. Sidecars:"
ls -la "$BIN"/uv-* "$BIN"/ffmpeg-* "$BIN"/ffprobe-* 2>/dev/null || true
