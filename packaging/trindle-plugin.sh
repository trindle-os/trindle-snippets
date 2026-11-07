#!/bin/sh
# Builds Snippets as a Trindle plugin (interface 1): the program in release,
set -eu

top=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$top/dist/trindle-plugin}
id=$(sed -n 's/^Id=//p' "$top/plugin.ini")
version=$(sed -n 's/^Version=//p' "$top/plugin.ini")
cargo_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$top/Cargo.toml" | head -n 1)
if [ "$version" != "$cargo_version" ]; then
    echo "plugin.ini says $version but Cargo.toml says $cargo_version" >&2
    exit 1
fi

target=${CARGO_TARGET_DIR:-$top/target/shipped}
cargo_home=${CARGO_HOME:-$HOME/.cargo}
RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/home --remap-path-prefix=$(dirname "$top")=/src --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$target=/target"
export RUSTFLAGS
CARGO_TARGET_DIR=$target cargo +1.85 build --release --locked --manifest-path "$top/Cargo.toml"

mkdir -p "$out"
dir=$out/$id
rm -rf "$dir" "$dir.tar.zst" "$dir.description.txt" "$dir.svg"
mkdir -p "$dir/bin"
install -m 0755 "$target/release/snippets" "$dir/bin/snippets"
strip "$dir/bin/snippets" 2>/dev/null || true
mkdir -p "$dir/share/icons"
install -m 0644 "$top"/share/icons/*.svg "$dir/share/icons/"
mkdir -p "$dir/share/licences"
install -m 0644 "$top/share/licences/THIRD-PARTY.txt" "$dir/share/licences/"
install -m 0644 "$top/plugin.ini" "$top/LICENSE" "$top/packaging/trindle/snippets-symbolic.svg" "$dir/"
install -m 0644 "$top/packaging/trindle/description.txt" "$dir.description.txt"
install -m 0644 "$top/packaging/trindle/snippets.svg" "$dir.svg"

tar --sort=name --owner=0 --group=0 --numeric-owner --mtime=@0 \
    -C "$out" -cf - "$id" | zstd -q -19 -o "$dir.tar.zst"

trindle-plugin catalogue "$out"

echo "$id $version: $(du -sh "$dir" | cut -f1) as a folder, $(du -h "$dir.tar.zst" | cut -f1) to download"
