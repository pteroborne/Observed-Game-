#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "usage: $0 VERSION [OUTPUT_DIRECTORY]" >&2
    exit 2
fi

version="$1"
if [[ ! "$version" =~ ^[0-9A-Za-z._-]+$ ]]; then
    echo "VERSION may contain only letters, digits, dots, underscores, and hyphens" >&2
    exit 2
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd -- "$script_dir/.." && pwd)"
output_dir="${2:-$repository_root/dist}"
target_root="${CARGO_TARGET_DIR:-$repository_root/target}"
if [[ "$target_root" != /* ]]; then
    target_root="$repository_root/$target_root"
fi

binary="$target_root/release/observed"
if [[ ! -f "$binary" ]]; then
    echo "release binary not found: $binary" >&2
    echo "build it with: cargo build --locked --release -p observed_game" >&2
    exit 1
fi
if [[ ! -d "$repository_root/assets/tiles" ]]; then
    echo "runtime assets not found: $repository_root/assets" >&2
    exit 1
fi

package_name="observed-${version}-steam-deck-linux-x86_64"
staging_root="$(mktemp -d)"
trap 'rm -rf -- "$staging_root"' EXIT
package_root="$staging_root/$package_name"

mkdir -p -- "$package_root" "$output_dir"
install -m 0755 -- "$binary" "$package_root/observed"
cp -a -- "$repository_root/assets" "$package_root/assets"
install -m 0644 -- "$repository_root/docs/steam_deck_release_readme.txt" \
    "$package_root/README.txt"

if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$package_root/observed"
fi

archive_name="$package_name.tar.gz"
tar -C "$staging_root" -czf "$output_dir/$archive_name" "$package_name"
(
    cd -- "$output_dir"
    sha256sum "$archive_name" > "$archive_name.sha256"
)

echo "created $output_dir/$archive_name"
echo "created $output_dir/$archive_name.sha256"
