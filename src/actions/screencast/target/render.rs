use std::{sync::Arc, time::Duration};

use openaction::{Action, Instance, OpenActionResult};
use umbriel_ipc::{Output, ScreencastCommand, Window};

use super::{
	super::{
		icon,
		model::{self, Display, Settings},
	},
	TargetAction,
	modes::application,
	state::ButtonState,
};
use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		render_queue::{RenderPriority, RenderQueue},
		titles::wrap_native_title,
	},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::{OutputUpdates, ScreencastUpdates, SessionUpdates, WindowUpdates},
};

pub(super) struct TargetUpdates {
	pub(super) windows: WindowUpdates,
	pub(super) outputs: OutputUpdates,
	pub(super) screencast: ScreencastUpdates,
	pub(super) session: SessionUpdates,
}

const INACTIVE: u16 = 0;
const ACTIVE: u16 = 1;

#[derive(Clone, PartialEq, Eq)]
struct ImageKey {
	theme_revision: u64,
	palette: String,
	display: Display,
	icon_revision: u64,
}

struct TargetSnapshot<'a> {
	windows: Option<&'a [Window]>,
	outputs: &'a [Output],
	command: Option<&'a ScreencastCommand>,
	processes: &'a application::ProcessNames,
}

fn image_key(
	settings: &Settings,
	snapshot: &TargetSnapshot<'_>,
	theme_revision: u64,
	icons: &ApplicationIcons,
) -> ImageKey {
	let display = if model::Mode::from_setting(&settings.mode) == model::Mode::DefinedApplication {
		application::display_for(
			settings,
			snapshot.command,
			snapshot.windows,
			icons,
			snapshot.processes,
		)
	} else {
		model::display_for(
			settings,
			settings.programmed_window.as_ref(),
			snapshot.command,
			snapshot.windows.unwrap_or(&[]),
			snapshot.outputs,
		)
	};
	ImageKey {
		icon_revision: icons.revision(),
		theme_revision,
		palette: settings.palette.clone(),
		display,
	}
}

fn title_key(settings: &Settings, display: &Display, icons: &ApplicationIcons) -> Option<String> {
	settings
		.show_title
		.then(|| target_title(settings, display, icons))
}

fn target_title(settings: &Settings, display: &Display, icons: &ApplicationIcons) -> String {
	match model::Mode::from_setting(&settings.mode) {
		model::Mode::FocusedWindow => match &display.main {
			model::Visual::Window { app_id, .. } => icons
				.display_name_for_app(app_id)
				.unwrap_or_else(|| app_id.clone()),
			_ => "Focused window".into(),
		},
		model::Mode::ProgrammedWindow => settings
			.programmed_window
			.as_ref()
			.map(|window| {
				icons
					.display_name_for_app(&window.app_id)
					.unwrap_or_else(|| window.app_id.clone())
			})
			.unwrap_or_else(|| "Unassigned window".into()),
		model::Mode::DefinedApplication => {
			if settings.application_source == "custom" {
				let name = settings.custom_name.trim();
				if name.is_empty() {
					settings.custom_pattern.trim().to_owned()
				} else {
					name.to_owned()
				}
			} else if let Some(id) = settings.application.strip_prefix("app-id:") {
				icons
					.display_name_for_app(id)
					.unwrap_or_else(|| id.to_owned())
			} else {
				icons
					.desktop_name(&settings.application)
					.unwrap_or_else(|| settings.application.clone())
			}
		}
		model::Mode::SpecificOutput => {
			if settings.output.trim().is_empty() {
				"Output".into()
			} else {
				settings.output.trim().to_owned()
			}
		}
		model::Mode::FollowWindow => "Follow window".into(),
		model::Mode::FollowOutput => "Follow output".into(),
	}
}

async fn update_image(
	instance: &Instance,
	key: &ImageKey,
	icons: Arc<ApplicationIcons>,
	themes: &ThemeManager,
	buttons: &TrackedButtons<ButtonState>,
	generation: u64,
) -> OpenActionResult<bool> {
	let state = if key.display.selected {
		ACTIVE
	} else {
		INACTIVE
	};
	let palette = themes.palette(&key.palette);
	let display = key.display.clone();

	let image = tokio::task::spawn_blocking(move || icon::render(&display, &palette, &icons)).await;
	let image = match image {
		Ok(image) => image,
		Err(error) => {
			eprintln!("Screencast icon worker failed: {error}");
			instance.show_alert().await?;
			return Ok(false);
		}
	};
	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance.set_image(Some(image), Some(state)).await?;
	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance.set_state(state).await?;
	Ok(true)
}

async fn update_title(
	instance: &Instance,
	title: Option<String>,
	buttons: &TrackedButtons<ButtonState>,
	generation: u64,
) -> OpenActionResult<bool> {
	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance.set_title(wrap_native_title(title), None).await?;
	Ok(true)
}

pub(super) fn spawn(
	action: TargetAction,
	updates: TargetUpdates,
	icons: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	refresh_rx: tokio::sync::watch::Receiver<u64>,
) {
	let buttons = action.buttons.clone();
	let pressed = action.pressed.clone();
	tokio::spawn(async move {
		let TargetUpdates {
			mut windows,
			mut outputs,
			mut screencast,
			mut session,
		} = updates;
		let mut rendered_images = Rendered::<ImageKey>::new();
		let mut rendered_titles = Rendered::<Option<String>>::new();
		let mut theme_updates = themes.subscribe();
		let mut icon_updates = icons.subscribe();
		let mut requests = refresh_rx;
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));
		loop {
			let pending = queue.is_pending();
			let flush = tokio::select! {
				_ = queue.sleep(), if pending => {
					queue.fire();
					true
				},
				result = windows.changed() => {
					if result.is_err() { break; }
					let _ = windows.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = outputs.changed() => {
					if result.is_err() { break; }
					let _ = outputs.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = screencast.changed() => {
					if result.is_err() { break; }
					let _ = screencast.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = theme_updates.changed() => {
					if result.is_err() { break; }
					let _ = *theme_updates.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = icon_updates.changed() => {
					if result.is_err() { break; }
					let _ = *icon_updates.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = session.changed() => {
					if result.is_err() { break; }
					let _ = *session.borrow_and_update();
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

			let current_session = *session.borrow();
			let ws = windows.borrow().clone();
			let os = outputs.borrow().clone();
			let sc = screencast.borrow().clone();
			let wins = ws.as_deref().map(Vec::as_slice).unwrap_or(&[]);
			// Reconnects cancel pending presses, but cannot prove a saved window is gone.
			for button in buttons.snapshot().await {
				if button.value.session != current_session {
					pressed.cancel(&button.id).await;
					buttons
						.change(&button.id, |state| state.update_session(current_session))
						.await;
				}
			}
			if let Some(windows) = ws.as_deref() {
				action.invalidate_closed(windows, current_session).await;
			}
			let tracked = buttons.snapshot().await;
			let processes = if tracked
				.iter()
				.any(|button| application::needs_process_info(&button.value.settings))
			{
				let windows = wins.to_vec();
				tokio::task::spawn_blocking(move || application::process_names(&windows))
					.await
					.unwrap_or_default()
			} else {
				application::ProcessNames::new()
			};
			rendered_images.retain(&tracked);
			rendered_titles.retain(&tracked);

			for button in tracked {
				let (id, button, generation) = (button.id, button.value, button.generation);
				let snapshot = TargetSnapshot {
					windows: ws.as_deref().map(Vec::as_slice),
					outputs: os.as_deref().map(Vec::as_slice).unwrap_or(&[]),
					command: sc.as_ref(),
					processes: &processes,
				};
				let image_key = image_key(&button.settings, &snapshot, themes.revision(), &icons);
				let title_key = title_key(&button.settings, &image_key.display, &icons);
				let needs_image = rendered_images.needs_render(&id, generation, &image_key);
				let needs_title = rendered_titles.needs_render(&id, generation, &title_key);
				if !needs_image && !needs_title {
					continue;
				}
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == TargetAction::UUID => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered_images.forget(&id);
						rendered_titles.forget(&id);
						continue;
					}
				};
				if needs_image {
					match update_image(
						&instance,
						&image_key,
						Arc::clone(&icons),
						&themes,
						&buttons,
						generation,
					)
					.await
					{
						Ok(true) => {
							rendered_images.mark_rendered(id.clone(), generation, image_key.clone())
						}
						Ok(false) => continue,
						Err(error) => {
							eprintln!("Screencast image refresh failed: {error}");
							continue;
						}
					}
				}
				if needs_title {
					match update_title(&instance, title_key.clone(), &buttons, generation).await {
						Ok(true) => rendered_titles.mark_rendered(id, generation, title_key),
						Ok(false) => {}
						Err(error) => eprintln!("Screencast title refresh failed: {error}"),
					}
				}
			}
		}
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	fn snapshot<'a>(processes: &'a application::ProcessNames) -> TargetSnapshot<'a> {
		TargetSnapshot {
			windows: Some(&[]),
			outputs: &[],
			command: None,
			processes,
		}
	}

	#[test]
	fn titles_are_opt_in_and_reflect_configured_targets() {
		let icons = ApplicationIcons::with_test_entries(vec![]);
		let processes = application::ProcessNames::new();
		let snapshot = snapshot(&processes);
		let mut settings = Settings {
			mode: "specific_output".into(),
			output: "DP-1".into(),
			..Settings::default()
		};
		let display = image_key(&settings, &snapshot, 0, &icons).display;
		assert_eq!(title_key(&settings, &display, &icons), None);
		settings.show_title = true;
		let display = image_key(&settings, &snapshot, 0, &icons).display;
		assert_eq!(title_key(&settings, &display, &icons), Some("DP-1".into()));
		settings.mode = "defined_application".into();
		settings.application_source = "custom".into();
		settings.custom_name = "Stardew Valley".into();
		let display = image_key(&settings, &snapshot, 0, &icons).display;
		assert_eq!(
			title_key(&settings, &display, &icons),
			Some("Stardew Valley".into())
		);
	}
}
