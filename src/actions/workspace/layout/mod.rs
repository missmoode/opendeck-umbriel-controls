use std::{sync::Arc, time::Duration};

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use umbriel_ipc::{Client, Workspace};

use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		render_queue::{RenderPriority, RenderQueue},
		titles::wrap_native_title,
		ui::ActionUi,
	},
	themes::ThemeManager,
	umbriel_observer::WorkspaceUpdates,
};

mod icon;

const MODES: [&str; 4] = ["scrolling", "dwindle", "master", "stacking"];

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
struct Settings {
	palette: String,
	/// Empty = let Umbriel choose its default cycle.
	cycle: Vec<String>,
	show_title: bool,
}

fn valid_cycle(cycle: &[String]) -> bool {
	if cycle.is_empty() {
		return true;
	}
	if !(2..=MODES.len()).contains(&cycle.len()) {
		return false;
	}
	cycle
		.iter()
		.enumerate()
		.all(|(index, mode)| MODES.contains(&mode.as_str()) && !cycle[..index].contains(mode))
}

fn focused_layout(snapshot: Option<&[Workspace]>) -> &str {
	snapshot
		.and_then(|workspaces| workspaces.iter().find(|workspace| workspace.focused))
		.map_or("", |workspace| workspace.layout.as_str())
}

#[derive(PartialEq, Eq)]
struct ImageKey {
	layout: String,
	palette: String,
	theme_revision: u64,
	show_title: bool,
}

struct WorkspaceLayout {
	ui: Arc<ActionUi>,
	buttons: TrackedButtons<Settings>,
}

impl WorkspaceLayout {
	async fn record_settings(&self, instance: &Instance, settings: &Settings) {
		self.buttons
			.put(&instance.instance_id, settings.clone())
			.await;
	}
}

#[async_trait]
impl Action for WorkspaceLayout {
	const UUID: &'static str = "dev.moode.umbriel-controls.workspace-layout";
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

	async fn key_down(&self, instance: &Instance, settings: &Settings) -> OpenActionResult<()> {
		if !valid_cycle(&settings.cycle) {
			eprintln!("Invalid saved layout cycle: {:?}", settings.cycle);
			instance.show_alert().await?;
			return Ok(());
		}
		let result = match Client::from_environment() {
			Ok(client) => client.cycle_workspace_layout(&settings.cycle).await,
			Err(error) => Err(error),
		};
		if let Err(error) = result {
			eprintln!("Workspace layout cycle failed: {error}");
			instance.show_alert().await?;
		}
		Ok(())
	}
}

pub async fn register(workspaces: WorkspaceUpdates, themes: Arc<ThemeManager>, ui: Arc<ActionUi>) {
	let (buttons, mut requests) = TrackedButtons::new();
	ui.register(WorkspaceLayout {
		ui: Arc::clone(&ui),
		buttons: buttons.clone(),
	})
	.await;

	let mut snapshots = workspaces;
	let mut theme_updates = themes.subscribe();
	tokio::spawn(async move {
		let mut rendered = Rendered::<ImageKey>::new();
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));
		loop {
			let pending = queue.is_pending();
			let flush = tokio::select! {
				_ = queue.sleep(), if pending => {
					queue.fire();
					true
				},
				result = snapshots.changed() => {
					if result.is_err() { break; }
					let _ = snapshots.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = theme_updates.changed() => {
					if result.is_err() { break; }
					let _ = *theme_updates.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = requests.changed() => {
					if result.is_err() { break; }
					let _ = *requests.borrow_and_update();
					queue.request(RenderPriority::Immediate)
				},
			};
			if !flush {
				continue;
			}
			let snapshot = snapshots.borrow().clone();
			let layout = focused_layout(snapshot.as_deref().map(Vec::as_slice));
			let tracked = buttons.snapshot().await;
			rendered.retain(&tracked);

			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == WorkspaceLayout::UUID => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered.forget(&id);
						continue;
					}
				};
				let key = ImageKey {
					layout: layout.to_owned(),
					palette: settings.palette.clone(),
					theme_revision: themes.revision(),
					show_title: settings.show_title,
				};
				if !rendered.needs_render(&id, generation, &key) {
					continue;
				}
				let image = icon::render(layout, &themes.palette(&settings.palette));
				if buttons.may_render(&instance, generation).await {
					match instance.set_image(Some(image), None).await {
						Ok(()) => {
							if !buttons.may_render(&instance, generation).await {
								continue;
							}
							let title = settings.show_title.then(|| {
								match layout {
									"scrolling" => "Scrolling",
									"dwindle" => "Dwindle",
									"master" => "Master",
									"stacking" => "Stacking",
									_ => "Unknown",
								}
								.to_owned()
							});
							match instance.set_title(wrap_native_title(title), None).await {
								Ok(()) => rendered.mark_rendered(id, generation, key),
								Err(error) => {
									eprintln!("Workspace layout title update failed: {error}")
								}
							}
						}
						Err(error) => eprintln!("Workspace layout icon update failed: {error}"),
					}
				}
			}
		}
	});
}

#[cfg(test)]
mod title_tests {
	use super::*;

	#[test]
	fn automatic_title_defaults_off() {
		assert!(!Settings::default().show_title);
	}
}
