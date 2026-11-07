//! Active-state and image render watchers. Neither performs workspace commands.
use super::{ACTIVE, INACTIVE, Settings, WorkspaceSwitch, icon, target};
use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		render_queue::{RenderPriority, RenderQueue},
	},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::{WindowUpdates, WorkspaceUpdates},
};
use openaction::{Action, Instance, OpenActionResult};
use std::{
	collections::HashMap,
	sync::{Arc, Weak},
	time::Duration,
};
use umbriel_ipc::{Window, Workspace};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ImageKey {
	theme_revision: u64,
	label: String,
	palette: String,
	show_badge: bool,
	apps: Vec<String>,
	icon_revision: u64,
}

pub(super) fn image_key(
	settings: &Settings,
	workspaces: Option<&[Workspace]>,
	windows: Option<&[Window]>,
	theme_revision: u64,
	icon_revision: u64,
) -> ImageKey {
	let apps = match (workspaces, windows) {
		(Some(ws), Some(wins)) => target::workspace_apps(settings, ws, wins),
		_ => Vec::new(),
	};
	ImageKey {
		theme_revision,
		label: target::display_label(settings),
		palette: settings.palette.clone(),
		show_badge: settings.show_badge,
		apps,
		icon_revision,
	}
}

async fn update_images(
	instance: &Instance,
	key: &ImageKey,
	resolver: Arc<ApplicationIcons>,
	themes: &ThemeManager,
	buttons: &TrackedButtons<Settings>,
	generation: u64,
) -> OpenActionResult<bool> {
	let palette = themes.palette(&key.palette);
	let apps = key.apps.clone();

	let images = match tokio::task::spawn_blocking(move || {
		apps.iter()
			.map(|app| resolver.for_app(app))
			.collect::<Vec<_>>()
	})
	.await
	{
		Ok(images) => images,
		Err(error) => {
			eprintln!("Workspace icon worker failed: {error}");
			return Ok(false);
		}
	};

	for (active, state) in [(false, INACTIVE), (true, ACTIVE)] {
		if !buttons.may_render(instance, generation).await {
			return Ok(false);
		}
		instance
			.set_image(
				Some(icon::render(
					&key.label,
					active,
					key.show_badge,
					&palette,
					&images,
				)),
				Some(state),
			)
			.await?;
	}
	Ok(true)
}

/// Workspace Switch has DisableAutomaticStates in the manifest, so the host
/// cannot toggle the displayed state behind this cache on key release.
/// A replacement using the same context ID must still receive its initial state.
fn already_sent<T>(previous: Option<&(Weak<T>, u16)>, instance: &Arc<T>, state: u16) -> bool {
	previous.is_some_and(|(owner, sent)| {
		*sent == state
			&& owner
				.upgrade()
				.is_some_and(|old| Arc::ptr_eq(&old, instance))
	})
}

async fn update_state(
	instance: &Arc<Instance>,
	settings: &Settings,
	snapshot: Option<&[Workspace]>,
	buttons: &TrackedButtons<Settings>,
	generation: u64,
	last_sent: &mut HashMap<String, (Weak<Instance>, u16)>,
) -> OpenActionResult<()> {
	if !buttons.may_render(instance, generation).await {
		return Ok(());
	}
	// When disconnected there is no trustworthy active state.
	let active = snapshot.is_some_and(|ws| target::target_is_active(settings, ws));
	let state = if active { ACTIVE } else { INACTIVE };
	if already_sent(last_sent.get(&instance.instance_id), instance, state) {
		return Ok(());
	}
	instance.set_state(state).await?;
	// Do not let an obsolete update mark the state of a newer button.
	if buttons.may_render(instance, generation).await {
		last_sent.insert(
			instance.instance_id.clone(),
			(Arc::downgrade(instance), state),
		);
	}
	Ok(())
}

pub(super) fn spawn(
	workspaces: WorkspaceUpdates,
	windows: WindowUpdates,
	resolver: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	buttons: TrackedButtons<Settings>,
	refresh_rx: tokio::sync::watch::Receiver<u64>,
) {
	let mut state_requests = buttons.subscribe();
	// Keep active-state changes responsive even when bitmap rendering is busy.
	// Only the image watcher below sends images to OpenDeck.
	let state_buttons = buttons.clone();
	let mut states = workspaces.clone();
	tokio::spawn(async move {
		let mut last_sent = HashMap::<String, (Weak<Instance>, u16)>::new();
		loop {
			tokio::select! {
				result = states.changed() => { if result.is_err() { break; } },
				result = state_requests.changed() => { if result.is_err() { break; } },
			}
			let snapshot = states.borrow_and_update().clone();
			let _ = *state_requests.borrow_and_update();
			let tracked = state_buttons.snapshot().await;
			last_sent.retain(|id, _| tracked.iter().any(|button| &button.id == id));
			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				if let Some(instance) = openaction::get_instance(id).await
					&& instance.action_uuid == WorkspaceSwitch::UUID
					&& let Err(error) = update_state(
						&instance,
						&settings,
						snapshot.as_deref().map(Vec::as_slice),
						&state_buttons,
						generation,
						&mut last_sent,
					)
					.await
				{
					eprintln!("Workspace state update failed: {error}");
				}
			}
		}
	});

	let mut ws_images = workspaces;
	let mut win_images = windows;
	let mut theme_images = themes.subscribe();
	let mut icon_updates = resolver.subscribe();
	let mut requests = refresh_rx;
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
				result = ws_images.changed() => {
					if result.is_err() { break; }
					let _ = ws_images.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = win_images.changed() => {
					if result.is_err() { break; }
					let _ = win_images.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = theme_images.changed() => {
					if result.is_err() { break; }
					let _ = *theme_images.borrow_and_update();
					queue.request(RenderPriority::Deferred)
				},
				result = icon_updates.changed() => {
					if result.is_err() { break; }
					let _ = *icon_updates.borrow_and_update();
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
			let latest_ws = ws_images.borrow().clone();
			let latest_win = win_images.borrow().clone();
			let ws = latest_ws.as_deref().map(Vec::as_slice);
			let wins = latest_win.as_deref().map(Vec::as_slice);
			let tracked = buttons.snapshot().await;
			rendered.retain(&tracked);

			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == WorkspaceSwitch::UUID => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered.forget(&id);
						continue;
					}
				};
				let key = image_key(&settings, ws, wins, themes.revision(), resolver.revision());
				if rendered.needs_render(&id, generation, &key) {
					match update_images(
						&instance,
						&key,
						Arc::clone(&resolver),
						&themes,
						&buttons,
						generation,
					)
					.await
					{
						Ok(true) => {
							rendered.mark_rendered(id, generation, key);
						}
						Ok(false) => {}
						Err(error) => eprintln!("Workspace image update failed: {error}"),
					}
				}
			}
		}
	});
}

#[cfg(test)]
mod state_tests {
	use super::*;

	#[test]
	fn duplicate_state_is_skipped_but_replacement_or_change_is_not() {
		let first = Arc::new(());
		let replacement = Arc::new(());
		let sent = (Arc::downgrade(&first), ACTIVE);

		assert!(already_sent(Some(&sent), &first, ACTIVE));
		assert!(!already_sent(Some(&sent), &first, INACTIVE));
		assert!(!already_sent(Some(&sent), &replacement, ACTIVE));
		assert!(!already_sent(None, &first, ACTIVE));
		drop(first);
		assert!(!already_sent(Some(&sent), &replacement, ACTIVE));
	}
}
