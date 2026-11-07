mod modes;
mod render;
mod state;

use std::sync::Arc;

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde_json::Value;
use umbriel_ipc::Client;

use super::model::{Mode, Settings};
use crate::{
	actions::{buttons::TrackedButtons, press::TimedPresses, ui::ActionUi},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::{OutputUpdates, ScreencastUpdates, SessionUpdates, WindowUpdates},
};
use state::ButtonState;

#[derive(Clone)]
struct TargetAction {
	outputs: OutputUpdates,
	session: SessionUpdates,
	buttons: TrackedButtons<ButtonState>,
	pressed: TimedPresses,
	settings_writes: Arc<tokio::sync::Mutex<()>>,
	ui: Arc<ActionUi>,
	applications: Arc<ApplicationIcons>,
}

#[async_trait]
impl Action for TargetAction {
	const UUID: &'static str = "dev.moode.umbriel-controls.screencast-target";
	type Settings = Settings;

	async fn will_appear(&self, instance: &Instance, settings: &Settings) -> OpenActionResult<()> {
		self.record_settings(instance, settings, true).await?;
		self.buttons.retry_initial_icon(instance).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Settings,
	) -> OpenActionResult<()> {
		self.record_settings(instance, settings, false).await
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Settings,
	) -> OpenActionResult<()> {
		self.pressed.cancel(&instance.instance_id).await;
		self.buttons.remove(&instance.instance_id).await;
		Ok(())
	}

	async fn key_down(&self, instance: &Instance, settings: &Settings) -> OpenActionResult<()> {
		let mode = Mode::from_setting(&settings.mode);
		if mode == Mode::ProgrammedWindow {
			return self.key_down_programmed(instance).await;
		}
		if mode == Mode::DefinedApplication {
			return self.select_application(instance).await;
		}

		if mode == Mode::SpecificOutput && !modes::output::is_available(settings, &self.outputs) {
			instance.show_alert().await?;
			return Ok(());
		}

		let result = match Client::from_environment() {
			Ok(client) => match mode {
				Mode::FocusedWindow => client.screencast_set_window(None).await,
				Mode::SpecificOutput => {
					client
						.screencast_set_output(Some(settings.output.trim()))
						.await
				}
				Mode::FollowWindow => client.screencast_follow_window().await,
				Mode::FollowOutput => client.screencast_follow_output().await,
				Mode::ProgrammedWindow | Mode::DefinedApplication => unreachable!(),
			},
			Err(error) => Err(error),
		};
		if let Err(error) = result {
			eprintln!("Screencast target change failed: {error}");
			instance.show_alert().await?;
		} else {
			self.buttons.refresh(&instance.instance_id).await;
		}
		Ok(())
	}

	async fn key_up(&self, instance: &Instance, settings: &Settings) -> OpenActionResult<()> {
		if Mode::from_setting(&settings.mode) == Mode::ProgrammedWindow {
			self.key_up_programmed(instance).await?;
		}
		Ok(())
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Settings,
		payload: &Value,
	) -> OpenActionResult<()> {
		if self.ui.respond_to_request(instance, payload).await? {
			return Ok(());
		}
		match payload.get("event").and_then(Value::as_str) {
			Some("getOutputs") => modes::output::send_choices(instance).await?,
			Some("getApplicationConversion") => {
				modes::application::send_conversion(
					instance,
					Arc::clone(&self.applications),
					payload
						.get("application")
						.and_then(Value::as_str)
						.unwrap_or_default(),
				)
				.await?;
			}
			Some("getApplications") => {
				// A catalogue rescan notifies all icon renderers itself.
				modes::application::send_choices(
					instance,
					Arc::clone(&self.applications),
					&settings.application,
				)
				.await?;
			}
			_ => {}
		}
		Ok(())
	}
}

pub async fn register(
	windows: WindowUpdates,
	outputs: OutputUpdates,
	screencast: ScreencastUpdates,
	session: SessionUpdates,
	icons: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	ui: Arc<ActionUi>,
) {
	let (buttons, refresh_rx) = TrackedButtons::new();
	let pressed = TimedPresses::default();
	let action = TargetAction {
		outputs: outputs.clone(),
		session: session.clone(),
		buttons: buttons.clone(),
		pressed: pressed.clone(),
		settings_writes: Arc::new(tokio::sync::Mutex::new(())),
		ui: Arc::clone(&ui),
		applications: Arc::clone(&icons),
	};
	ui.register(action.clone()).await;
	render::spawn(
		action,
		render::TargetUpdates {
			windows,
			outputs,
			screencast,
			session,
		},
		icons,
		themes,
		refresh_rx,
	);
}
