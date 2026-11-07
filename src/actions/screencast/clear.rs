use std::sync::Arc;

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use umbriel_ipc::Client;

use super::icon;
use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		ui::ActionUi,
	},
	themes::ThemeManager,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Settings {
	palette: String,
}

struct ClearScreencast {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<String>,
}

impl ClearScreencast {
	async fn record_settings(&self, instance: &Instance, settings: &Settings) {
		self.buttons
			.put(&instance.instance_id, settings.palette.clone())
			.await;
	}
}

#[async_trait]
impl Action for ClearScreencast {
	const UUID: &'static str = "dev.moode.umbriel-controls.screencast-clear";
	type Settings = Settings;

	async fn will_appear(&self, instance: &Instance, settings: &Settings) -> OpenActionResult<()> {
		self.record_settings(instance, settings).await;
		self.buttons.retry_initial_icon(instance).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Settings,
	) -> OpenActionResult<()> {
		self.record_settings(instance, settings).await;
		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Settings,
	) -> OpenActionResult<()> {
		self.buttons.remove(&instance.instance_id).await;
		Ok(())
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Settings,
		payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		self.ui.respond_to_request(instance, payload).await?;
		Ok(())
	}

	async fn key_down(&self, instance: &Instance, _settings: &Settings) -> OpenActionResult<()> {
		let result = match Client::from_environment() {
			Ok(client) => client.screencast_clear().await,
			Err(error) => Err(error),
		};
		if let Err(error) = result {
			eprintln!("Screencast clear failed: {error}");
			instance.show_alert().await?;
		}
		Ok(())
	}
}

pub async fn register(themes: Arc<ThemeManager>, ui: Arc<ActionUi>) {
	let (buttons, mut refresh_rx) = TrackedButtons::new();

	ui.register(ClearScreencast {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
	})
	.await;

	tokio::spawn(async move {
		let mut rendered = Rendered::new();
		let mut theme_updates = themes.subscribe();
		loop {
			tokio::select! {
				result = refresh_rx.changed() => { if result.is_err() { break; } },
				result = theme_updates.changed() => { if result.is_err() { break; } },
			}
			let tracked = buttons.snapshot().await;
			rendered.retain(&tracked);
			for button in tracked {
				let (id, choice, generation) = (button.id, button.value, button.generation);
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == ClearScreencast::UUID => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered.forget(&id);
						continue;
					}
				};
				let key = (choice.clone(), themes.revision());
				if !rendered.needs_render(&id, generation, &key) {
					continue;
				}
				let image = icon::render_pause(&themes.palette(&choice));
				if buttons.may_render(&instance, generation).await {
					match instance.set_image(Some(image), None).await {
						Ok(()) => rendered.mark_rendered(id, generation, key),
						Err(error) => eprintln!("Screencast pause icon update failed: {error}"),
					}
				}
			}
		}
	});
}
