#!/usr/bin/env bash
# A renderer-free WASM simulation and accessible DOM/SVG interface.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="$repo_root/web-dist/architect-lab"
cd "$repo_root"
bindgen="$(command -v wasm-bindgen || true)"
if [[ -z "$bindgen" ]]; then
  echo "Install wasm-bindgen-cli 0.2.125 (matching Cargo.lock)." >&2
  exit 1
fi
cargo build -p architect_lab --lib --no-default-features --features web \
  --profile wasm-release --target wasm32-unknown-unknown
# Ask Cargo where its configured output lives instead of assuming target/.
target_dir="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')"
mkdir -p "$output_dir"
"$bindgen" --target web --no-typescript --out-dir "$output_dir" --out-name architect_lab \
  "$target_dir/wasm32-unknown-unknown/wasm-release/architect_lab.wasm"
cp labs/architect_lab/web/*.js labs/architect_lab/web/style.css "$output_dir/"
mkdir -p "$output_dir/art"
cp labs/architect_lab/web/art/*.webp "$output_dir/art/"
build_hash="$(cat "$output_dir/architect_lab_bg.wasm" "$output_dir/architect_lab.js" "$output_dir"/*.js "$output_dir/style.css" "$output_dir"/art/*.webp | sha256sum | cut -c1-16)"
# Version the entire module graph, including JS-to-WASM imports.
sed -i "s/__BUILD__/$build_hash/g" "$output_dir"/*.js
sed "s/__BUILD__/$build_hash/g" labs/architect_lab/web/index.html > "$output_dir/index.html"
gzip -kf "$output_dir/architect_lab_bg.wasm"
echo "Browser bundle: $output_dir"
du -h "$output_dir/architect_lab_bg.wasm" "$output_dir/architect_lab_bg.wasm.gz"
