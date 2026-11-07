//! Desktop-entry enumeration and conservative window identity matching.
use freedesktop_desktop_entry::{DesktopEntry, find_app_by_id, unicase::Ascii};
use std::collections::{HashMap, HashSet};

pub(super) fn application_choices(entries: &[DesktopEntry]) -> Vec<(String, String)> {
	let mut seen = HashSet::new();
	let mut choices = entries
		.iter()
		.filter(|entry| seen.insert(entry.id().to_owned()))
		.filter(|entry| {
			entry.type_() == Some("Application") && !entry.hidden() && !entry.no_display()
		})
		.filter_map(|entry| {
			let name = entry.name::<&str>(&[])?;
			Some((entry.id().to_owned(), name.into_owned()))
		})
		.collect::<Vec<_>>();
	choices.sort_by(|a, b| {
		a.1.to_lowercase()
			.cmp(&b.1.to_lowercase())
			.then_with(|| a.0.cmp(&b.0))
	});
	let mut name_counts = HashMap::new();
	for (_, name) in &choices {
		*name_counts.entry(name.to_lowercase()).or_insert(0_usize) += 1;
	}
	for (id, name) in &mut choices {
		if name_counts.get(&name.to_lowercase()).copied().unwrap_or(0) > 1 {
			name.push_str(&format!(" ({id})"));
		}
	}
	choices
}

fn identity_strength(entry: &DesktopEntry, app_id: &str) -> u8 {
	if entry.id().eq_ignore_ascii_case(app_id) {
		2
	} else if entry.matches_wm_class(Ascii::new(app_id)) {
		1
	} else {
		0
	}
}

pub(super) fn matches_desktop_window(
	entries: &[DesktopEntry],
	desktop_id: &str,
	app_id: &str,
) -> bool {
	if app_id.is_empty() {
		return false;
	}
	let Some(selected) = entries.iter().find(|entry| entry.id() == desktop_id) else {
		return false;
	};
	if selected.hidden() {
		return false;
	}
	let strength = identity_strength(selected, app_id);
	if strength == 0 {
		return false;
	}
	let mut seen = HashSet::new();
	!entries.iter().any(|other| {
		seen.insert(other.id())
			&& other.id() != desktop_id
			&& !other.hidden()
			&& identity_strength(other, app_id) >= strength
	})
}

pub(super) fn icon_name_for<'a>(entries: &'a [DesktopEntry], app_id: &str) -> Option<&'a str> {
	find_app_by_id(entries, Ascii::new(app_id)).and_then(DesktopEntry::icon)
}
