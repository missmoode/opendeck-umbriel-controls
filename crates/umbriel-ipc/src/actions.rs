use crate::{Client, Result};
use serde_json::{Value, json};

impl Client {
	async fn scratchpad_action(&self, action: &str, name: Option<&str>) -> Result<()> {
		let mut request = json!({ "cmd": "action", "action": action });
		if let Some(name) = name.filter(|name| !name.is_empty()) {
			request["scratchpad"] = json!(name);
		}
		self.send(&request).await?;
		Ok(())
	}

	/// Toggle the implicit scratchpad (None), or a named scratchpad.
	pub async fn toggle_scratchpad(&self, name: Option<&str>) -> Result<()> {
		self.scratchpad_action("scratchpad-toggle", name).await
	}

	/// Move the focused workspace window into the selected scratchpad.
	pub async fn move_window_to_scratchpad(&self, name: Option<&str>) -> Result<()> {
		self.scratchpad_action("window-move-to-scratchpad", name)
			.await
	}

	/// Reveal a hidden scratchpad window (if necessary) and focus it.
	pub async fn focus_window(&self, id: &str) -> Result<()> {
		self.send(&json!({"cmd": "action", "action": "window-focus", "window": id}))
			.await?;
		Ok(())
	}

	/// Return Umbriel's raw workspace inventory as JSON.
	pub async fn workspaces(&self) -> Result<Value> {
		self.send(&json!({"cmd": "workspaces"})).await
	}

	/// Choose the seat's focused window when `window_id` is None.
	pub async fn screencast_set_window(&self, window_id: Option<&str>) -> Result<()> {
		let mut request = json!({"cmd": "action", "action": "screencast-set-window"});
		if let Some(id) = window_id.filter(|value| !value.is_empty()) {
			request["window"] = json!(id);
		}
		self.send(&request).await?;
		Ok(())
	}

	/// Choose the focused output when `output_name` is None.
	pub async fn screencast_set_output(&self, output_name: Option<&str>) -> Result<()> {
		let mut request = json!({"cmd": "action", "action": "screencast-set-output"});
		if let Some(name) = output_name.filter(|value| !value.is_empty()) {
			request["output"] = json!(name);
		}
		self.send(&request).await?;
		Ok(())
	}

	pub async fn screencast_follow_window(&self) -> Result<()> {
		self.send(&json!({"cmd":"action","action":"screencast-follow-window"}))
			.await?;
		Ok(())
	}

	pub async fn screencast_follow_output(&self) -> Result<()> {
		self.send(&json!({"cmd":"action","action":"screencast-follow-output"}))
			.await?;
		Ok(())
	}

	pub async fn screencast_clear(&self) -> Result<()> {
		self.send(&json!({"cmd":"action","action":"screencast-clear"}))
			.await?;
		Ok(())
	}
}
