#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

plugin_id="$(cargo metadata --no-deps --format-version 1 |
  jq -er '.packages[] | select(.name == "umbriel-controls") | .metadata["umbriel-controls"].plugin_id')"

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/opendeck"
plugin_dir="$config_dir/plugins"
target="$plugin_dir/$plugin_id"

# Don't interfere with Home Manager's ownership.
if [[ -f "$config_dir/externally-managed-plugins" ]] &&
  grep -Fxq -- "$plugin_id" "$config_dir/externally-managed-plugins"; then
  echo "Umbriel-Controls is managed by Home Manager; use the declarative workflow." >&2
  exit 1
fi

nix build .#default -L

source="$(realpath "result/$plugin_id")"

mkdir -p "$plugin_dir"

# Don't overwrite a plugin installed through the GUI.
if [[ -e "$target" && ! -L "$target" ]]; then
  echo "Refusing to replace existing plugin: $target" >&2
  exit 1
fi

ln -sfnT "$source" "$target"

# Notify the running OpenDeck instance.
opendeck --reload-plugin "$plugin_id"

echo "Reloaded $plugin_id"
