#!/usr/bin/env bash
# Build the spectator viewer to wasm and stage it in dist/viewer.
# Creates dist/viewer/READY only when every step exits 0.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
out="$root/dist/viewer"
rm -f "$out/READY"
cargo build -p tankwar-viewer --release --target wasm32-unknown-unknown --target-dir target/viewer-wasm
mkdir -p "$out"
stage="target/viewer-wasm/stage"
rm -rf "$stage"
mkdir -p "$stage"
wasm-bindgen --target web --no-typescript --out-dir "$stage" --out-name tankwar-viewer \
  target/viewer-wasm/wasm32-unknown-unknown/release/tankwar-viewer.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
    -o "$stage/opt.wasm" "$stage/tankwar-viewer_bg.wasm"
  mv "$stage/opt.wasm" "$stage/tankwar-viewer_bg.wasm"
fi
hash=$(shasum -a 256 "$stage/tankwar-viewer_bg.wasm" | awk '{print $1}')
perl -i -pe "s#new URL\\('tankwar-viewer_bg\\.wasm'#new URL('tankwar-viewer_bg.wasm?v=${hash}'#" "$stage/tankwar-viewer.js"
cp "$stage/tankwar-viewer.js" "$stage/tankwar-viewer_bg.wasm" "$out/"
printf '%s\n' "$hash" > "$out/wasm.version"
cp crates/viewer/web/index.html "$out/index.html"
rm -rf "$out/assets"
mkdir -p "$out/assets"
cp -R assets/derived/. "$out/assets/"
# READY means the js and its wasm sit side by side and the wasm is valid.
test -s "$out/tankwar-viewer.js"
test "$(head -c 4 "$out/tankwar-viewer_bg.wasm" | od -An -tx1 | tr -d ' ')" = "0061736d"
touch "$out/READY"
echo "dist/viewer/READY"
