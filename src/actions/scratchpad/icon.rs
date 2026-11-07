use crate::icons::{palette::Palette, svg};

const TEMPLATE: &str = include_str!("scratchpad.svg");

/// Slots are ordered back → middle → front; missing URLs leave empty panels.
pub fn render(palette: &Palette, icons: &[Option<String>]) -> String {
	let mut slots = [String::new(), String::new(), String::new()];

	// With 1 window, decorate the front panel; with 2, middle + front.
	let count = icons.len().min(3);
	for (index, icon) in icons.iter().take(3).enumerate() {
		let slot = 3 - count + index;
		if let Some(url) = icon {
			let (x, y, size) = match slot {
				0 => (29, 31, 28), // back
				1 => (72, 35, 28), // middle
				_ => (39, 70, 36), // front
			};
			// `url` is generated internally by bitmap::png_data_url.
			slots[slot] = format!(
				"<image x=\"{x}\" y=\"{y}\" width=\"{size}\" \
                 height=\"{size}\" href=\"{url}\"/>"
			);
		}
	}

	let svg_text = TEMPLATE
		.replace("{{BACK_FILL}}", &palette.muted.fill)
		.replace("{{BACK_STROKE}}", &palette.muted.stroke)
		.replace("{{MIDDLE_FILL}}", &palette.secondary.fill)
		.replace("{{MIDDLE_STROKE}}", &palette.secondary.stroke)
		.replace("{{FRONT_FILL}}", &palette.accent.fill)
		.replace("{{FRONT_STROKE}}", &palette.accent.stroke) // 🏊
		.replace("{{BACK_ICON}}", &slots[0])
		.replace("{{MIDDLE_ICON}}", &slots[1])
		.replace("{{FRONT_ICON}}", &slots[2]);
	svg::data_url(&svg_text)
}
