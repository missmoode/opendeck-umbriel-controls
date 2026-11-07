//! Scratchpad window inventory and artwork/native title updates.
use super::{ScratchpadToggle, Settings, icon, selected_name};
use crate::{
	actions::{
		buttons::{Rendered, TrackedButtons},
		render_queue::{RenderPriority, RenderQueue},
		titles::wrap_native_title,
	},
	icons::application::ApplicationIcons,
	themes::ThemeManager,
	umbriel_observer::WindowUpdates,
};
use openaction::{Action, Instance, OpenActionResult};
use std::{sync::Arc, time::Duration};
use umbriel_ipc::Window;

fn scratchpad_apps(windows: &[Window], name: &str) -> Vec<String> {
	let mut matching: Vec<_> = windows
		.iter()
		.filter(|window| window.scratchpad == name && !window.app_id.is_empty())
		.collect();
	// Umbriel does not promise a useful visual order. Stable IDs avoid
	// reshuffling artwork when unrelated window details change.
	matching.sort_by(|a, b| a.id.cmp(&b.id));
	matching
		.into_iter()
		.take(3)
		.map(|window| window.app_id.clone())
		.collect()
}

async fn update_icon(
	instance: &Instance,
	key: &RenderKey,
	resolver: Arc<ApplicationIcons>,
	themes: &ThemeManager,
	buttons: &TrackedButtons<Settings>,
	generation: u64,
) -> OpenActionResult<bool> {
	let palette = themes.palette(&key.palette);
	let apps = key.apps.clone();

	// Desktop-file parsing, image decoding and SVG rasterising are blocking.
	// Keep them off the Tokio worker threads that process OpenAction events.
	let images = match tokio::task::spawn_blocking(move || {
		apps.iter()
			.map(|app| resolver.for_app(app))
			.collect::<Vec<_>>()
	})
	.await
	{
		Ok(images) => images,
		Err(error) => {
			eprintln!("Scratchpad icon worker failed: {error}");
			return Ok(false);
		}
	};

	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance
		.set_image(Some(icon::render(&palette, &images)), None)
		.await?;
	if !buttons.may_render(instance, generation).await {
		return Ok(false);
	}
	instance
		.set_title(wrap_native_title(key.title.clone()), None)
		.await?;
	Ok(true)
}

#[derive(PartialEq, Eq)]
struct RenderKey {
	palette: String,
	theme_revision: u64,
	apps: Vec<String>,
	title: Option<String>,
	icon_revision: u64,
}

fn render_key(
	settings: &Settings,
	snapshot: Option<&[Window]>,
	theme_revision: u64,
	icon_revision: u64,
) -> RenderKey {
	let name = selected_name(settings).unwrap_or("default");
	RenderKey {
		palette: settings.palette.clone(),
		theme_revision,
		icon_revision,
		title: settings.show_title.then(|| name.to_owned()),
		apps: snapshot
			.map(|w| scratchpad_apps(w, name))
			.unwrap_or_default(),
	}
}

pub(super) fn spawn(
	windows: WindowUpdates,
	resolver: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	buttons: TrackedButtons<Settings>,
	refresh_rx: tokio::sync::watch::Receiver<u64>,
) {
	tokio::spawn(async move {
		let mut updates = windows;
		let mut rendered = Rendered::<RenderKey>::new();
		let mut theme_updates = themes.subscribe();
		let mut icon_updates = resolver.subscribe();
		let mut requests = refresh_rx;
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));

		loop {
			let pending = queue.is_pending();
			let flush = tokio::select! {
				_ = queue.sleep(), if pending => {
					queue.fire();
					true
				},
				result = updates.changed() => {
					if result.is_err() { break; }
					let _ = updates.borrow_and_update();
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
				result = requests.changed() => {
					if result.is_err() { break; }
					let _ = *requests.borrow_and_update();
					queue.request(RenderPriority::Immediate)
				},
			};
			if !flush {
				continue;
			}
			let latest = updates.borrow().clone();
			let tracked = buttons.snapshot().await;
			rendered.retain(&tracked);

			for button in tracked {
				let (id, settings, generation) = (button.id, button.value, button.generation);
				let snapshot = latest.as_deref().map(Vec::as_slice);
				let key = render_key(&settings, snapshot, themes.revision(), resolver.revision());
				if !rendered.needs_render(&id, generation, &key) {
					continue;
				}
				let instance = match openaction::get_instance(id.clone()).await {
					Some(instance) if instance.action_uuid == ScratchpadToggle::UUID => instance,
					_ => {
						buttons.remove_if_generation(&id, generation).await;
						rendered.forget(&id);
						continue;
					}
				};
				match update_icon(
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
					Err(error) => eprintln!("Scratchpad icon update failed: {error}"),
				}
			}
		}
	});
}

#[cfg(test)]
mod title_tests {
	use super::*;
	#[test]
	fn scratchpad_native_title_is_opt_in() {
		let mut settings = Settings::default();
		assert_eq!(render_key(&settings, None, 0, 0).title, None);
		settings.show_title = true;
		assert_eq!(
			render_key(&settings, None, 0, 0).title,
			Some("default".into())
		);
		settings.scratchpad = Some("music".into());
		assert_eq!(
			render_key(&settings, None, 0, 0).title,
			Some("music".into())
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn window(id: &str, app: &str, scratchpad: &str) -> Window {
		Window {
			id: id.into(),
			app_id: app.into(),
			title: String::new(),
			pid: None,
			workspace: String::new(),
			scratchpad: scratchpad.into(),
			active: false,
			output: String::new(),
			visible: false,
		}
	}

	#[test]
	fn limits_to_three_windows_in_stable_order() {
		let windows = vec![
			window("b", "browser", "terminal"),
			window("a", "terminal", "terminal"),
			window("c", "terminal", "terminal"),
			window("d", "music", "terminal"),
			window("x", "chat", "other"),
		];
		assert_eq!(
			scratchpad_apps(&windows, "terminal"),
			vec!["terminal", "browser", "terminal"]
		);
	}

	#[test]
	fn blank_name_means_default_for_membership() {
		let settings = Settings::default();
		let windows = vec![window("a", "terminal", "default")];
		assert_eq!(
			render_key(&settings, Some(&windows), 0, 0).apps,
			vec!["terminal"]
		);
	}
}
