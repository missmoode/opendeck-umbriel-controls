use openaction::{Instance, OpenActionResult};
use serde_json::json;
use umbriel_ipc::Client;

use super::super::super::model::Settings;
use crate::umbriel_observer::OutputUpdates;

pub(in crate::actions::screencast::target) fn is_available(
	settings: &Settings,
	outputs: &OutputUpdates,
) -> bool {
	let output = settings.output.trim();
	if output.is_empty() {
		return false;
	}
	outputs
		.borrow()
		.as_deref()
		.is_none_or(|outputs| outputs.iter().any(|o| o.name == output && o.enabled))
}

pub(in crate::actions::screencast::target) async fn send_choices(
	instance: &Instance,
) -> OpenActionResult<()> {
	let items = match Client::from_environment() {
		Ok(client) => match client.list_outputs().await {
			Ok(outputs) => outputs
				.into_iter()
				.filter(|output| output.enabled)
				.map(|output| json!({"label": output.name, "value": output.name}))
				.collect::<Vec<_>>(),
			Err(error) => {
				eprintln!("Could not list Umbriel outputs: {error}");
				Vec::new()
			}
		},
		Err(error) => {
			eprintln!("Could not connect to Umbriel: {error}");
			Vec::new()
		}
	};
	instance
		.send_to_property_inspector(json!({ "event": "getOutputs", "items": items }))
		.await?;
	Ok(())
}
