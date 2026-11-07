use base64::{Engine as _, engine::general_purpose::STANDARD};

pub fn escape_text(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&apos;")
}

pub fn data_url(svg: &str) -> String {
	format!(
		"data:image/svg+xml;base64,{}",
		STANDARD.encode(svg.as_bytes())
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn escapes_label_text() {
		assert_eq!(escape_text("A&B <test>"), "A&amp;B &lt;test&gt;");
	}
}
