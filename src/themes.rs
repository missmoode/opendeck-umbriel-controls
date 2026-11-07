//! Shared palette resolution and hot-reload of user-supplied theme files.
use std::{
	collections::BTreeMap,
	fs,
	io::{self, Read},
	path::{Path, PathBuf},
	sync::{Arc, RwLock},
	time::Duration,
};

use tokio::sync::watch;

use crate::icons::palette::{Palette, builtin_palette, parse_palette};

const PLUGIN_ID: &str = "dev.moode.umbriel-controls.sdPlugin";
const DEFAULT_THEME: &str = "builtin:blue";

struct ThemeState {
	global: String,
	files: BTreeMap<String, Arc<Palette>>,
	revision: u64,
}

pub struct ThemeManager {
	directory: PathBuf,
	package_default: String,
	state: RwLock<ThemeState>,
	changed: watch::Sender<u64>,
}

fn config_dir() -> PathBuf {
	let config = std::env::var_os("XDG_CONFIG_HOME")
		.filter(|path| !path.is_empty())
		.map(PathBuf::from)
		.unwrap_or_else(|| {
			PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
		});
	config.join("opendeck")
}

fn valid_stem(stem: &str) -> bool {
	!stem.is_empty()
		&& stem.len() <= 80
		&& stem
			.bytes()
			.all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

// An existing but unreadable/oversized JSON file is represented by None, so
// its last valid palette remains usable. Absence means the file was deleted.
fn read_files(dir: &Path) -> io::Result<BTreeMap<String, Option<Vec<u8>>>> {
	const MAX_PALETTE_BYTES: u64 = 65536;
	let mut files = BTreeMap::new();
	for entry in fs::read_dir(dir)? {
		let path = entry?.path();
		if path.extension().and_then(|s| s.to_str()) != Some("json") {
			continue;
		}
		let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
			continue;
		};
		if !valid_stem(stem) {
			continue;
		}
		let bytes = match fs::metadata(&path) {
			Ok(meta) if meta.is_file() && meta.len() <= MAX_PALETTE_BYTES => fs::File::open(&path)
				.and_then(|file| {
					let mut bytes = Vec::new();
					file.take(MAX_PALETTE_BYTES + 1).read_to_end(&mut bytes)?;
					Ok(bytes)
				})
				.ok()
				.filter(|bytes| bytes.len() as u64 <= MAX_PALETTE_BYTES),
			_ => None,
		};
		files.insert(stem.to_owned(), bytes);
	}
	Ok(files)
}

impl ThemeManager {
	pub fn start() -> Arc<Self> {
		let directory = std::env::var_os("UMBRIEL_CONTROLS_PALETTES_DIR")
			.filter(|path| !path.is_empty())
			.map(PathBuf::from)
			.unwrap_or_else(|| {
				config_dir()
					.join("settings")
					.join(PLUGIN_ID)
					.join("palettes")
			});
		if let Err(error) = fs::create_dir_all(&directory) {
			eprintln!(
				"Cannot create Umbriel Controls palette directory {}: {error}",
				directory.display()
			);
		}
		let package_default = std::env::var("UMBRIEL_CONTROLS_DEFAULT_PALETTE")
			.unwrap_or_default()
			.trim()
			.to_owned();
		let (changed, _) = watch::channel(0);
		let manager = Arc::new(Self {
			directory,
			package_default,
			state: RwLock::new(ThemeState {
				global: String::new(),
				files: BTreeMap::new(),
				revision: 0,
			}),
			changed,
		});
		let initial_files = read_files(&manager.directory).unwrap_or_default();
		manager.load_files(&initial_files);
		let watcher = Arc::clone(&manager);
		// A directory rescan also catches atomic file replacements and Nix symlink swaps.
		// Keep it dependency-free: a changed scan reparses available files.
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(Duration::from_millis(800));
			let mut previous = initial_files;
			loop {
				interval.tick().await;
				let path = watcher.directory.clone();
				let current = match tokio::task::spawn_blocking(move || read_files(&path)).await {
					Ok(Ok(current)) => current,
					Ok(Err(_)) => continue, // Keep palettes on transient directory failure.
					Err(error) => {
						eprintln!("Palette scan task failed: {error}");
						break;
					}
				};
				if current != previous {
					watcher.load_files(&current);
					previous = current;
				}
			}
		});
		manager
	}

	fn load_files(&self, files: &BTreeMap<String, Option<Vec<u8>>>) {
		let mut state = self.state.write().unwrap();
		let mut loaded = state.files.clone();
		loaded.retain(|id, _| files.contains_key(id));
		for (name, bytes) in files {
			let Some(bytes) = bytes else { continue };
			match parse_palette(bytes) {
				Ok(palette) => {
					loaded.insert(name.clone(), Arc::new(palette));
				}
				Err(error) => {
					eprintln!("Palette {name}.json: {error} (keeping last valid version, if any)")
				}
			}
		}
		if state.files != loaded {
			state.files = loaded;
			state.revision = state.revision.wrapping_add(1);
			self.changed.send_replace(state.revision);
		}
	}

	pub fn set_global(&self, choice: &str) {
		let mut state = self.state.write().unwrap();
		if state.global != choice {
			state.global = choice.to_owned();
			state.revision = state.revision.wrapping_add(1);
			self.changed.send_replace(state.revision);
		}
	}

	pub fn subscribe(&self) -> watch::Receiver<u64> {
		self.changed.subscribe()
	}

	pub fn revision(&self) -> u64 {
		self.state.read().unwrap().revision
	}

	pub fn palette(&self, button_choice: &str) -> Arc<Palette> {
		let state = self.state.read().unwrap();
		let choice = if !button_choice.is_empty() {
			button_choice
		} else if !state.global.is_empty() {
			&state.global
		} else if !self.package_default.is_empty() {
			&self.package_default
		} else {
			DEFAULT_THEME
		};

		let resolve = |choice: &str| {
			if let Some(name) = choice.strip_prefix("builtin:") {
				builtin_palette(name)
			} else {
				state.files.get(choice).cloned()
			}
		};

		resolve(choice)
			.or_else(|| resolve(DEFAULT_THEME))
			.expect("bundled default palette")
	}

	pub fn package_default(&self) -> &str {
		if self.package_default.is_empty() {
			DEFAULT_THEME
		} else {
			&self.package_default
		}
	}

	pub fn theme_names(&self) -> Vec<String> {
		self.state.read().unwrap().files.keys().cloned().collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_manager(package_default: &str) -> ThemeManager {
		let (changed, _) = watch::channel(0);
		ThemeManager {
			directory: PathBuf::new(),
			package_default: package_default.to_owned(),
			state: RwLock::new(ThemeState {
				global: String::new(),
				files: BTreeMap::new(),
				revision: 0,
			}),
			changed,
		}
	}

	#[test]
	fn button_and_global_preferences_override_the_package_default() {
		let manager = sample_manager("builtin:gold");
		assert_eq!(
			manager.palette("").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);
		manager.set_global("builtin:green");
		assert_eq!(
			manager.palette("").muted.fill,
			builtin_palette("green").unwrap().muted.fill
		);
		assert_eq!(
			manager.palette("builtin:red").muted.fill,
			builtin_palette("red").unwrap().muted.fill
		);
		manager.set_global("");
		assert_eq!(
			manager.palette("").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);
	}

	#[test]
	fn scanned_palette_keeps_last_valid_file_until_deleted() {
		let directory = tempfile::tempdir().unwrap();
		let file = directory.path().join("custom.json");
		let manager = sample_manager("builtin:blue");
		fs::write(&file, include_bytes!("icons/palettes/gold.json")).unwrap();
		manager.load_files(&read_files(directory.path()).unwrap());
		assert_eq!(manager.theme_names(), vec!["custom"]);
		assert_eq!(
			manager.palette("custom").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);
		let revision = manager.revision();

		fs::write(&file, "{").unwrap();
		manager.load_files(&read_files(directory.path()).unwrap());
		assert_eq!(manager.revision(), revision);
		assert_eq!(
			manager.palette("custom").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);

		fs::write(&file, vec![b' '; 65537]).unwrap();
		manager.load_files(&read_files(directory.path()).unwrap());
		assert_eq!(manager.revision(), revision);
		assert_eq!(
			manager.palette("custom").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);

		fs::remove_file(&file).unwrap();
		manager.load_files(&read_files(directory.path()).unwrap());
		assert_eq!(
			manager.palette("custom").muted.fill,
			builtin_palette("blue").unwrap().muted.fill
		);
		assert!(manager.theme_names().is_empty());
	}

	#[test]
	fn custom_palette_can_share_name_with_builtin() {
		let manager = sample_manager("builtin:blue");
		let mut files = BTreeMap::new();
		files.insert(
			"blue".to_owned(),
			Some(include_bytes!("icons/palettes/gold.json").to_vec()),
		);
		manager.load_files(&files);

		assert_eq!(
			manager.palette("blue").muted.fill,
			builtin_palette("gold").unwrap().muted.fill
		);
		assert_eq!(
			manager.palette("builtin:blue").muted.fill,
			builtin_palette("blue").unwrap().muted.fill
		);
	}

	#[test]
	fn file_names_are_restricted() {
		assert!(valid_stem("my-theme_2"));
		assert!(!valid_stem("../theme"));
		assert!(!valid_stem("builtin:blue"));
	}
}
