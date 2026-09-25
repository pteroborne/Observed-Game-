#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_root/web-dist/architect-hand-lab}"
expected_root="$repo_root/web-dist"
case "$(realpath -m "$output_dir")" in
  "$expected_root"/*) ;;
  *) echo "output directory must stay inside $expected_root" >&2; exit 1 ;;
esac

bindgen="$(command -v wasm-bindgen || true)"
if [[ -z "$bindgen" && -x "$HOME/.cargo/bin/wasm-bindgen" ]]; then
  bindgen="$HOME/.cargo/bin/wasm-bindgen"
fi
if [[ -z "$bindgen" ]]; then
  echo "wasm-bindgen 0.2.125 is required" >&2
  exit 1
fi

cd "$repo_root"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
cargo build -p architect_hand_lab --bin architect_hand_lab \
  --profile wasm-release --target wasm32-unknown-unknown

profile_dir="$target_dir/wasm32-unknown-unknown/wasm-release"
wasm="$(find "$profile_dir" -maxdepth 1 -name 'architect?hand?lab.wasm' -print -quit)"
if [[ -z "$wasm" ]]; then
  echo "cargo completed but no architect_hand_lab wasm was produced" >&2
  exit 1
fi

mkdir -p "$expected_root"
staging="$(mktemp -d "$expected_root/.architect-hand.XXXXXX")"
cleanup() { rm -rf "$staging"; }
trap cleanup EXIT

"$bindgen" --target web --no-typescript \
  --out-dir "$staging" --out-name architect_hand_lab "$wasm"
bundle="$staging/architect_hand_lab_bg.wasm"
build_hash="$(sha256sum "$bundle" | cut -c1-16)"
mv "$bundle" "$staging/architect_hand_lab_bg.$build_hash.wasm"
sed "s/__ARCHITECT_HAND_BUILD__/$build_hash/g" \
  "$repo_root/labs/architect_hand_lab/web/index.html" > "$staging/index.html"

previous="$output_dir.previous"
rm -rf "$previous"
if [[ -d "$output_dir" ]]; then mv "$output_dir" "$previous"; fi
mv "$staging" "$output_dir"
rm -rf "$previous"
trap - EXIT

size="$(du -h "$output_dir/architect_hand_lab_bg.$build_hash.wasm" | cut -f1)"
echo "Architect Hand Lab web bundle: $size"
echo "Build cache key:                $build_hash"
echo "Output:                         $output_dir"
