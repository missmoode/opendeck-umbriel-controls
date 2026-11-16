#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
plugin_id="$(cargo metadata --no-deps --format-version 1 |
  jq -er '.packages[] | select(.name == "umbriel-controls") | .metadata["umbriel-controls"].dev_plugin_id')"

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/opendeck"
plugin_path="$config_dir/plugins/$plugin_id"

# Don't interfere with Home Manager's ownership.
if [[ -f "$config_dir/externally-managed-plugins" ]] &&
  grep -Fxq -- "$plugin_id" "$config_dir/externally-managed-plugins"; then
  echo "Umbriel-Controls is managed by Home Manager; use the declarative workflow." >&2
  exit 1
fi

# Only remove a development link created using this plugin ID.
if [[ -L "$plugin_path" ]]; then
  source="$(readlink -- "$plugin_path")"
  if [[ "${source##*/}" != "$plugin_id" ]]; then
    echo "Refusing to remove unexpected symlink: $plugin_path -> $source" >&2
    exit 1
  fi
  unlink "$plugin_path"
  opendeck --reload-plugin "$plugin_id"
  echo "Unloaded $plugin_id"
elif [[ -e "$plugin_path" ]]; then
  echo "Refusing to remove non-symlink plugin: $plugin_path" >&2
  exit 1
else
  echo "No development plugin link to unload"
fi
