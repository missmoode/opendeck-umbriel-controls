use crate::{Client, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Window {
	pub id: String,
	pub app_id: String,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub pid: Option<i64>,
	pub workspace: String,
	pub scratchpad: String,
	// Focus state and output-preview fields.
	#[serde(default)]
	pub active: bool,
	#[serde(default)]
	pub output: String,
	#[serde(default)]
	pub visible: bool,
}

impl Client {
	/// Query a fresh window snapshot (useful for debugging and tests).
	pub async fn list_windows(&self) -> Result<Vec<Window>> {
		let raw = self.send(&json!({ "cmd": "windows" })).await?;
		Ok(serde_json::from_value(raw)?)
	}

	/// Query the same focused window Umbriel would select for a click.
	pub async fn focused_window(&self) -> Result<Window> {
		let raw = self.send(&json!({"cmd": "focused-window"})).await?;
		Ok(serde_json::from_value(raw)?)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn decodes_membership_and_ignores_unneeded_fields() {
		let window: Window = serde_json::from_value(json!({
			"id": "top-42",
			"app_id": "org.mozilla.firefox",
			"workspace": "ws-7",
			"scratchpad": "",
			"title": "An unrelated title",
			"x": 100,
			"y": 200
		}))
		.unwrap();

		assert_eq!(window.title, "An unrelated title");
		assert_eq!(window.pid, None);
		assert_eq!(window.id, "top-42");
		assert_eq!(window.workspace, "ws-7");
		assert_eq!(window.scratchpad, "");
		assert!(!window.visible); // Older fixture omits the field.
		let with_pid: Window = serde_json::from_value(json!({
			"id": "b", "app_id": "game", "title": "My Game", "pid": -1,
			"workspace": "w", "scratchpad": ""
		}))
		.unwrap();
		assert_eq!(with_pid.pid, Some(-1));
	}
}
