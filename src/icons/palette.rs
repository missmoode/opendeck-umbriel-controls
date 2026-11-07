//! Palette colours and compile-time bundled themes.
use std::{
	collections::BTreeMap,
	sync::{Arc, LazyLock},
};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IconColours {
	pub fill: String,
	pub stroke: String,
	pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChromeColours {
	pub surface: String,
	pub text: String,
	pub border: String,
	pub selection: String,
	pub indicator: String,
	pub indicator_border: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Palette {
	pub muted: IconColours,
	pub secondary: IconColours,
	pub accent: IconColours,
	pub chrome: ChromeColours,
}

pub struct BuiltinTheme {
	pub id: &'static str,
	pub label: &'static str,
	json: &'static str,
}

pub const BUILTIN_THEMES: &[BuiltinTheme] = &[
	BuiltinTheme {
		id: "blue",
		label: "Blue",
		json: include_str!("palettes/blue.json"),
	},
	BuiltinTheme {
		id: "gold",
		label: "Gold",
		json: include_str!("palettes/gold.json"),
	},
	BuiltinTheme {
		id: "purple",
		label: "Purple",
		json: include_str!("palettes/purple.json"),
	},
	BuiltinTheme {
		id: "pink",
		label: "Pink",
		json: include_str!("palettes/pink.json"),
	},
	BuiltinTheme {
		id: "red",
		label: "Red",
		json: include_str!("palettes/red.json"),
	},
	BuiltinTheme {
		id: "green",
		label: "Green",
		json: include_str!("palettes/green.json"),
	},
	BuiltinTheme {
		id: "hidden",
		label: "Hidden",
		json: include_str!("palettes/hidden.json"),
	},
];

fn valid_hex(value: &str) -> bool {
	matches!(value.len(), 7 | 9)
		&& value.starts_with('#')
		&& value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn validate(palette: &Palette) -> bool {
	let colours = [
		&palette.muted.fill,
		&palette.muted.stroke,
		&palette.muted.text,
		&palette.secondary.fill,
		&palette.secondary.stroke,
		&palette.secondary.text,
		&palette.accent.fill,
		&palette.accent.stroke,
		&palette.accent.text,
		&palette.chrome.surface,
		&palette.chrome.text,
		&palette.chrome.border,
		&palette.chrome.selection,
		&palette.chrome.indicator,
		&palette.chrome.indicator_border,
	];
	colours.into_iter().all(|colour| valid_hex(colour))
}

pub fn parse_palette(bytes: &[u8]) -> Result<Palette, String> {
	let palette: Palette = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
	if !validate(&palette) {
		return Err("colours must be #RRGGBB or #RRGGBBAA".into());
	}
	Ok(palette)
}

static BUILTINS: LazyLock<BTreeMap<&'static str, Arc<Palette>>> = LazyLock::new(|| {
	BUILTIN_THEMES
		.iter()
		.map(|theme| {
			let palette = parse_palette(theme.json.as_bytes())
				.unwrap_or_else(|error| panic!("Invalid bundled theme {}: {error}", theme.id));
			(theme.id, Arc::new(palette))
		})
		.collect()
});

pub fn builtin_palette(id: &str) -> Option<Arc<Palette>> {
	BUILTINS.get(id).cloned()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_builtin_palette_parses() {
		for theme in BUILTIN_THEMES {
			assert!(builtin_palette(theme.id).is_some());
		}
	}

	#[test]
	fn rejects_svg_injection_in_colours() {
		let mut theme: serde_json::Value = serde_json::from_str(BUILTIN_THEMES[0].json).unwrap();
		theme["chrome"]["border"] = r##"#ABCDEF" onload="alert(1)"##.into();
		assert!(parse_palette(&serde_json::to_vec(&theme).unwrap()).is_err());
	}
}
