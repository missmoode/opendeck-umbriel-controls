#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
	Secondary,
	Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
	pub x: u16,
	pub y: u16,
	pub w: u16,
	pub h: u16,
	pub role: Role,
}

const EMPTY: [Slot; 0] = [];

const ONE: [Slot; 1] = [Slot {
	x: 14,
	y: 14,
	w: 116,
	h: 116,
	role: Role::Accent,
}];

const TWO: [Slot; 2] = [
	Slot {
		x: 14,
		y: 14,
		w: 55,
		h: 116,
		role: Role::Accent,
	},
	Slot {
		x: 75,
		y: 14,
		w: 55,
		h: 116,
		role: Role::Accent,
	},
];

const THREE: [Slot; 3] = [
	Slot {
		x: 14,
		y: 14,
		w: 55,
		h: 55,
		role: Role::Accent,
	},
	Slot {
		x: 14,
		y: 75,
		w: 55,
		h: 55,
		role: Role::Secondary,
	},
	Slot {
		x: 75,
		y: 14,
		w: 55,
		h: 116,
		role: Role::Accent,
	},
];

const FOUR: [Slot; 4] = [
	Slot {
		x: 14,
		y: 14,
		w: 55,
		h: 55,
		role: Role::Accent,
	},
	Slot {
		x: 75,
		y: 14,
		w: 55,
		h: 55,
		role: Role::Secondary,
	},
	Slot {
		x: 14,
		y: 75,
		w: 55,
		h: 55,
		role: Role::Secondary,
	},
	Slot {
		x: 75,
		y: 75,
		w: 55,
		h: 55,
		role: Role::Accent,
	},
];

/// The render layer limits its resolved icons to four first.
pub(crate) fn slots_for(count: usize) -> &'static [Slot] {
	match count {
		0 => &EMPTY,
		1 => &ONE,
		2 => &TWO,
		3 => &THREE,
		_ => &FOUR,
	}
}
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn slots_fit_and_do_not_overlap() {
		for count in 1..=4 {
			let slots = slots_for(count);
			for (index, a) in slots.iter().enumerate() {
				assert!(a.x >= 14 && a.y >= 14);
				assert!(a.x + a.w <= 130 && a.y + a.h <= 130);
				for b in slots.iter().skip(index + 1) {
					let separated = a.x + a.w <= b.x
						|| b.x + b.w <= a.x
						|| a.y + a.h <= b.y
						|| b.y + b.h <= a.y;
					assert!(separated, "panels overlap at count {count}");
				}
			}
		}
	}

	#[test]
	fn third_panel_spans_both_rows_on_the_right() {
		let slots = slots_for(3);
		assert_eq!((slots[2].x, slots[2].y), (75, 14));
		assert_eq!((slots[2].w, slots[2].h), (55, 116));
		assert_eq!(slots[0].role, Role::Accent);
		assert_eq!(slots[2].role, Role::Accent);
	}
}
