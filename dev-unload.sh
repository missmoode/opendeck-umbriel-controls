#!/usr/bin/env bash
plugin_id="$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "umbriel-controls") | .metadata["umbriel-controls"].plugin_id')"

config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/opendeck"
plugin_path="$config_dir/plugins/$plugin_id"

# Don't interfere with Home Manager's ownership.
if [[ -f "$config_dir/externally-managed-plugins" ]] &&
  grep -Fxq -- "$plugin_id" "$config_dir/externally-managed-plugins"; then
  echo "Umbriel-Controls is managed by Home Manager; use the declarative workflow." >&2
  exit 1
fi

# Remove only the symlink, not its target
if [[ -L "$plugin_path" ]]; then
  unlink "$plugin_path"
fi

# Notify the running OpenDeck instance.
opendeck --reload-plugin "$plugin_id"
