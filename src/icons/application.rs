use freedesktop_desktop_entry::{DesktopEntry, desktop_entries, find_app_by_id, unicase::Ascii};
use freedesktop_icons::lookup;
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{Mutex, RwLock},
};
use tokio::sync::watch;

use super::bitmap;
mod catalogue;
#[cfg(test)]
mod tests;
mod theme;
use catalogue::{application_choices, icon_name_for, matches_desktop_window};
use theme::system_icon_theme;

/// Resolved icons are associated with a catalogue revision. A lookup started
/// before a refresh must not repopulate the refreshed cache.
#[derive(Default)]
struct ResolvedCache {
	revision: u64,
	images: HashMap<String, Option<String>>,
}

/// Linux freedesktop app artwork
pub struct ApplicationIcons {
	entries: RwLock<Vec<DesktopEntry>>,
	theme: String,
	cache: Mutex<ResolvedCache>,
	custom_cache: Mutex<HashMap<String, String>>,
	changed: watch::Sender<u64>,
}

impl ApplicationIcons {
	pub fn new() -> Self {
		// Icon theme and lookups are cached. Desktop entries can be rescanned
		// from the application's property-inspector refresh control.
		let locales: &[String] = &[];
		let (changed, _) = watch::channel(0);

		Self {
			entries: RwLock::new(desktop_entries(locales)),
			theme: system_icon_theme(),
			cache: Mutex::new(ResolvedCache::default()),
			custom_cache: Mutex::new(HashMap::new()),
			changed,
		}
	}

	/// Resolve a Wayland app ID into a normalised PNG data URL.
	pub fn for_app(&self, app_id: &str) -> Option<String> {
		let app_id = app_id.trim();
		if app_id.is_empty() {
			return None;
		}
		self.cached_icon(app_id, || self.resolve_uncached(app_id))
	}

	/// Retry if the desktop catalogue changes while a disk or image lookup runs.
	/// The catalogue update and cache revision bump share a lock boundary.
	fn cached_icon(
		&self,
		key: &str,
		mut resolve: impl FnMut() -> Option<String>,
	) -> Option<String> {
		loop {
			let revision = {
				let cache = self.cache.lock().ok()?;
				if let Some(cached) = cache.images.get(key) {
					return cached.clone();
				}
				cache.revision
			};

			let resolved = resolve();
			let mut cache = self.cache.lock().ok()?;
			if cache.revision != revision {
				continue;
			}
			cache.images.insert(key.to_owned(), resolved.clone());
			return resolved;
		}
	}

	fn resolve_uncached(&self, app_id: &str) -> Option<String> {
		let name = self
			.entries
			.read()
			.ok()
			.and_then(|entries| icon_name_for(&entries, app_id).map(str::to_owned))
			.unwrap_or_else(|| app_id.to_owned());

		let path = if Path::new(&name).is_absolute() {
			Some(Path::new(&name).to_path_buf())
		} else {
			self.lookup_icon(&name).or_else(|| self.lookup_icon(app_id))
		}?;

		bitmap::png_data_url(&path)
	}

	#[cfg(test)]
	pub(crate) fn with_test_entries(entries: Vec<DesktopEntry>) -> Self {
		let (changed, _) = watch::channel(0);
		Self {
			entries: RwLock::new(entries),
			theme: "hicolor".into(),
			cache: Mutex::new(ResolvedCache::default()),
			custom_cache: Mutex::new(HashMap::new()),
			changed,
		}
	}

	pub fn subscribe(&self) -> watch::Receiver<u64> {
		self.changed.subscribe()
	}

	pub fn revision(&self) -> u64 {
		*self.changed.borrow()
	}

	#[cfg(test)]
	pub(crate) fn seed_test_icon(&self, app_id: &str) {
		self.cache
			.lock()
			.unwrap()
			.images
			.insert(app_id.to_owned(), Some("data:image/png;base64,AAAA".into()));
	}

	/// Enumerate launchers for the inspector, refreshing the shared catalogue.
	pub fn applications(&self) -> Vec<(String, String)> {
		let locales: &[String] = &[];
		let entries = desktop_entries(locales);
		let choices = application_choices(&entries);
		self.replace_entries(entries);
		choices
	}

	fn replace_entries(&self, entries: Vec<DesktopEntry>) {
		if let Ok(mut stored) = self.entries.write() {
			*stored = entries;
			if let Ok(mut cache) = self.cache.lock() {
				cache.revision = cache.revision.wrapping_add(1);
				cache.images.clear();
			}
			drop(stored);
			self.changed
				.send_modify(|value| *value = value.wrapping_add(1));
		}
	}

	/// Retain a resolved icon for a custom matching rule for this plugin run.
	/// No icon binaries are written to OpenDeck settings.
	pub fn for_custom(&self, key: &str, app_id: Option<&str>) -> Option<String> {
		if let Some(app_id) = app_id
			&& let Some(icon) = self.for_app(app_id)
		{
			if let Ok(mut cache) = self.custom_cache.lock() {
				cache.insert(key.to_owned(), icon.clone());
			}
			return Some(icon);
		}
		self.custom_cache.lock().ok()?.get(key).cloned()
	}

	pub fn display_name_for_app(&self, app_id: &str) -> Option<String> {
		let entries = self.entries.read().ok()?;
		find_app_by_id(&entries, Ascii::new(app_id))?
			.name::<&str>(&[])
			.map(|name| name.into_owned())
	}

	pub fn desktop_name(&self, id: &str) -> Option<String> {
		self.entries
			.read()
			.ok()?
			.iter()
			.find(|entry| entry.id() == id && !entry.hidden())?
			.name::<&str>(&[])
			.map(|name| name.into_owned())
	}

	/// Fetch the configured application's own icon, not a window's guessed identity.
	pub fn for_desktop_id(&self, id: &str) -> Option<String> {
		self.cached_icon(&format!("desktop:{id}"), || {
			let icon = self
				.entries
				.read()
				.ok()?
				.iter()
				.find(|entry| entry.id() == id)
				.filter(|entry| !entry.hidden())
				.and_then(DesktopEntry::icon)
				.map(str::to_owned);
			icon.and_then(|name| {
				let path = if Path::new(&name).is_absolute() {
					Some(PathBuf::from(&name))
				} else {
					self.lookup_icon(&name)
				}?;
				bitmap::png_data_url(&path)
			})
		})
	}

	/// A screencast target must not rely on the icon resolver's fuzzy matches.
	pub fn matches_desktop_window(&self, desktop_id: &str, app_id: &str) -> bool {
		self.entries
			.read()
			.is_ok_and(|entries| matches_desktop_window(&entries, desktop_id, app_id))
	}

	fn lookup_icon(&self, name: &str) -> Option<std::path::PathBuf> {
		lookup(name)
			.with_theme(&self.theme)
			.with_size(64)
			.with_cache()
			.find()
	}
}
