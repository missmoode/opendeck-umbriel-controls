use crate::actions::ui::ActionUi;
use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
	request: String,
}

struct GenericIpcAction {
	ui: Arc<ActionUi>,
}

#[async_trait]
impl Action for GenericIpcAction {
	const UUID: &'static str = "dev.moode.umbriel-controls.ipc";

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

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		// An unconfigured button shouldn't run anything.
		if settings.request.trim().is_empty() {
			instance.show_alert().await?;
			return Ok(());
		}

		let request: serde_json::Value = match serde_json::from_str(&settings.request) {
			Ok(value) => value,
			Err(error) => {
				eprintln!("Invalid IPC request: {error}");
				instance.show_alert().await?;
				return Ok(());
			}
		};

		if let Err(error) = umbriel_ipc::send(&request).await {
			eprintln!("Umbriel IPC failed: {error}");
			instance.show_alert().await?;
		}

		Ok(())
	}
}

pub async fn register(ui: Arc<ActionUi>) {
	ui.register(GenericIpcAction {
		ui: Arc::clone(&ui),
	})
	.await;
}
