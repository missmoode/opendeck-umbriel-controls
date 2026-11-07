//! Property-inspector application inventory and conversion to editable matchers.

use std::{collections::BTreeMap, sync::Arc};

use openaction::{Instance, OpenActionResult};
use serde_json::json;
use umbriel_ipc::{Client, Window};

use super::RUNNING_PREFIX;
use crate::icons::application::ApplicationIcons;

/// Convert a currently selected desktop entry or exact running app ID into an
/// editable custom matcher. Prefer the app ID reported by an actual window;
/// without a running window, a desktop ID is the portable Wayland fallback.
pub(in crate::actions::screencast::target) async fn send_conversion(
	instance: &Instance,
	apps: Arc<ApplicationIcons>,
	selected: &str,
) -> OpenActionResult<()> {
	let windows = match Client::from_environment() {
		Ok(client) => client.list_windows().await.unwrap_or_default(),
		Err(_) => Vec::new(),
	};
	let selected = selected.to_owned();
	let conversion =
		match tokio::task::spawn_blocking(move || conversion_for(&apps, &selected, &windows)).await
		{
			Ok(conversion) => conversion,
			Err(error) => {
				eprintln!("Could not prepare application conversion: {error}");
				None
			}
		};
	instance
		.send_to_property_inspector(json!({
			"event": "getApplicationConversion", "conversion": conversion,
		}))
		.await?;
	Ok(())
}

pub(super) fn conversion_for(
	apps: &ApplicationIcons,
	selected: &str,
	windows: &[Window],
) -> Option<serde_json::Value> {
	let selected = selected.trim();
	if selected.is_empty() {
		return None;
	}
	let running_id = selected.strip_prefix(RUNNING_PREFIX);
	let matched = windows
		.iter()
		.filter(|window| {
			!window.app_id.is_empty()
				&& running_id.map_or_else(
					|| apps.matches_desktop_window(selected, &window.app_id),
					|id| window.app_id == id,
				)
		})
		.min_by(|a, b| a.id.cmp(&b.id));
	let pattern = matched
		.map(|window| window.app_id.as_str())
		.or(running_id)
		.unwrap_or(selected);
	if pattern.is_empty() {
		return None;
	}
	let name = if running_id.is_some() {
		matched
			.map(|window| window.title.trim())
			.filter(|name| !name.is_empty())
			.map(str::to_owned)
	} else {
		apps.desktop_name(selected)
	}
	.unwrap_or_else(|| pattern.to_owned());
	Some(json!({ "name": name, "pattern": pattern }))
}

/// One grouped selector: desktop apps that are currently running appear once
/// at the top; windows without a known desktop entry get an exact app-ID choice.
pub(in crate::actions::screencast::target) async fn send_choices(
	instance: &Instance,
	apps: Arc<ApplicationIcons>,
	selected_id: &str,
) -> OpenActionResult<()> {
	let catalogue = Arc::clone(&apps);
	let choices = match tokio::task::spawn_blocking(move || catalogue.applications()).await {
		Ok(choices) => choices,
		Err(error) => {
			eprintln!("Could not enumerate desktop applications: {error}");
			Vec::new()
		}
	};
	let windows = match Client::from_environment() {
		Ok(client) => match client.list_windows().await {
			Ok(windows) => windows,
			Err(error) => {
				eprintln!("Could not list running applications for inspector: {error}");
				Vec::new()
			}
		},
		Err(error) => {
			eprintln!("Could not connect to Umbriel for application choices: {error}");
			Vec::new()
		}
	};
	let selected = selected_id.to_owned();
	let items = match tokio::task::spawn_blocking(move || {
		grouped_choices(&apps, &choices, &windows, &selected)
	})
	.await
	{
		Ok(items) => items,
		Err(error) => {
			eprintln!("Could not build application choices: {error}");
			Vec::new()
		}
	};
	instance
		.send_to_property_inspector(json!({ "event": "getApplications", "items": items }))
		.await?;
	Ok(())
}

pub(super) fn grouped_choices(
	apps: &ApplicationIcons,
	installed: &[(String, String)],
	windows: &[Window],
	selected: &str,
) -> Vec<serde_json::Value> {
	let mut running = BTreeMap::<String, String>::new();
	let mut app_ids = BTreeMap::<String, (&str, &str)>::new();
	for window in windows
		.iter()
		.filter(|window| !window.id.is_empty() && !window.app_id.is_empty())
	{
		let entry = app_ids
			.entry(window.app_id.clone())
			.or_insert((&window.id, &window.title));
		// Keep the display label stable if the compositor returns windows in
		// a different order on successive queries.
		if window.id.as_str() < entry.0 {
			*entry = (&window.id, &window.title);
		}
	}
	for (app_id, (_, title)) in app_ids {
		if let Some((desktop_id, name)) = installed
			.iter()
			.find(|(id, _)| apps.matches_desktop_window(id, &app_id))
		{
			running.insert(desktop_id.clone(), name.clone());
		} else {
			let title = title.trim();
			let label = if title.is_empty() || title == app_id {
				app_id.clone()
			} else {
				format!("{title} ({app_id})")
			};
			running.insert(format!("{RUNNING_PREFIX}{app_id}"), label);
		}
	}
	let mut running = running.into_iter().collect::<Vec<_>>();
	running.sort_by(|a, b| {
		a.1.to_lowercase()
			.cmp(&b.1.to_lowercase())
			.then_with(|| a.0.cmp(&b.0))
	});
	let installed = installed
		.iter()
		.filter(|(id, _)| !running.iter().any(|(key, _)| key == id))
		.cloned()
		.collect::<Vec<_>>();
	let mut result = Vec::new();
	if !selected.is_empty()
		&& !running
			.iter()
			.chain(installed.iter())
			.any(|(id, _)| id == selected)
	{
		result.push(json!({ "label": "Saved selection", "children": [{
			"label": format!(
				"{} (unavailable)",
				selected.strip_prefix(RUNNING_PREFIX).unwrap_or(selected)
			),
			"value": selected,
		}]}));
	}
	if !running.is_empty() {
		result.push(json!({
			"label": "Running applications",
			"children": choices_json(running),
		}));
	}
	if !installed.is_empty() {
		result.push(json!({
			"label": "Other installed applications",
			"children": choices_json(installed),
		}));
	}
	result
}

fn choices_json(choices: Vec<(String, String)>) -> Vec<serde_json::Value> {
	choices
		.into_iter()
		.map(|(value, label)| json!({ "label": label, "value": value }))
		.collect()
}
