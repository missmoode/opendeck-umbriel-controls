//! Matching non-desktop-entry applications to Umbriel windows.
use std::{collections::HashMap, fs, path::Path};

use regex::{Regex, RegexBuilder};
use umbriel_ipc::Window;

use crate::actions::screencast::model::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
	AppId,
	Title,
	Executable,
	ProcessName,
}

impl Field {
	fn from_setting(field: &str) -> Option<Self> {
		match field {
			"app_id" => Some(Self::AppId),
			"title" => Some(Self::Title),
			"executable" => Some(Self::Executable),
			"process_name" => Some(Self::ProcessName),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, Default)]
pub(in crate::actions::screencast::target) struct ProcessIdentity {
	pub(super) executable: Option<String>,
	pub(super) name: Option<String>,
}

pub(in crate::actions::screencast::target) type ProcessNames = HashMap<i64, ProcessIdentity>;

pub(super) fn needs_process_info(settings: &Settings) -> bool {
	settings.application_source == "custom"
		&& matches!(
			settings.custom_field.as_str(),
			"executable" | "process_name"
		)
}

/// Only resolve processes which actually own a window. Missing /proc information
/// is treated as unknown, never as a match.
pub(super) fn process_names(windows: &[Window]) -> ProcessNames {
	let mut names = HashMap::new();
	for pid in windows.iter().filter_map(|w| w.pid).filter(|pid| *pid > 0) {
		names.entry(pid).or_insert_with(|| {
			let root = Path::new("/proc").join(pid.to_string());
			let executable = fs::read_link(root.join("exe")).ok().and_then(|path| {
				path.file_name()
					.map(|name| name.to_string_lossy().into_owned())
			});
			let name = fs::read_to_string(root.join("comm"))
				.ok()
				.map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
				.filter(|s| !s.is_empty());
			ProcessIdentity { executable, name }
		});
	}
	names
}

/// Match full fields. Only `*` and `?` are globs; brackets are literal.
fn compile_glob(pattern: &str, case_sensitive: bool) -> Result<Regex, String> {
	let pattern = pattern.trim();
	if pattern.is_empty() {
		return Err("Custom application pattern is empty".into());
	}
	if pattern.chars().count() > 256 {
		return Err("Custom application pattern is too long".into());
	}
	let mut expression = String::from("^");
	for c in pattern.chars() {
		match c {
			'*' => expression.push_str(".*"),
			'?' => expression.push('.'),
			_ => expression.push_str(&regex::escape(&c.to_string())),
		}
	}
	expression.push('$');
	RegexBuilder::new(&expression)
		.case_insensitive(!case_sensitive)
		.dot_matches_new_line(true)
		.size_limit(1024 * 1024)
		.build()
		.map_err(|error| error.to_string())
}

pub(super) struct CustomMatcher {
	field: Field,
	pattern: Regex,
}

impl CustomMatcher {
	pub(super) fn from_settings(settings: &Settings) -> Result<Self, String> {
		let field = Field::from_setting(&settings.custom_field)
			.ok_or_else(|| "Unknown custom application match field".to_owned())?;
		let pattern = compile_glob(&settings.custom_pattern, settings.custom_case_sensitive)?;
		Ok(Self { field, pattern })
	}

	fn matches(&self, window: &Window, processes: &ProcessNames) -> bool {
		let value = match self.field {
			Field::AppId => Some(window.app_id.as_str()),
			Field::Title => Some(window.title.as_str()),
			Field::Executable => window
				.pid
				.and_then(|pid| processes.get(&pid))
				.and_then(|info| info.executable.as_deref()),
			Field::ProcessName => window
				.pid
				.and_then(|pid| processes.get(&pid))
				.and_then(|info| info.name.as_deref()),
		};
		value.is_some_and(|s| !s.is_empty() && self.pattern.is_match(s))
	}

	pub(super) fn matching_window<'a>(
		&self,
		windows: &'a [Window],
		processes: &ProcessNames,
	) -> Option<&'a Window> {
		windows
			.iter()
			.filter(|window| !window.id.is_empty() && self.matches(window, processes))
			.min_by(|a, b| a.id.cmp(&b.id))
	}

	pub(super) fn matches_window(&self, window: &Window, processes: &ProcessNames) -> bool {
		!window.id.is_empty() && self.matches(window, processes)
	}
}

pub(super) fn cache_key(settings: &Settings) -> String {
	// Separate fields to prevent a collision between different matching rules.
	format!(
		"{}\0{}\0{}",
		settings.custom_field, settings.custom_pattern, settings.custom_case_sensitive
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn window(id: &str, app: &str, title: &str, pid: Option<i64>) -> Window {
		serde_json::from_value(json!({
			"id": id, "app_id": app, "title": title, "pid": pid,
			"workspace": "w", "scratchpad": ""
		}))
		.unwrap()
	}

	#[test]
	fn globs_are_anchored_escaped_and_optionally_case_sensitive() {
		let matcher = compile_glob("steam_app_*", false).unwrap();
		assert!(matcher.is_match("STEAM_APP_1234"));
		assert!(!matcher.is_match("prefix-steam_app_1234"));
		assert!(
			compile_glob("game?.exe", true)
				.unwrap()
				.is_match("game1.exe")
		);
		assert!(
			!compile_glob("game?.exe", true)
				.unwrap()
				.is_match("game1Xexe")
		);
		assert!(compile_glob("a[1]", true).unwrap().is_match("a[1]"));
		assert!(!compile_glob("  ", false).is_ok());
	}

	#[test]
	fn matching_respects_field_and_never_falls_back_to_focus() {
		let windows = [
			window("z", "steam_app_50", "A", None),
			window("b", "steam_app_50", "B", None),
			window("a", "other", "Game 50", None),
		];
		let mut settings = Settings {
			custom_field: "app_id".into(),
			custom_pattern: "steam_app_*".into(),
			..Settings::default()
		};
		let matcher = CustomMatcher::from_settings(&settings).unwrap();
		assert_eq!(
			matcher
				.matching_window(&windows, &ProcessNames::new())
				.unwrap()
				.id,
			"b"
		);
		settings.custom_field = "title".into();
		settings.custom_pattern = "Game *".into();
		let matcher = CustomMatcher::from_settings(&settings).unwrap();
		assert_eq!(
			matcher
				.matching_window(&windows, &ProcessNames::new())
				.unwrap()
				.id,
			"a"
		);
	}

	#[test]
	fn process_matching_uses_requested_property_only() {
		let windows = [
			window("b", "wine", "Game", Some(34)),
			window("a", "wine", "Other", None),
		];
		let processes = HashMap::from([(
			34,
			ProcessIdentity {
				executable: Some("wine64-preloader".into()),
				name: Some("game.exe".into()),
			},
		)]);
		let mut settings = Settings {
			custom_field: "process_name".into(),
			custom_pattern: "game.exe".into(),
			..Settings::default()
		};
		assert_eq!(
			CustomMatcher::from_settings(&settings)
				.unwrap()
				.matching_window(&windows, &processes)
				.unwrap()
				.id,
			"b"
		);
		settings.custom_field = "executable".into();
		assert!(
			CustomMatcher::from_settings(&settings)
				.unwrap()
				.matching_window(&windows, &processes)
				.is_none()
		);
		settings.custom_pattern = "wine*".into();
		assert!(
			CustomMatcher::from_settings(&settings)
				.unwrap()
				.matching_window(&windows, &processes)
				.is_some()
		);
	}
}
