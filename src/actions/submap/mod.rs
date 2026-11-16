//! Set and reset Umbriel keybinding submaps from Stream Deck keys.
mod icon;
mod render;

use std::sync::Arc;

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use umbriel_ipc::Client;

use crate::{
	actions::{buttons::TrackedButtons, ui::ActionUi},
	themes::ThemeManager,
	umbriel_observer::SubmapUpdates,
};

const INACTIVE: u16 = 0;
const ACTIVE: u16 = 1;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct SetSettings {
	path: String,
	show_title: bool,
	palette: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ResetSettings {
	show_title: bool,
	palette: String,
}

#[derive(Clone)]
enum ButtonSettings {
	Set(SetSettings),
	Reset(ResetSettings),
}

impl ButtonSettings {
	fn uuid(&self) -> &'static str {
		match self {
			Self::Set(_) => SetSubmap::UUID,
			Self::Reset(_) => ResetSubmaps::UUID,
		}
	}

	fn palette(&self) -> &str {
		match self {
			Self::Set(settings) => &settings.palette,
			Self::Reset(settings) => &settings.palette,
		}
	}

	fn show_title(&self) -> bool {
		match self {
			Self::Set(settings) => settings.show_title,
			Self::Reset(settings) => settings.show_title,
		}
	}

	fn is_active(&self, current: Option<&str>) -> bool {
		match self {
			// Umbriel only exposes the topmost submap, not the full stack.
			Self::Set(settings) => {
				let target = parse_path(&settings.path)
					.ok()
					.and_then(|path| path.last().copied());
				target.is_some() && target == current
			}
			Self::Reset(_) => current.is_some(),
		}
	}

	fn native_title(&self, current: Option<&str>) -> Option<String> {
		if !self.show_title() {
			return None;
		}
		Some(match self {
			Self::Set(settings) => settings
				.path
				.lines()
				.map(str::trim)
				.rfind(|name| !name.is_empty())
				.unwrap_or("")
				.to_owned(),
			Self::Reset(_) => match current {
				Some("") => "Reset Unnamed".to_owned(),
				Some(name) => format!("Reset {name}"),
				None => "Reset".to_owned(),
			},
		})
	}
}

fn parse_path(path: &str) -> Result<Vec<&str>, &'static str> {
	let names: Vec<_> = path
		.lines()
		.map(str::trim)
		.filter(|name| !name.is_empty())
		.collect();
	if names.is_empty() {
		return Err("Submap path cannot be empty");
	}
	if names.len() > umbriel_ipc::MAX_SUBMAP_DEPTH {
		return Err("Submap path is too deep");
	}
	if names
		.iter()
		.any(|name| !umbriel_ipc::valid_submap_name(name))
	{
		return Err("Submap path contains a reserved or invalid name");
	}
	Ok(names)
}

struct SetSubmap {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<ButtonSettings>,
	operations: Arc<Mutex<()>>,
}

#[async_trait]
impl Action for SetSubmap {
	const UUID: &'static str = crate::actions::action_uuid!("submap-set");
	type Settings = SetSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons
			.put(&instance.instance_id, ButtonSettings::Set(settings.clone()))
			.await;
		self.buttons.retry_initial_icon(instance).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons
			.put(&instance.instance_id, ButtonSettings::Set(settings.clone()))
			.await;
		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons.remove(&instance.instance_id).await;
		Ok(())
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &Value,
	) -> OpenActionResult<()> {
		self.ui.respond_to_request(instance, payload).await?;
		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let path = match parse_path(&settings.path) {
			Ok(path) => path,
			Err(reason) => {
				eprintln!("Invalid submap path: {reason}");
				instance.show_alert().await?;
				return Ok(());
			}
		};
		let result = async {
			// Prevent this plugin's buttons from interleaving stack operations.
			let _guard = self.operations.lock().await;
			let client = Client::from_environment()?;
			client.set_submap_path(&path).await
		}
		.await;
		if let Err(error) = result {
			eprintln!("Set Submap failed: {error}");
			instance.show_alert().await?;
		}
		Ok(())
	}
}

struct ResetSubmaps {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<ButtonSettings>,
	operations: Arc<Mutex<()>>,
}

#[async_trait]
impl Action for ResetSubmaps {
	const UUID: &'static str = crate::actions::action_uuid!("submap-reset");
	type Settings = ResetSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons
			.put(
				&instance.instance_id,
				ButtonSettings::Reset(settings.clone()),
			)
			.await;
		self.buttons.retry_initial_icon(instance).await;
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons
			.put(
				&instance.instance_id,
				ButtonSettings::Reset(settings.clone()),
			)
			.await;
		Ok(())
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.buttons.remove(&instance.instance_id).await;
		Ok(())
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		payload: &Value,
	) -> OpenActionResult<()> {
		self.ui.respond_to_request(instance, payload).await?;
		Ok(())
	}

	async fn key_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let result = async {
			let _guard = self.operations.lock().await;
			let client = Client::from_environment()?;
			client.reset_submaps().await
		}
		.await;
		if let Err(error) = result {
			eprintln!("Reset Submaps failed: {error}");
			instance.show_alert().await?;
		}
		Ok(())
	}
}

pub async fn register(submaps: SubmapUpdates, themes: Arc<ThemeManager>, ui: Arc<ActionUi>) {
	let (buttons, refresh) = TrackedButtons::new();
	let operations = Arc::new(Mutex::new(()));
	ui.register(SetSubmap {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
		operations: Arc::clone(&operations),
	})
	.await;
	ui.register(ResetSubmaps {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
		operations,
	})
	.await;
	render::spawn(submaps, themes, buttons, refresh);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_full_paths_without_empty_lines() {
		assert_eq!(
			parse_path(" gaming\n\n  streaming  \n audio\n").unwrap(),
			vec!["gaming", "streaming", "audio"]
		);
		assert!(parse_path("\n \n").is_err());
		assert!(parse_path("gaming\nreset").is_err());
		assert!(parse_path("gaming\ndisable").is_err());
		assert!(parse_path("gaming\nbroken]name").is_err());
		assert!(parse_path(&"a\n".repeat(umbriel_ipc::MAX_SUBMAP_DEPTH + 1)).is_err());
	}

	#[test]
	fn selected_state_depends_only_on_last_name() {
		let set = ButtonSettings::Set(SetSettings {
			path: "gaming\nstreaming\naudio".to_owned(),
			..SetSettings::default()
		});
		assert!(set.is_active(Some("audio")));
		assert!(!set.is_active(Some("gaming")));
		assert!(!set.is_active(None));
		assert!(!ButtonSettings::Set(SetSettings::default()).is_active(None));
	}

	#[test]
	fn titles_are_optional() {
		let settings = SetSettings {
			path: "gaming\nstreaming".into(),
			..SetSettings::default()
		};
		assert_eq!(
			ButtonSettings::Set(settings.clone()).native_title(None),
			None
		);
		assert_eq!(
			ButtonSettings::Set(SetSettings {
				show_title: true,
				..settings
			})
			.native_title(None),
			Some("streaming".into())
		);
		let reset = ButtonSettings::Reset(ResetSettings {
			show_title: true,
			..ResetSettings::default()
		});
		assert_eq!(
			reset.native_title(Some("streaming")),
			Some("Reset streaming".into())
		);
		assert_eq!(reset.native_title(Some("")), Some("Reset Unnamed".into()));
		assert_eq!(reset.native_title(None), Some("Reset".into()));
	}

	#[test]
	fn reset_is_available_for_any_stack_entry() {
		let reset = ButtonSettings::Reset(ResetSettings::default());
		assert!(reset.is_active(Some("")));
		assert!(reset.is_active(Some("gaming")));
		assert!(!reset.is_active(None));
	}
}
