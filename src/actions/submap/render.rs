//! State and icon/title watchers shared by both submap actions.
use std::{
	collections::HashMap,
	sync::{Arc, Weak},
	time::Duration,
};

use openaction::{Instance, OpenActionResult};

use super::{ACTIVE, ButtonSettings, INACTIVE, icon};
use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		render_queue::{RenderPriority, RenderQueue},
		titles::wrap_native_title,
	},
	themes::ThemeManager,
	umbriel_observer::SubmapUpdates,
};

#[derive(Debug, PartialEq, Eq)]
struct ImageKey {
	reset: bool,
	palette: String,
	theme_revision: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct TitleKey {
	title: Option<String>,
}

fn image_key(settings: &ButtonSettings, revision: u64) -> ImageKey {
	ImageKey {
		reset: matches!(settings, ButtonSettings::Reset(_)),
		palette: settings.palette().to_owned(),
		theme_revision: revision,
	}
}

fn title_key(settings: &ButtonSettings, current: Option<&str>) -> TitleKey {
	TitleKey {
		title: settings.native_title(current),
	}
}

fn current_submap(snapshot: &Option<Option<String>>) -> Option<&str> {
	snapshot.as_ref().and_then(Option::as_deref)
}

async fn update_state(
	instance: &Arc<Instance>,
	settings: &ButtonSettings,
	current: Option<&str>,
	buttons: &TrackedButtons<ButtonSettings>,
	generation: u64,
	last_sent: &mut HashMap<String, (Weak<Instance>, u16)>,
) -> OpenActionResult<()> {
	if !buttons.may_render(instance, generation).await {
		return Ok(());
	}
	let state = if settings.is_active(current) {
		ACTIVE
	} else {
		INACTIVE
	};
	if last_sent
		.get(&instance.instance_id)
		.is_some_and(|(owner, sent)| {
			*sent == state
				&& owner
					.upgrade()
					.is_some_and(|old| Arc::ptr_eq(&old, instance))
		}) {
		return Ok(());
	}
	instance.set_state(state).await?;
	if buttons.may_render(instance, generation).await {
		last_sent.insert(
			instance.instance_id.clone(),
			(Arc::downgrade(instance), state),
		);
	}
	Ok(())
}

async fn update_images(
	instance: &Instance,
	key: &ImageKey,
	themes: &ThemeManager,
	buttons: &TrackedButtons<ButtonSettings>,
	generation: u64,
) -> OpenActionResult<bool> {
	let palette = themes.palette(&key.palette);
	for (active, state) in [(false, INACTIVE), (true, ACTIVE)] {
		if !buttons.may_render(instance, generation).await {
			return Ok(false);
		}
		instance
			.set_image(Some(icon::render(key.reset, active, &palette)), Some(state))
			.await?;
	}
	Ok(true)
}

async fn update_title(
	instance: &Instance,
	key: &TitleKey,
	buttons: &TrackedButtons<ButtonSettings>,
	generation: u64,
) -> OpenActionResult<bool> {
	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance
		.set_title(wrap_native_title(key.title.clone()), None)
		.await?;
	Ok(true)
}

pub(super) fn spawn(
	mut submaps: SubmapUpdates,
	themes: Arc<ThemeManager>,
	buttons: TrackedButtons<ButtonSettings>,
	mut requests: tokio::sync::watch::Receiver<u64>,
) {
	let mut state_submaps = submaps.clone();
	let mut state_requests = buttons.subscribe();
	let state_buttons = buttons.clone();
	tokio::spawn(async move {
		let mut last_sent = HashMap::<String, (Weak<Instance>, u16)>::new();
		loop {
			tokio::select! {
				result = state_submaps.changed() => { if result.is_err() { break; } },
				result = state_requests.changed() => { if result.is_err() { break; } },
			}
			let snapshot = state_submaps.borrow_and_update().clone();
			let _ = *state_requests.borrow_and_update();
			let tracked = state_buttons.snapshot().await;
			last_sent.retain(|id, _| tracked.iter().any(|button| &button.id == id));
			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				if let Some(instance) = openaction::get_instance(id).await
					&& instance.action_uuid == settings.uuid()
					&& let Err(error) = update_state(
						&instance,
						&settings,
						current_submap(&snapshot),
						&state_buttons,
						generation,
						&mut last_sent,
					)
					.await
				{
					eprintln!("Submap state update failed: {error}");
				}
			}
		}
	});

	let mut theme_updates = themes.subscribe();
	tokio::spawn(async move {
		let mut rendered_images = Rendered::<ImageKey>::new();
		let mut rendered_titles = Rendered::<TitleKey>::new();
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));
		loop {
			let pending = queue.is_pending();
			let flush = tokio::select! {
				_ = queue.sleep(), if pending => {
					queue.fire();
					true
				},
				result = submaps.changed() => {
					if result.is_err() { break; }
					let _ = submaps.borrow_and_update();
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
			let snapshot = submaps.borrow().clone();
			let tracked = buttons.snapshot().await;
			rendered_images.retain(&tracked);
			rendered_titles.retain(&tracked);
			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == settings.uuid() => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered_images.forget(&id);
						rendered_titles.forget(&id);
						continue;
					}
				};
				let image = image_key(&settings, themes.revision());
				if rendered_images.needs_render(&id, generation, &image) {
					match update_images(&instance, &image, &themes, &buttons, generation).await {
						Ok(true) => rendered_images.mark_rendered(id.clone(), generation, image),
						Ok(false) => continue,
						Err(error) => {
							eprintln!("Submap icon update failed: {error}");
							continue;
						}
					}
				}
				let title = title_key(&settings, current_submap(&snapshot));
				if rendered_titles.needs_render(&id, generation, &title) {
					match update_title(&instance, &title, &buttons, generation).await {
						Ok(true) => rendered_titles.mark_rendered(id, generation, title),
						Ok(false) => {}
						Err(error) => eprintln!("Submap title update failed: {error}"),
					}
				}
			}
		}
	});
}

#[cfg(test)]
mod tests {
	use super::super::{ResetSettings, SetSettings};
	use super::*;

	#[test]
	fn distinguishes_default_disconnected_and_unnamed_submaps() {
		let disconnected = None;
		let default = Some(None);
		let unnamed = Some(Some(String::new()));
		assert_eq!(current_submap(&disconnected), None);
		assert_eq!(current_submap(&default), None);
		assert_eq!(current_submap(&unnamed), Some(""));
		assert!(ButtonSettings::Reset(ResetSettings::default()).is_active(current_submap(&unnamed)));
	}

	#[test]
	fn title_updates_only_depend_on_active_name_if_enabled() {
		let disabled = ButtonSettings::Reset(ResetSettings::default());
		assert_eq!(
			title_key(&disabled, Some("one")),
			title_key(&disabled, Some("two"))
		);
		let enabled = ButtonSettings::Reset(ResetSettings {
			show_title: true,
			..ResetSettings::default()
		});
		assert_ne!(
			title_key(&enabled, Some("one")),
			title_key(&enabled, Some("two"))
		);
		let target = ButtonSettings::Set(SetSettings::default());
		assert_eq!(
			title_key(&target, Some("one")),
			title_key(&target, Some("two"))
		);
	}

	#[test]
	fn submap_updates_do_not_invalidate_keyboard_artwork() {
		let target = ButtonSettings::Set(SetSettings::default());
		let equivalent = target.clone();
		assert_eq!(image_key(&target, 1), image_key(&equivalent, 1));
		assert_ne!(image_key(&target, 1), image_key(&target, 2));
	}
}
