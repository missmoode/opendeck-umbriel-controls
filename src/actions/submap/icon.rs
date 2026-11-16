//! Shared keyboard glyph: a destination for Set, a dashed outline for Reset.
use crate::icons::{palette::Palette, svg};

const TEMPLATE: &str = include_str!("keyboard.svg");

pub(super) fn render(reset: bool, active: bool, palette: &Palette) -> String {
	let background = if active && !reset {
		format!(
			r#"<rect x="2" y="2" width="140" height="140" rx="17" fill="{}" stroke="{}" stroke-width="3"/>"#,
			palette.muted.fill, palette.muted.stroke
		)
	} else {
		String::new()
	};

	let (keyboard, opacity) = if reset {
		// Remains a dashed, unfilled keyboard in both states. Colour alone
		// indicates whether there is anything to reset.
		let colour = if active {
			&palette.secondary.fill
		} else {
			&palette.muted.fill
		};
		(
			format!(
				r##"<rect x="13" y="39" width="118" height="66" rx="13" fill="none" stroke="{}" stroke-width="5" stroke-dasharray="6 9" stroke-linecap="round"/>
<use href="#keyboard-keys" fill="{}"/>"##,
				colour, colour
			),
			if active { "1" } else { "0.75" },
		)
	} else {
		(
			format!(
				r##"<rect x="13" y="39" width="118" height="66" rx="13" fill="{}" stroke="{}" stroke-width="5"/>
<use href="#keyboard-keys" fill="{}"/>"##,
				palette.secondary.fill, palette.secondary.stroke, palette.accent.stroke
			),
			"1",
		)
	};

	let markup = TEMPLATE
		.replace("{{BACKGROUND}}", &background)
		.replace("{{KEYBOARD_OPACITY}}", opacity)
		.replace("{{KEYBOARD}}", &keyboard);

	svg::data_url(&markup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::icons::palette::builtin_palette;
	use base64::{Engine as _, engine::general_purpose::STANDARD};

	fn decode(url: &str) -> String {
		String::from_utf8(
			STANDARD
				.decode(url.strip_prefix("data:image/svg+xml;base64,").unwrap())
				.unwrap(),
		)
		.unwrap()
	}

	#[test]
	fn set_has_selected_background_only_in_active_state() {
		let palette = builtin_palette("purple").unwrap();
		let inactive = decode(&render(false, false, &palette));
		let active = decode(&render(false, true, &palette));
		assert_eq!(inactive.matches("rx=\"4\"").count(), 7);
		assert!(active.contains(&format!(
			"fill=\"{}\" stroke=\"{}\" stroke-width=\"3\"",
			palette.muted.fill, palette.muted.stroke
		)));
		assert!(!inactive.contains("<rect x=\"2\" y=\"2\""));
		assert!(!inactive.contains("stroke-dasharray"));
	}

	#[test]
	fn reset_is_always_dashed_and_unfilled_with_distinct_colours() {
		let palette = builtin_palette("purple").unwrap();
		let unavailable = decode(&render(true, false, &palette));
		let available = decode(&render(true, true, &palette));
		for markup in [&unavailable, &available] {
			assert!(markup.contains("stroke-dasharray=\"6 9\""));
			assert!(markup.contains("fill=\"none\""));
			assert!(!markup.contains("<rect x=\"2\" y=\"2\""));
			assert!(!markup.contains("<mask"));
		}
		assert!(unavailable.contains(&format!("stroke=\"{}\"", palette.muted.fill)));
		assert!(available.contains(&format!("stroke=\"{}\"", palette.secondary.fill)));
		assert!(unavailable.contains("opacity=\"0.75\""));
		assert!(available.contains("opacity=\"1\""));
	}

	#[test]
	fn hidden_palette_has_no_opaque_keyboard_fill() {
		let palette = builtin_palette("hidden").unwrap();
		for reset in [false, true] {
			for active in [false, true] {
				let svg = decode(&render(reset, active, &palette));
				assert!(!svg.contains(&palette.chrome.surface));
				assert!(svg.contains(&palette.secondary.fill) || svg.contains(&palette.muted.fill));
			}
		}
	}
}
