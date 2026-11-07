//! Shared property-inspector messages for palette-aware actions.
use std::sync::{Arc, RwLock};

use openaction::{
	Action, Instance, OpenActionResult, async_trait,
	global_events::{DidReceiveGlobalSettingsEvent, GlobalEventHandler},
};
use serde_json::{Value, json};

use crate::{icons::palette::BUILTIN_THEMES, themes::ThemeManager};

pub struct ActionUi {
	themes: Arc<ThemeManager>,
	actions: RwLock<Vec<&'static str>>,
}

fn friendly_name(name: &str) -> String {
	name.split(['-', '_'])
		.filter(|word| !word.is_empty())
		.map(|word| {
			let mut chars = word.chars();
			let first = chars.next().unwrap().to_ascii_uppercase();
			format!("{first}{}", chars.as_str())
		})
		.collect::<Vec<_>>()
		.join(" ")
}

fn palette_label(id: &str) -> String {
	friendly_name(id.strip_prefix("builtin:").unwrap_or(id))
}

impl ActionUi {
	pub fn start(themes: Arc<ThemeManager>) -> Arc<Self> {
		let ui = Arc::new(Self {
			themes,
			actions: RwLock::new(Vec::new()),
		});
		let watcher = Arc::clone(&ui);
		let mut updates = ui.themes.subscribe();
		tokio::spawn(async move {
			while updates.changed().await.is_ok() {
				watcher.send_palette_lists().await;
			}
		});
		ui
	}

	pub async fn register<A: Action>(&self, action: A) {
		openaction::register_action(action).await;
		self.actions.write().unwrap().push(A::UUID);
	}

	fn choices(&self, global: bool) -> Vec<Value> {
		let mut items = Vec::new();

		if global {
			items.push(json!({
				"label": format!(
					"Default ({})",
					palette_label(self.themes.package_default())
				),
				"value": "",
			}));
		} else {
			items.push(json!({"label": "Global (default)", "value": ""}));
		}

		items.extend(BUILTIN_THEMES.iter().map(|theme| {
			json!({
				"label": theme.label,
				"value": format!("builtin:{}", theme.id),
			})
		}));

		items.extend(self.themes.theme_names().into_iter().map(|name| {
			json!({
				"label": format!("{} (Custom)", friendly_name(&name)),
				"value": name,
			})
		}));

		items
	}

	pub async fn respond_to_request(
		&self,
		instance: &Instance,
		payload: &Value,
	) -> OpenActionResult<bool> {
		let event = match payload.get("event").and_then(Value::as_str) {
			Some("getPalettes") => "getPalettes",
			Some("getGlobalPalettes") => "getGlobalPalettes",
			_ => return Ok(false),
		};
		instance
			.send_to_property_inspector(json!({
				"event": event, "items": self.choices(event == "getGlobalPalettes"),
			}))
			.await?;
		Ok(true)
	}

	async fn send_palette_lists(&self) {
		let palettes = json!({"event":"getPalettes", "items":self.choices(false)});
		let global_palettes = json!({"event":"getGlobalPalettes", "items":self.choices(true)});
		let actions = self.actions.read().unwrap().clone();
		for uuid in actions {
			for instance in openaction::visible_instances(uuid).await {
				let _ = instance.send_to_property_inspector(palettes.clone()).await;
				let _ = instance
					.send_to_property_inspector(global_palettes.clone())
					.await;
			}
		}
	}
}

pub struct GlobalEvents(pub Arc<ActionUi>);

#[async_trait]
impl GlobalEventHandler for GlobalEvents {
	async fn plugin_ready(&self) -> OpenActionResult<()> {
		openaction::get_global_settings().await
	}

	async fn did_receive_global_settings(
		&self,
		event: DidReceiveGlobalSettingsEvent,
	) -> OpenActionResult<()> {
		let choice = event
			.payload
			.settings
			.get("palette")
			.and_then(Value::as_str)
			.unwrap_or("");
		self.0.themes.set_global(choice);
		Ok(())
	}
}
