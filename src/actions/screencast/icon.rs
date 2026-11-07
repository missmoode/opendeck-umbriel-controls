use crate::actions::screencast::model::{Display, Visual};
use crate::icons::{application::ApplicationIcons, palette::Palette, panels, svg};

const TEMPLATE: &str = include_str!("target.svg");
const WINDOW: &str = include_str!("assets/window.svg");
const OUTPUT: &str = include_str!("assets/output.svg");
const FOLLOW_WINDOW: &str = include_str!("assets/follow-window.svg");
const FOLLOW_OUTPUT: &str = include_str!("assets/follow-output.svg");

/// All standalone art uses a 64x64 viewBox. Nested SVGs remain valid SVG,
/// and the transform affects artwork, not the capture brackets.
fn embedded(art: &str, x: u16, y: u16, size: u16, text_color: &str) -> String {
	let scale = f32::from(size) / 64.0;
	format!(r##"<g transform="translate({x} {y}) scale({scale})" color="{text_color}">{art}</g>"##)
}

fn window_glyph(
	app_id: &str,
	x: u16,
	y: u16,
	size: u16,
	icons: &ApplicationIcons,
	text_color: &str,
) -> String {
	if let Some(url) = icons.for_app(app_id) {
		// The image resolver owns this PNG data URL, not an untrusted event.
		return format!(
			r#"<image x="{x}" y="{y}" width="{size}" height="{size}" preserveAspectRatio="xMidYMid meet" href="{url}"/>"#
		);
	}
	embedded(WINDOW, x, y, size, text_color)
}

fn main_markup(visual: &Visual, palette: &Palette, icons: &ApplicationIcons) -> String {
	let text_color = palette.muted.text.as_str();
	match visual {
		Visual::Window { app_id, .. } => window_glyph(app_id, 22, 22, 100, icons, text_color),
		Visual::Application {
			desktop_id,
			running,
		} => {
			let artwork = icons
				.for_desktop_id(desktop_id)
				.map(|url| format!(r#"<image x="22" y="22" width="100" height="100" preserveAspectRatio="xMidYMid meet" href="{url}"/>"#))
				.unwrap_or_else(|| embedded(WINDOW, 22, 22, 100, text_color));
			if *running != Some(false) {
				artwork
			} else {
				format!(
					r#"{artwork}<path d="M36 108L108 36" stroke="{}" stroke-width="6" stroke-linecap="round"/>"#,
					palette.chrome.indicator
				)
			}
		}
		Visual::CustomApplication {
			key,
			app_id,
			running,
		} => {
			let artwork = icons.for_custom(key, app_id.as_deref())
                .map(|url| format!(r#"<image x="22" y="22" width="100" height="100" preserveAspectRatio="xMidYMid meet" href="{url}"/>"#))
                .unwrap_or_else(|| embedded(WINDOW, 22, 22, 100, text_color));
			if *running != Some(false) {
				artwork
			} else {
				format!(
					r#"{artwork}<path d="M36 108L108 36" stroke="{}" stroke-width="6" stroke-linecap="round"/>"#,
					palette.chrome.indicator
				)
			}
		}
		Visual::Output { apps, .. } => {
			let resolved: Vec<_> = apps.iter().map(|app| icons.for_app(app)).collect();
			let visible: Vec<_> = resolved
				.iter()
				.filter_map(Option::as_deref)
				.filter(|url| url.starts_with("data:image/png;base64,"))
				.take(4)
				.collect();
			if visible.is_empty() {
				embedded(OUTPUT, 22, 22, 100, text_color)
			} else {
				panels::markup(&visible, palette)
			}
		}
		Visual::FollowWindow => embedded(FOLLOW_WINDOW, 22, 22, 100, text_color),
		Visual::FollowOutput => embedded(FOLLOW_OUTPUT, 22, 22, 100, text_color),
		Visual::UnknownWindow => embedded(WINDOW, 22, 22, 100, text_color),
		Visual::UnknownOutput => {
			let mut markup = embedded(OUTPUT, 22, 22, 100, text_color);
			markup.push_str(&format!(
				r#"<path d="M36 108L108 36" stroke="{}" stroke-width="6" stroke-linecap="round"/>"#,
				palette.chrome.indicator
			));
			markup
		}
	}
}

/// A pin is a reusable SVG decoration, not part of any application glyph.
fn pin_markup(x: u16, y: u16, palette: &Palette) -> String {
	let fill = palette.chrome.indicator.as_str();
	let stroke = palette.chrome.indicator_border.as_str();
	format!(
		r#"<g transform="translate({x} {y}) rotate(32) scale(1.5)" fill="{fill}" stroke="{stroke}" stroke-width="2" stroke-linecap="round"><circle r="6"/><path d="M0 5V17" stroke="{fill}" stroke-width="3"/></g>"#
	)
}

/// The secondary target is always inset in the upper-left corner.
fn inset_markup(url: &str, selected: bool, palette: &Palette) -> String {
	let fill = if selected {
		palette.muted.fill.as_str()
	} else {
		"none"
	};
	let stroke = if selected {
		palette.chrome.selection.as_str()
	} else {
		palette.chrome.border.as_str()
	};
	format!(
		r#"<rect x="14" y="14" width="42" height="42" rx="10" fill="{}"/><rect x="14" y="14" width="42" height="42" rx="10" fill="{fill}" stroke="{stroke}" stroke-width="3"/><image x="20" y="20" width="30" height="30" preserveAspectRatio="xMidYMid meet" href="{url}"/>"#,
		palette.chrome.surface
	)
}

pub fn render(display: &Display, palette: &Palette, icons: &ApplicationIcons) -> String {
	// An inset is a preview of a real app icon, never a placeholder window.
	let inset = display
		.inset
		.as_ref()
		.and_then(|visual| match visual {
			Visual::Window { app_id, .. } => icons
				.for_app(app_id)
				.map(|url| inset_markup(&url, display.accepted_inset, palette)),
			_ => None,
		})
		.unwrap_or_default();
	let accepted_inset = display.accepted_inset && !inset.is_empty();
	let background = if display.selected && !accepted_inset {
		format!(
			r#"<rect x="2" y="2" width="140" height="140" rx="18" fill="{}" stroke="{}" stroke-width="3"/>"#,
			palette.muted.fill, palette.muted.stroke
		)
	} else {
		String::new()
	};
	let main = main_markup(&display.main, palette, icons);
	// Only a deliberately programmed target gets a pin; it is always the main icon.
	let pin = if display.pinned {
		pin_markup(114, 33, palette)
	} else {
		String::new()
	};
	let brackets = if display.selected && !accepted_inset {
		palette.chrome.selection.as_str()
	} else {
		palette.chrome.border.as_str()
	};
	let markup = TEMPLATE
		.replace("{{BACKGROUND}}", &background)
		.replace("{{MAIN}}", &main)
		.replace("{{INSET}}", &inset)
		.replace("{{PIN}}", &pin)
		.replace("{{BRACKET_STROKE}}", brackets);
	svg::data_url(&markup)
}

/// Neutral pause control using the same inactive brackets as target buttons.
pub fn render_pause(palette: &Palette) -> String {
	let pause = format!(
		r#"<g fill="{}"><rect x="57" y="48" width="12" height="48" rx="4"/><rect x="75" y="48" width="12" height="48" rx="4"/></g>"#,
		palette.chrome.text
	);
	let markup = TEMPLATE
		.replace("{{BACKGROUND}}", "")
		.replace("{{MAIN}}", &pause)
		.replace("{{INSET}}", "")
		.replace("{{PIN}}", "")
		.replace("{{BRACKET_STROKE}}", &palette.chrome.border);
	svg::data_url(&markup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::icons::palette::builtin_palette;
	use base64::{Engine as _, engine::general_purpose::STANDARD};

	fn render(display: &Display, theme: &str, icons: &ApplicationIcons) -> String {
		super::render(display, &builtin_palette(theme).unwrap(), icons)
	}

	fn decode(url: &str) -> String {
		let data = url.strip_prefix("data:image/svg+xml;base64,").unwrap();
		String::from_utf8(STANDARD.decode(data).unwrap()).unwrap()
	}

	#[test]
	fn accepted_main_brackets_use_chrome_selection_and_follow_modes_differ() {
		let icons = ApplicationIcons::new();
		let make = |main| Display {
			selected: true,
			accepted_inset: false,
			pinned: false,
			main,
			inset: None,
		};
		let window = decode(&render(&make(Visual::FollowWindow), "hidden", &icons));
		let output = decode(&render(&make(Visual::FollowOutput), "hidden", &icons));
		let palette = builtin_palette("hidden").unwrap();
		let selection = &palette.chrome.selection;
		assert!(window.contains(&format!(r#"<g stroke="{selection}" stroke-width="5""#)));
		assert!(output.contains(&format!(r#"<g stroke="{selection}" stroke-width="5""#)));
		assert_ne!(window, output);
		assert!(window.contains("M7 22H57"));
		assert!(output.contains("M32 47v10"));
	}

	#[test]
	fn defined_application_uses_no_inset_or_pin_and_marks_absence() {
		let icons = ApplicationIcons::new();
		let display = Display {
			selected: false,
			accepted_inset: false,
			pinned: false,
			main: Visual::Application {
				desktop_id: "org.example.Missing".into(),
				running: Some(false),
			},
			inset: None,
		};
		let svg = decode(&render(&display, "hidden", &icons));
		assert!(svg.contains("M36 108L108 36"));
		assert!(!svg.contains("x=\"14\" y=\"14\" width=\"42\""));
		let running = Display {
			main: Visual::Application {
				desktop_id: "org.example.Missing".into(),
				running: Some(true),
			},
			..display
		};
		assert!(!decode(&render(&running, "hidden", &icons)).contains("M36 108L108 36"));
	}

	#[test]
	fn no_inset_when_unselected_and_hidden_palette_keeps_brackets() {
		let icons = ApplicationIcons::new();
		let state = Display {
			selected: false,
			accepted_inset: false,
			pinned: false,
			main: Visual::UnknownOutput,
			inset: None,
		};
		let svg = decode(&render(&state, "hidden", &icons));
		let palette = builtin_palette("hidden").unwrap();
		let border = &palette.chrome.border;
		assert!(svg.contains(&format!(r#"<g stroke="{border}" stroke-width="5""#)));
		assert!(!svg.contains("x=\"14\" y=\"14\" width=\"42\""));
		assert!(!svg.contains("x=\"2\" y=\"2\""));
	}

	#[test]
	fn inset_overlays_main_but_brackets_are_drawn_last() {
		let icons = ApplicationIcons::with_test_entries(vec![]);
		icons.seed_test_icon("known.icon");
		let state = Display {
			selected: true,
			accepted_inset: false,
			pinned: false,
			main: Visual::UnknownWindow,
			inset: Some(Visual::Window {
				id: "w2".into(),
				app_id: "known.icon".into(),
			}),
		};
		let svg = decode(&render(&state, "blue", &icons));
		let inset = svg.find("x=\"14\" y=\"14\" width=\"42\"").unwrap();
		let brackets = svg.find("M9 38V20").unwrap();
		assert!(inset < brackets);
		let palette = builtin_palette("blue").unwrap();
		let chrome = &palette.chrome;
		assert!(svg.contains(&format!(r#"stroke="{}" stroke-width="3""#, chrome.border)));
		assert!(svg.contains(&format!(
			r#"<g stroke="{}" stroke-width="5""#,
			chrome.selection
		)));
	}

	#[test]
	fn accepted_inset_gets_palette_background_instead_of_main() {
		let icons = ApplicationIcons::with_test_entries(vec![]);
		icons.seed_test_icon("known.icon");
		let state = Display {
			selected: true,
			accepted_inset: true,
			pinned: false,
			main: Visual::UnknownWindow,
			inset: Some(Visual::Window {
				id: "w1".into(),
				app_id: "known.icon".into(),
			}),
		};
		for kind in ["blue", "hidden"] {
			let svg = decode(&render(&state, kind, &icons));
			let palette = builtin_palette(kind).unwrap();
			assert!(!svg.contains("x=\"2\" y=\"2\""));
			let surface = format!(
				r#"x="14" y="14" width="42" height="42" rx="10" fill="{}"/>"#,
				palette.chrome.surface
			);
			let selection = format!(
				r#"x="14" y="14" width="42" height="42" rx="10" fill="{}" stroke="{}" stroke-width="3""#,
				palette.muted.fill,
				palette.chrome.selection.as_str()
			);
			assert!(svg.find(&surface).unwrap() < svg.find(&selection).unwrap());
			assert!(svg.contains(&format!(
				r#"<g stroke="{}" stroke-width="5""#,
				palette.chrome.border.as_str()
			)));
		}
	}

	#[test]
	fn red_palette_pin_and_hidden_chrome_remain_visible() {
		let icons = ApplicationIcons::new();
		let state = Display {
			selected: true,
			accepted_inset: false,
			pinned: true,
			main: Visual::UnknownWindow,
			inset: None,
		};
		let red = builtin_palette("red").unwrap();
		let svg = decode(&render(&state, "red", &icons));
		assert_ne!(red.chrome.indicator, red.accent.fill);
		assert!(svg.contains(&format!(
			"fill=\"{}\" stroke=\"{}\"",
			red.chrome.indicator, red.chrome.indicator_border
		)));
		let hidden = builtin_palette("hidden").unwrap();
		let svg = decode(&render(&state, "hidden", &icons));
		assert!(svg.contains(&format!(
			r#"<g stroke="{}" stroke-width="5""#,
			hidden.chrome.selection
		)));
		assert!(svg.contains(&format!(
			"fill=\"{}\" stroke=\"{}\"",
			hidden.chrome.indicator, hidden.chrome.indicator_border
		)));
	}
	#[test]
	fn missing_icons_never_produce_placeholder_insets() {
		let icons = ApplicationIcons::with_test_entries(vec![]);
		let palette = builtin_palette("hidden").unwrap();
		for inset in [
			Some(Visual::UnknownWindow),
			Some(Visual::Window {
				id: "w2".into(),
				app_id: "missing.icon".into(),
			}),
		] {
			let display = Display {
				selected: true,
				accepted_inset: true,
				pinned: false,
				main: Visual::UnknownWindow,
				inset,
			};
			let raw = decode(&super::render(&display, &palette, &icons));
			assert!(!raw.contains("x=\"14\" y=\"14\" width=\"42\""));
			assert!(raw.contains("x=\"2\" y=\"2\""));
		}
	}

	#[test]
	fn unavailable_custom_application_uses_generic_glyph_and_slash() {
		let icons = ApplicationIcons::with_test_entries(vec![]);
		let display = Display {
			selected: false,
			accepted_inset: false,
			pinned: false,
			main: Visual::CustomApplication {
				key: "my-rule".into(),
				app_id: None,
				running: Some(false),
			},
			inset: None,
		};
		let palette = builtin_palette("hidden").unwrap();
		let raw = decode(&super::render(&display, &palette, &icons));
		assert!(raw.contains("M36 108L108 36"));
		assert!(!raw.contains("{{ANNOTATION}}"));
	}
}
