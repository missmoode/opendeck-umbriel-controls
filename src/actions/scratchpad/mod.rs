//! Scratchpad action: settings, key events and registration.
mod icon;
mod operations;
mod render;

use crate::{
	actions::{
		buttons::TrackedButtons,
		press::{PressKind, TimedPresses},
		ui::ActionUi,
	},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::WindowUpdates,
};
use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

const HOLD_THRESHOLD: Duration = Duration::from_millis(650);

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
	scratchpad: Option<String>,
	palette: String,
	hide_on_remove: bool,
	show_title: bool,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			scratchpad: None,
			palette: String::new(),
			hide_on_remove: true,
			show_title: false,
		}
	}
}

fn selected_name(settings: &Settings) -> Option<&str> {
	settings
		.scratchpad
		.as_deref()
		.map(str::trim)
		.filter(|name| !name.is_empty())
}

struct ScratchpadToggle {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<Settings>,
	pressed: TimedPresses,
}

impl ScratchpadToggle {
	async fn record_settings(&self, instance: &Instance, settings: &Settings) {
		self.buttons
			.put(&instance.instance_id, settings.clone())
			.await;
	}
}

#[async_trait]
impl Action for ScratchpadToggle {
	const UUID: &'static str = "dev.moode.umbriel-controls.scratchpad-toggle";

	type Settings = Settings;

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		self.ui.respond_to_request(instance, payload).await?;
		Ok(())
	}

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

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let settings = settings.clone();
		self.pressed
			.arm(
				instance,
				HOLD_THRESHOLD,
				move |original, guard| async move {
					if !guard.is_current().await {
						return;
					}
					if let Err(error) = operations::run_action(&original, &settings, true).await {
						eprintln!("Scratchpad hold failed: {error}");
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
		if let Some((kind, guard)) = self.pressed.up(&instance.instance_id, HOLD_THRESHOLD).await
			&& guard.is_current().await
		{
			return operations::run_action(instance, settings, kind == PressKind::Hold).await;
		}
		Ok(())
	}
}

pub async fn register(
	windows: WindowUpdates,
	resolver: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	ui: Arc<ActionUi>,
) {
	let (buttons, refresh_rx) = TrackedButtons::new();
	ui.register(ScratchpadToggle {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
		pressed: TimedPresses::default(),
	})
	.await;
	render::spawn(windows, resolver, themes, buttons, refresh_rx);
}
