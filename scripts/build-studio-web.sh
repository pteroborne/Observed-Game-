#!/usr/bin/env bash
# Build the composition studio as a browser bundle.
#
# The bash counterpart to build-tactics-web.ps1. CI runs the PowerShell one on
# a Windows-flavoured shell; this exists so the same bundle can be produced on
# a Linux workstation, where pwsh is usually absent.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_root/web-dist/composition-studio}"

# A distribution directory is one deployable build; refuse to scribble outside
# web-dist even if a caller passes something odd.
expected_root="$repo_root/web-dist"
case "$(realpath -m "$output_dir")" in
  "$expected_root"/*) ;;
  *) echo "output directory must stay inside $expected_root" >&2; exit 1 ;;
esac

# cargo-installed tools land in ~/.cargo/bin, which is not on PATH when cargo
# itself came from a distro package.
bindgen="$(command -v wasm-bindgen || true)"
if [[ -z "$bindgen" && -x "$HOME/.cargo/bin/wasm-bindgen" ]]; then
  bindgen="$HOME/.cargo/bin/wasm-bindgen"
fi
if [[ -z "$bindgen" ]]; then
  echo "wasm-bindgen is required. Install the lockfile-matched CLI with:" >&2
  echo "  cargo install wasm-bindgen-cli --version 0.2.125 --locked" >&2
  exit 1
fi

cd "$repo_root"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

cargo build -p composition_studio --bin composition-studio \
  --profile wasm-release --target wasm32-unknown-unknown

profile_dir="$target_dir/wasm32-unknown-unknown/wasm-release"
wasm="$(find "$profile_dir" -maxdepth 1 -name 'composition?studio.wasm' -print -quit)"
if [[ -z "$wasm" ]]; then
  echo "cargo completed but no composition-studio wasm was produced in $profile_dir" >&2
  exit 1
fi

rm -rf "$output_dir"
mkdir -p "$output_dir"
"$bindgen" --target web --no-typescript \
  --out-dir "$output_dir" --out-name composition_studio "$wasm"

# Content-address the bundle so a browser never serves a stale build from cache.
bundle="$output_dir/composition_studio_bg.wasm"
build_hash="$(sha256sum "$bundle" | cut -c1-16)"
mv "$bundle" "$output_dir/composition_studio_bg.$build_hash.wasm"

sed "s/__STUDIO_BUILD__/$build_hash/g" \
  "$repo_root/tools/composition_studio/web/index.html" > "$output_dir/index.html"

size="$(du -h "$output_dir/composition_studio_bg.$build_hash.wasm" | cut -f1)"
echo "Studio web bundle: $size"
echo "Build cache key:   $build_hash"
echo "Output:            $output_dir"
