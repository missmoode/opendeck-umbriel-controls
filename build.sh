#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

case "${1:---bundle}" in
--bundle | --archive) mode="${1:---bundle}" ;;
*)
  echo "Usage: $0 [--bundle|--archive]" >&2
  exit 2
  ;;
esac

# One binary crate. This information comes from Cargo.toml.
metadata="$(cargo metadata --no-deps --format-version 1)"
binary="$(jq -er '.packages[] | select(.name == "umbriel-controls") |.name' <<<"$metadata")"
version="$(jq -er '.packages[] | select(.name == "umbriel-controls") |.version' <<<"$metadata")"
plugin_id="$(jq -er '.packages[] | select(.name == "umbriel-controls") | .metadata["umbriel-controls"].plugin_id' <<<"$metadata")"
host="$(rustc -vV | sed -n 's/^host: //p')"

case "$host" in
x86_64-unknown-linux-gnu) archive_arch=x86_64 ;;
aarch64-unknown-linux-gnu) archive_arch=aarch64 ;;
*)
  echo "Unsupported native target: $host" >&2
  exit 1
  ;;
esac

# The source manifest is kept in sync with Cargo by Release Please.
manifest_version="$(jq -er '.Version' assets/manifest.json)"
manifest_binary="$(jq -er --arg target "$host" '.CodePaths[$target]' assets/manifest.json)"
if [[ "$manifest_version" != "$version" ]]; then
  echo "Version mismatch: Cargo.toml=$version, assets/manifest.json=$manifest_version" >&2
  exit 1
fi
if [[ "$manifest_binary" != "$binary-$host" ]]; then
  echo "Manifest CodePaths[$host]=$manifest_binary, expected $binary-$host" >&2
  exit 1
fi

cargo build --release --locked --target "$host"

bundle="dist/${plugin_id}"
rm -rf "$bundle"
mkdir -p "$bundle"
cp -a assets/. "$bundle/"
# Include notices for both the GPL plugin and its MIT-licensed IPC crate.
install -Dm644 LICENSE "$bundle/LICENSE"
install -Dm644 crates/umbriel-ipc/LICENSE "$bundle/crates/umbriel-ipc/LICENSE"
install -m755 "target/$host/release/$binary" "$bundle/$binary-$host"
printf 'Bundle: %s\n' "$bundle"

if [[ "$mode" == --archive ]]; then
  archive="${binary}-${version}-${archive_arch}-linux.streamDeckPlugin"
  (
    cd dist
    rm -f "$archive"
    zip -qr "$archive" "${plugin_id}"
  )
  printf 'Archive: dist/%s\n' "$archive"
fi
