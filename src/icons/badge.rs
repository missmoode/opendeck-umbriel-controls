//! Palette-aware workspace-switcher badges.
use super::{palette::Palette, svg};

pub fn shorten(label: &str) -> String {
	let mut chars = label.trim().chars();
	let mut short: String = chars.by_ref().take(8).collect();
	if chars.next().is_some() {
		short.pop();
		short.push('…');
	}
	if short.is_empty() {
		"?".to_owned()
	} else {
		short
	}
}

/// Upper-right badge in the same coordinate system as the action's SVG.
pub fn badge(label: &str, palette: &Palette, canvas: u16) -> String {
	let label = shorten(label);
	let width = (24 + label.chars().count() as u16 * 13).clamp(50, canvas.saturating_sub(16));
	let x = canvas - 8 - width;
	let middle = x + width / 2;
	format!(
		"<rect x=\"{x}\" y=\"4\" width=\"{width}\" height=\"38\" \
         rx=\"16\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\
         <text x=\"{middle}\" y=\"30\" text-anchor=\"middle\" \
         font-family=\"sans-serif\" font-size=\"20\" font-weight=\"700\" \
         fill=\"{}\">{}</text>",
		palette.chrome.surface,
		palette.chrome.border,
		palette.chrome.text,
		svg::escape_text(&label)
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::icons::palette::builtin_palette;
	#[test]
	fn truncates_and_escapes_labels() {
		assert_eq!(shorten("LONG-NAME"), "LONG-NA…");
		let svg = badge("A&B <work>", &builtin_palette("hidden").unwrap(), 144);
		assert!(svg.contains("A&amp;B"));
		assert!(!svg.contains("<work>"));
		assert!(svg.contains("width=\"128\""));
	}
}
