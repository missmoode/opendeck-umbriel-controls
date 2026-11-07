use crate::icons::{badge, palette::Palette, panels, svg};

const TEMPLATE: &str = include_str!("switch.svg");

pub fn render(
	label: &str,
	active: bool,
	show_badge: bool,
	palette: &Palette,
	icons: &[Option<String>],
) -> String {
	// Only PNG data URLs produced by our resolver are embedded.
	// None or missing files do not take up a panel.
	let visible: Vec<&str> = icons
		.iter()
		.filter_map(Option::as_deref)
		.filter(|url| url.starts_with("data:image/png;base64,"))
		.take(4)
		.collect();

	let background = if active {
		format!(
			r#"<rect x="2" y="2"
                  width="140" height="140"
                  rx="17"
                  fill="{}"
                  stroke="{}"
                  stroke-width="3"/>"#,
			palette.muted.fill, palette.muted.stroke,
		)
	} else {
		String::new()
	};

	let badge = if show_badge {
		badge::badge(label, palette, 144)
	} else {
		String::new()
	};

	let markup = TEMPLATE
		.replace("{{BACKGROUND}}", &background)
		.replace("{{PANELS}}", &panels::markup(&visible, palette))
		.replace("{{BADGE}}", &badge);

	svg::data_url(&markup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::icons::palette::builtin_palette;
	use base64::{Engine as _, engine::general_purpose::STANDARD};

	fn svg_text(url: &str) -> String {
		let data = url.strip_prefix("data:image/svg+xml;base64,").unwrap();
		String::from_utf8(STANDARD.decode(data).unwrap()).unwrap()
	}

	fn png_url() -> String {
		"data:image/png;base64,AAAA".to_owned()
	}

	#[test]
	fn renders_exactly_zero_to_four_app_images() {
		for count in 0..=4 {
			let images = (0..count).map(|_| Some(png_url())).collect::<Vec<_>>();
			let output = svg_text(&render(
				"WEB",
				true,
				true,
				&builtin_palette("blue").unwrap(),
				&images,
			));
			assert_eq!(output.matches("<image ").count(), count);
			assert!(!output.contains("{{"));
		}
	}

	#[test]
	fn skips_missing_icons_before_selecting_layout() {
		let images = vec![None, Some(png_url()), None];
		let output = svg_text(&render(
			"WEB",
			false,
			true,
			&builtin_palette("blue").unwrap(),
			&images,
		));
		assert_eq!(output.matches("<image ").count(), 1);
		assert!(output.contains("width=\"116\" height=\"116\""));
	}

	#[test]
	fn badges_escape_xml() {
		let output = svg_text(&render(
			"A&B <work>",
			false,
			true,
			&builtin_palette("blue").unwrap(),
			&[],
		));
		assert!(!output.contains("A&B"));
		assert!(output.contains("A&amp;B"));
	}

	#[test]
	fn hidden_badge_uses_visible_chrome() {
		let palette = builtin_palette("hidden").unwrap();
		let svg = svg_text(&render(
			"WEB",
			false,
			true,
			&builtin_palette("hidden").unwrap(),
			&[],
		));
		assert!(svg.contains(&format!("fill=\"{}\"", palette.chrome.surface)));
		assert!(svg.contains(&format!("stroke=\"{}\"", palette.chrome.border)));
		assert!(svg.contains(&format!("fill=\"{}\"", palette.chrome.text)));
	}
}
