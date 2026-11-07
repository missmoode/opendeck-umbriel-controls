plugin_id="$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "umbriel-controls") | .metadata["umbriel-controls"].plugin_id')"

plugin_path="${XDG_CONFIG_HOME:-$HOME/.config}/opendeck/plugins/$plugin_id"

# Remove only the symlink, not its target
if [[ -L "$plugin_path" ]]; then
  unlink "$plugin_path"
fi

# Notify the running OpenDeck instance.
opendeck --reload-plugin "$plugin_id"
