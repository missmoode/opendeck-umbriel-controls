use crate::icons::{palette::Palette, svg};

const SCROLLING: &str = include_str!("scrolling.svg");
const DWINDLE: &str = include_str!("dwindle.svg");
const MASTER: &str = include_str!("master.svg");
const STACKING: &str = include_str!("stacking.svg");
const UNKNOWN: &str = include_str!("unknown.svg");

pub fn render(layout: &str, palette: &Palette) -> String {
	let template = match layout {
		"scrolling" => SCROLLING,
		"dwindle" => DWINDLE,
		"master" => MASTER,
		"stacking" => STACKING,
		_ => UNKNOWN,
	};
	let artwork = template
		.replace("{{MUTED_FILL}}", &palette.muted.fill)
		.replace("{{SECONDARY_FILL}}", &palette.secondary.fill)
		.replace("{{ACCENT_FILL}}", &palette.accent.fill)
		.replace("{{CHROME_BORDER}}", &palette.chrome.border)
		.replace("{{CHROME_TEXT}}", &palette.chrome.text);
	svg::data_url(&artwork)
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
	fn layout_icon_has_no_embedded_label() {
		let palette = builtin_palette("blue").unwrap();
		let svg = decode(&render("dwindle", &palette));
		assert!(!svg.contains("Dwindle"));
		assert!(!svg.contains("{{"));
	}
}
