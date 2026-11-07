//! System icon-theme selection from freedesktop configuration.
use freedesktop_icons::default_theme_gtk;
use std::{
	env, fs,
	path::{Path, PathBuf},
};

pub(super) fn system_icon_theme() -> String {
	if let Ok(theme) = env::var("UMBRIEL_CONTROLS_ICON_THEME")
		&& !theme.trim().is_empty()
	{
		return theme.trim().to_owned();
	}

	let config = config_home();

	let gtk_theme = || {
		let root = config.as_ref()?;

		ini_value(
			&root.join("gtk-3.0/settings.ini"),
			"Settings",
			"gtk-icon-theme-name",
		)
		.or_else(|| {
			ini_value(
				&root.join("gtk-4.0/settings.ini"),
				"Settings",
				"gtk-icon-theme-name",
			)
		})
	};

	let kde_theme = || ini_value(&config.as_ref()?.join("kdeglobals"), "Icons", "Theme");

	let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();

	let is_desktop = |name: &str| {
		desktop
			.split(':')
			.any(|part| part.eq_ignore_ascii_case(name))
	};

	let theme = if is_desktop("KDE") {
		kde_theme().or_else(gtk_theme).or_else(default_theme_gtk)
	} else if is_desktop("GNOME") {
		default_theme_gtk().or_else(gtk_theme).or_else(kde_theme)
	} else {
		// Hyprland, Sway and other standalone compositors.
		gtk_theme().or_else(default_theme_gtk).or_else(kde_theme)
	};

	theme.unwrap_or_else(|| "hicolor".to_owned())
}

fn config_home() -> Option<PathBuf> {
	if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
		let path = PathBuf::from(path);
		if path.is_absolute() {
			return Some(path);
		}
	}

	env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
}

// Read one key from a simple INI-style configuration file.
fn ini_value(path: &Path, section: &str, key: &str) -> Option<String> {
	let contents = fs::read_to_string(path).ok()?;
	let mut in_section = false;

	for line in contents.lines() {
		let line = line.trim();

		if line.starts_with('[') && line.ends_with(']') {
			in_section = &line[1..line.len() - 1] == section;
			continue;
		}

		if !in_section || line.starts_with('#') || line.starts_with(';') {
			continue;
		}

		if let Some((name, value)) = line.split_once('=')
			&& name.trim() == key
		{
			let value = value.trim().trim_matches('"').trim_matches('\'');

			return (!value.is_empty()).then(|| value.to_owned());
		}
	}

	None
}
