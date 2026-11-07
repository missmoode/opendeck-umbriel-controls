use crate::{Client, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A subset of one entry returned by Umbriel's "workspaces" query.
#[derive(Debug, Deserialize, Serialize)]
pub struct Workspace {
	pub id: String,
	pub name: String,
	pub named: bool,
	pub index: u32,
	pub output: String,
	pub active: bool,
	#[serde(default)]
	pub focused: bool,
	/// Preserve unfamiliar layout values from future compositor versions.
	#[serde(default)]
	pub layout: String,
}

/// Umbriel accepts either a one-based position OR an exact name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceSelector {
	Index(u32),
	Name(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTarget {
	pub workspace: WorkspaceSelector,
	pub output: String,
}

impl Workspace {
	pub fn target(&self) -> WorkspaceTarget {
		let workspace = if self.named {
			WorkspaceSelector::Name(self.name.clone())
		} else {
			WorkspaceSelector::Index(self.index)
		};

		WorkspaceTarget {
			workspace,
			output: self.output.clone(),
		}
	}
}

impl Client {
	/// Return typed workspace entries rather than an unstructured Value.
	pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
		let raw: Value = self.workspaces().await?;
		Ok(serde_json::from_value(raw)?)
	}

	async fn workspace_action(&self, action: &str, target: &WorkspaceTarget) -> Result<()> {
		self.send(&json!({
			"cmd": "action",
			"action": action,
			"workspace": &target.workspace,
			"output": &target.output,
		}))
		.await?;
		Ok(())
	}

	/// Cycle the focused workspace through Umbriel's default modes, or a
	/// caller-supplied ordered subset (two or more distinct known modes).
	pub async fn cycle_workspace_layout(&self, cycle: &[String]) -> Result<()> {
		let mut request = json!({
			"cmd": "action",
			"action": "workspace-set-layout",
			"layout": "toggle",
		});
		if !cycle.is_empty() {
			request["cycle"] = json!(cycle);
		}
		self.send(&request).await?;
		Ok(())
	}

	pub async fn switch_workspace(&self, target: &WorkspaceTarget) -> Result<()> {
		self.workspace_action("workspace-switch", target).await
	}

	pub async fn move_window_to_workspace(&self, target: &WorkspaceTarget) -> Result<()> {
		self.workspace_action("window-move-to-workspace", target)
			.await
	}

	pub async fn move_window_to_workspace_silent(&self, target: &WorkspaceTarget) -> Result<()> {
		self.workspace_action("window-move-to-workspace-silent", target)
			.await
	}

	pub async fn move_column_to_workspace(&self, target: &WorkspaceTarget) -> Result<()> {
		self.workspace_action("column-move-to-workspace", target)
			.await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn numeric_name_is_not_a_numeric_position() {
		let workspace = Workspace {
			id: "ws-example".to_owned(),
			name: "2".to_owned(),
			named: true,
			index: 1,
			output: "DP-2".to_owned(),
			active: false,
			focused: false,
			layout: "scrolling".to_owned(),
		};

		let target = workspace.target();
		let value = serde_json::to_value(target).unwrap();

		assert_eq!(
			value,
			json!({
				"workspace": "2",
				"output": "DP-2",
			})
		);
	}

	#[test]
	fn anonymous_workspace_uses_its_position() {
		let workspace = Workspace {
			id: "ws-example".to_owned(),
			name: "2".to_owned(),
			named: false,
			index: 2,
			output: "DP-1".to_owned(),
			active: false,
			focused: false,
			layout: "scrolling".to_owned(),
		};

		let target = workspace.target();
		let value = serde_json::to_value(target).unwrap();

		assert_eq!(
			value,
			json!({
				"workspace": 2,
				"output": "DP-1",
			})
		);
	}
}
