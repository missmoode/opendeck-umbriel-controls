use std::time::Duration;

use serde::{Deserialize, Serialize};
use umbriel_ipc::{Output, ScreencastCommand, Window};

use crate::actions::press::PressKind;

/// A held programmed-window key assigns its focused window.
pub(super) const HOLD_THRESHOLD: Duration = Duration::from_millis(900);

/// Identity of one concrete window, not a reusable application preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedWindow {
	pub id: String,
	pub app_id: String,
}

impl From<&Window> for RememberedWindow {
	fn from(window: &Window) -> Self {
		Self {
			id: window.id.clone(),
			app_id: window.app_id.clone(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressIntent {
	Assign,
	Select(RememberedWindow),
	Unassigned,
}

/// Only the action layer performs IPC or changes state.
pub fn classify_press(kind: PressKind, remembered: Option<&RememberedWindow>) -> PressIntent {
	match kind {
		PressKind::Hold => PressIntent::Assign,
		PressKind::Tap => remembered
			.cloned()
			.map(PressIntent::Select)
			.unwrap_or(PressIntent::Unassigned),
	}
}

/// Do not send a stale ID (or one reused for a different application).
/// This is a best-effort check; IDs are only valid for a running Umbriel session.
pub fn is_present(target: &RememberedWindow, windows: &[Window]) -> bool {
	windows
		.iter()
		.any(|window| window.id == target.id && window.app_id == target.app_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	FocusedWindow,
	ProgrammedWindow,
	DefinedApplication,
	SpecificOutput,
	FollowWindow,
	FollowOutput,
}

impl Mode {
	pub fn from_setting(value: &str) -> Self {
		match value {
			"programmed_window" => Self::ProgrammedWindow,
			"defined_application" => Self::DefinedApplication,
			"specific_output" => Self::SpecificOutput,
			"follow_window" => Self::FollowWindow,
			"follow_output" => Self::FollowOutput,
			_ => Self::FocusedWindow,
		}
	}
}

fn default_mode() -> String {
	"focused_window".into()
}
fn default_primary_target() -> String {
	"next".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
	pub mode: String,
	pub output: String,
	pub application: String,
	pub application_source: String,
	pub custom_field: String,
	pub custom_pattern: String,
	pub custom_name: String,
	pub custom_case_sensitive: bool,
	pub show_title: bool,
	pub palette: String,
	pub primary_target: String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub programmed_window: Option<RememberedWindow>,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			mode: default_mode(),
			output: String::new(),
			application: String::new(),
			application_source: "installed".into(),
			custom_field: "app_id".into(),
			custom_pattern: String::new(),
			custom_name: String::new(),
			custom_case_sensitive: false,
			show_title: false,
			palette: String::new(),
			primary_target: default_primary_target(),
			programmed_window: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visual {
	Window {
		id: String,
		app_id: String,
	},
	Application {
		desktop_id: String,
		running: Option<bool>,
	},
	CustomApplication {
		key: String,
		app_id: Option<String>,
		running: Option<bool>,
	},
	Output {
		name: String,
		apps: Vec<String>,
	},
	FollowWindow,
	FollowOutput,
	UnknownWindow,
	UnknownOutput,
}

/// The primary icon shows the configured target, or the accepted target when
/// requested. When they differ, the other target remains visible in the inset.
/// Selected represents the accepted command, not an active portal session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
	pub selected: bool,
	/// True when the accepted target is displayed in the inset.
	pub accepted_inset: bool,
	pub pinned: bool,
	pub main: Visual,
	pub inset: Option<Visual>,
}

fn window_visual(window: &Window) -> Visual {
	Visual::Window {
		id: window.id.clone(),
		app_id: window.app_id.clone(),
	}
}

fn focused_window(windows: &[Window]) -> Option<&Window> {
	windows.iter().find(|window| window.active)
}

pub fn output_apps(name: &str, windows: &[Window]) -> Vec<String> {
	let mut apps: Vec<String> = windows
		.iter()
		.filter(|window| window.output == name && window.visible)
		.filter(|window| !window.app_id.is_empty())
		.map(|window| window.app_id.clone())
		.collect();
	apps.sort();
	apps.dedup();
	apps.truncate(4);
	apps
}

fn output_visual(name: &str, outputs: &[Output], windows: &[Window]) -> Visual {
	if !outputs
		.iter()
		.any(|output| output.name == name && output.enabled)
	{
		return Visual::UnknownOutput;
	}
	Visual::Output {
		name: name.to_owned(),
		apps: output_apps(name, windows),
	}
}

fn remembered_visual(remembered: Option<&RememberedWindow>, windows: &[Window]) -> Visual {
	let Some(remembered) = remembered else {
		return Visual::UnknownWindow;
	};
	windows
		.iter()
		.find(|w| w.id == remembered.id && w.app_id == remembered.app_id)
		.map(window_visual)
		.unwrap_or(Visual::UnknownWindow)
}

fn proposed_visual(
	settings: &Settings,
	remembered: Option<&RememberedWindow>,
	windows: &[Window],
	outputs: &[Output],
) -> Visual {
	match Mode::from_setting(&settings.mode) {
		Mode::FocusedWindow => focused_window(windows)
			.map(window_visual)
			.unwrap_or(Visual::UnknownWindow),
		Mode::ProgrammedWindow => remembered_visual(remembered, windows),
		Mode::DefinedApplication => Visual::Application {
			desktop_id: settings.application.clone(),
			running: Some(false),
		},
		Mode::SpecificOutput => {
			let name = settings.output.trim();
			if name.is_empty() {
				Visual::UnknownOutput
			} else {
				output_visual(name, outputs, windows)
			}
		}
		Mode::FollowWindow => Visual::FollowWindow,
		Mode::FollowOutput => Visual::FollowOutput,
	}
}

pub fn selected(
	settings: &Settings,
	remembered: Option<&RememberedWindow>,
	current: Option<&ScreencastCommand>,
) -> bool {
	let Some(current) = current else { return false };
	match (Mode::from_setting(&settings.mode), current) {
		(Mode::FocusedWindow, ScreencastCommand::Window { .. }) => true,
		(Mode::ProgrammedWindow, ScreencastCommand::Window { identifier, .. }) => {
			remembered.is_some_and(|window| window.id == *identifier)
		}
		(Mode::SpecificOutput, ScreencastCommand::Output { output, .. }) => {
			!settings.output.trim().is_empty() && settings.output.trim() == output
		}
		(Mode::FollowWindow, ScreencastCommand::FollowWindow { .. }) => true,
		(Mode::FollowOutput, ScreencastCommand::FollowOutput { .. }) => true,
		// Defined Application needs the installed-entry catalogue and is
		// resolved in target/modes/application.rs.
		_ => false,
	}
}

fn current_window_visual(
	current: Option<&ScreencastCommand>,
	windows: &[Window],
) -> Option<Visual> {
	let Some(ScreencastCommand::Window { identifier, .. }) = current else {
		return None;
	};
	Some(
		windows
			.iter()
			.find(|window| window.id == *identifier)
			.map(window_visual)
			.unwrap_or(Visual::UnknownWindow),
	)
}

pub fn display_for(
	settings: &Settings,
	remembered: Option<&RememberedWindow>,
	current: Option<&ScreencastCommand>,
	windows: &[Window],
	outputs: &[Output],
) -> Display {
	let mode = Mode::from_setting(&settings.mode);
	let proposed = proposed_visual(settings, remembered, windows, outputs);
	let selected = selected(settings, remembered, current);
	// A focused-window command can remain accepted after focus changes, even
	// while no portal share is running. Display both targets without implying
	// that either is actively being captured.
	let accepted = (mode == Mode::FocusedWindow && selected)
		.then(|| current_window_visual(current, windows))
		.flatten()
		.filter(|visual| visual != &proposed);
	let accepted_inset = accepted.is_some() && settings.primary_target == "next";
	let (main, inset) = if let Some(accepted) = accepted {
		if !accepted_inset {
			(accepted, Some(proposed))
		} else {
			(proposed, Some(accepted))
		}
	} else {
		(proposed, None)
	};

	Display {
		selected,
		accepted_inset,
		// Only a deliberately programmed key is pinned. An accepted focused
		// window is not locked: pressing the key can select a new window.
		pinned: mode == Mode::ProgrammedWindow && remembered.is_some(),
		main,
		inset,
	}
}

#[cfg(test)]
mod tests;
