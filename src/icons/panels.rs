use crate::icons::{
	layout::{self, Role, Slot},
	palette::{IconColours, Palette},
};

fn colours_for(palette: &Palette, role: Role) -> &IconColours {
	match role {
		Role::Secondary => &palette.secondary,
		Role::Accent => &palette.accent,
	}
}

pub fn markup(icons: &[&str], palette: &Palette) -> String {
	if icons.is_empty() {
		// No resolved apps: intentional empty-workspace artwork.
		return format!(
			"<rect x=\"14\" y=\"14\" width=\"116\" height=\"116\" \
             rx=\"18\" fill=\"none\" stroke=\"{}\" stroke-width=\"3\"/>\
             <rect x=\"47\" y=\"50\" width=\"50\" height=\"44\" \
             rx=\"9\" fill=\"none\" stroke=\"{}\" stroke-width=\"3\"/>\
             <path d=\"M47 62H97\" stroke=\"{}\" stroke-width=\"3\"/>",
			palette.muted.stroke, palette.muted.stroke, palette.muted.stroke,
		);
	}

	let mut result = String::new();
	for (slot, url) in layout::slots_for(icons.len()).iter().zip(icons) {
		let colours = colours_for(palette, slot.role);
		let radius = if slot.w > 100 { 18 } else { 10 };
		let (icon_x, icon_y, icon_size) = image_geometry(slot);

		// A full-sized coloured window panel. Its app icon is contained.
		result.push_str(&format!(
			"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             rx=\"{radius}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"3\"/>",
			slot.x, slot.y, slot.w, slot.h, colours.fill, colours.stroke,
		));
		// `url` is our internally generated PNG data URL, not arbitrary markup.
		result.push_str(&format!(
			"<image x=\"{icon_x}\" y=\"{icon_y}\" \
             width=\"{icon_size}\" height=\"{icon_size}\" \
             preserveAspectRatio=\"xMidYMid meet\" href=\"{url}\"/>",
		));
	}
	result
}

/// Fit a square icon within the pane without stretching it.
/// The top row reserves space below the corner badge.
fn image_geometry(slot: &Slot) -> (u16, u16, u16) {
	let top = slot.y + 7;
	let bottom = slot.y + slot.h - 7;
	let usable_height = bottom.saturating_sub(top);
	let size = slot.w.saturating_sub(14).min(usable_height).min(72);
	let x = slot.x + (slot.w - size) / 2;
	let y = top + (usable_height - size) / 2;
	(x, y, size)
}
