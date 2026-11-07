//! Word-wrap automatic OpenDeck titles; the host only breaks at explicit newlines.

// OpenDeck's default key font is fairly large relative to the key width.
// This is a conservative character-based estimate, not font measurement.
const MAX_LINE_CHARS: usize = 9;

pub(crate) fn wrap_native_title(title: Option<String>) -> Option<String> {
	title.map(|title| wrap(&title))
}

fn wrap(title: &str) -> String {
	let mut lines = Vec::new();
	for paragraph in title.split('\n') {
		let mut line = String::new();
		for word in paragraph.split_whitespace() {
			let width = word.chars().count();
			if !line.is_empty() && line.chars().count() + 1 + width > MAX_LINE_CHARS {
				lines.push(std::mem::take(&mut line));
			}
			if width > MAX_LINE_CHARS {
				// With no word boundary available, divide the long word into
				// balanced pieces rather than leave one nearly-empty line.
				let chars: Vec<char> = word.chars().collect();
				let parts = chars.len().div_ceil(MAX_LINE_CHARS);
				let chunk_size = chars.len().div_ceil(parts);
				for chunk in chars.chunks(chunk_size) {
					lines.push(chunk.iter().collect::<String>());
				}
			} else {
				if !line.is_empty() {
					line.push(' ');
				}
				line.push_str(word);
			}
		}
		if !line.is_empty() || paragraph.trim().is_empty() {
			lines.push(line);
		}
	}
	lines.join("\n")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn follow_modes_wrap_without_changing_their_labels() {
		assert_eq!(wrap("Follow window"), "Follow\nwindow");
		assert_eq!(wrap("Follow output"), "Follow\noutput");
		assert_eq!(wrap("Focused window"), "Focused\nwindow");
	}

	#[test]
	fn short_titles_remain_unchanged() {
		assert_eq!(wrap("DP-1"), "DP-1");
		assert_eq!(wrap("Scrolling"), "Scrolling");
		assert_eq!(wrap_native_title(None), None);
	}

	#[test]
	fn breaks_at_words_before_splitting_long_words() {
		assert_eq!(wrap("Stardew Valley"), "Stardew\nValley");
		assert_eq!(wrap("Unassigned window"), "Unass\nigned\nwindow");
	}

	#[test]
	fn respects_explicit_line_breaks() {
		assert_eq!(wrap("One\nTwo three"), "One\nTwo three");
		assert_eq!(wrap("One\n\nTwo"), "One\n\nTwo");
	}
}
