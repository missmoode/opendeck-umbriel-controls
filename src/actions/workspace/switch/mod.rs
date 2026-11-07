//! Workspace Switch action: settings, OpenAction events and registration.
use crate::{
	actions::{
		buttons::TrackedButtons,
		press::{PressKind, TimedPresses},
		ui::ActionUi,
	},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::{WindowUpdates, WorkspaceUpdates},
};
use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use umbriel_ipc::Client;

mod icon;
mod operations;
mod render;
mod target;
#[cfg(test)]
mod tests;

const HOLD_THRESHOLD: Duration = Duration::from_millis(650);
const INACTIVE: u16 = 0;
const ACTIVE: u16 = 1;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
	target: String,
	label: String,
	palette: String,
	show_badge: bool,
	hold_action: HoldAction,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			target: String::new(),
			label: String::new(),
			palette: String::new(),
			show_badge: true,
			hold_action: HoldAction::default(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HoldAction {
	Disabled,
	#[default]
	MoveWindow,
	MoveWindowSilent,
	MoveColumn,
}

struct WorkspaceSwitch {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<Settings>,
	pressed: TimedPresses,
}

impl WorkspaceSwitch {
	async fn record_settings(&self, instance: &Instance, settings: &Settings) {
		self.buttons
			.put(&instance.instance_id, settings.clone())
			.await;
	}
}

#[async_trait]
impl Action for WorkspaceSwitch {
	const UUID: &'static str = "dev.moode.umbriel-controls.workspace-switch";

	type Settings = Settings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.record_settings(instance, settings).await;
		self.buttons.retry_initial_icon(instance).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.pressed.cancel(&instance.instance_id).await;
		self.record_settings(instance, settings).await;
		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.pressed.cancel(&instance.instance_id).await;
		self.buttons.remove(&instance.instance_id).await;
		Ok(())
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &Value,
	) -> OpenActionResult<()> {
		if self.ui.respond_to_request(instance, payload).await? {
			return Ok(());
		}

		if payload.get("event").and_then(Value::as_str) != Some("getWorkspaceDestinations") {
			return Ok(());
		}

		// Return the full inventories. The inspector filters workspaces by the
		// selected output, without changing the stored WorkspaceTarget format.
		let (outputs, workspaces) = match Client::from_environment() {
			Ok(client) => {
				let outputs = client.list_outputs().await;
				let workspaces = client.list_workspaces().await;
				match (outputs, workspaces) {
					(Ok(outputs), Ok(workspaces)) => (outputs, workspaces),
					(Err(error), _) | (_, Err(error)) => {
						eprintln!("Failed to list Umbriel destinations: {error}");
						(Vec::new(), Vec::new())
					}
				}
			}
			Err(error) => {
				eprintln!("Failed to connect to Umbriel for destinations: {error}");
				(Vec::new(), Vec::new())
			}
		};

		instance
			.send_to_property_inspector(json!({
				"event": "getWorkspaceDestinations",
				"outputs": outputs,
				"workspaces": workspaces,
			}))
			.await?;

		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		if settings.hold_action == HoldAction::Disabled {
			return operations::run_workspace_action(instance, settings, false).await;
		}

		let settings = settings.clone();
		self.pressed
			.arm(
				instance,
				HOLD_THRESHOLD,
				move |original, guard| async move {
					if !guard.is_current().await {
						return;
					}
					if let Err(error) =
						operations::run_workspace_action(&original, &settings, true).await
					{
						eprintln!("Workspace hold failed: {error}");
					}
				},
			)
			.await;
		Ok(())
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		if !crate::actions::instance_is_current(instance).await {
			self.pressed.cancel(&instance.instance_id).await;
			return Ok(());
		}
		if settings.hold_action == HoldAction::Disabled {
			return Ok(());
		}

		if let Some((kind, guard)) = self.pressed.up(&instance.instance_id, HOLD_THRESHOLD).await
			&& guard.is_current().await
		{
			return operations::run_workspace_action(instance, settings, kind == PressKind::Hold)
				.await;
		}
		Ok(())
	}
}

pub async fn register(
	workspaces: WorkspaceUpdates,
	windows: WindowUpdates,
	resolver: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	ui: Arc<ActionUi>,
) {
	let (buttons, refresh_rx) = TrackedButtons::new();
	ui.register(WorkspaceSwitch {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
		pressed: TimedPresses::default(),
	})
	.await;
	render::spawn(workspaces, windows, resolver, themes, buttons, refresh_rx);
}
