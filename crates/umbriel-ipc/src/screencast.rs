use serde::Deserialize;

/// The accepted screencast command broadcast by Umbriel.
/// The serial advances even when the same target is selected again.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScreencastCommand {
	Window { serial: u64, identifier: String },
	Output { serial: u64, output: String },
	FollowWindow { serial: u64 },
	FollowOutput { serial: u64 },
	FollowStop { serial: u64 },
	Clear { serial: u64 },
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn decodes_fixed_source() {
		let event: ScreencastCommand = serde_json::from_value(json!({
			"serial": 42,
			"kind": "window",
			"identifier": "window-id-123"
		}))
		.unwrap();
		assert_eq!(
			event,
			ScreencastCommand::Window {
				serial: 42,
				identifier: "window-id-123".into(),
			}
		);
	}

	#[test]
	fn decodes_following_without_an_identifier() {
		let event: ScreencastCommand = serde_json::from_value(json!({
			"serial": 43,
			"kind": "follow_output"
		}))
		.unwrap();
		assert_eq!(event, ScreencastCommand::FollowOutput { serial: 43 });
	}
}
