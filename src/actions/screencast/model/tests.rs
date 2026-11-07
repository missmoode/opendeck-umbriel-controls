use super::*;
use serde_json::json;

fn settings(mode: &str, output: &str) -> Settings {
	Settings {
		mode: mode.into(),
		output: output.into(),
		..Settings::default()
	}
}

fn output(name: &str, enabled: bool) -> Output {
	serde_json::from_value(json!({
		"name": name, "focused": false, "enabled": enabled
	}))
	.unwrap()
}

fn window(id: &str, app_id: &str, output: &str, visible: bool, active: bool) -> Window {
	Window {
		id: id.into(),
		app_id: app_id.into(),
		title: String::new(),
		pid: None,
		workspace: "ws-1".into(),
		scratchpad: String::new(),
		active,
		output: output.into(),
		visible,
	}
}

#[test]
fn programmed_window_settings_round_trip_and_unassigned_defaults() {
	let basic: Settings = serde_json::from_value(json!({
		"mode": "programmed_window", "palette": "builtin:blue"
	}))
	.unwrap();
	assert_eq!(basic.programmed_window, None);

	let remembered = RememberedWindow {
		id: "win-1".into(),
		app_id: "browser".into(),
	};
	let settings = Settings {
		mode: "programmed_window".into(),
		programmed_window: Some(remembered.clone()),
		..basic
	};
	let encoded = serde_json::to_value(&settings).unwrap();
	assert_eq!(
		encoded["programmed_window"],
		json!({ "id": "win-1", "app_id": "browser" })
	);
	let restored: Settings = serde_json::from_value(encoded).unwrap();
	assert_eq!(restored.programmed_window, Some(remembered));
	assert!(
		serde_json::to_value(Settings::default())
			.unwrap()
			.get("programmed_window")
			.is_none()
	);
}

#[test]
fn defined_application_settings_round_trip() {
	let settings = Settings {
		mode: "defined_application".into(),
		application: "org.example.Editor".into(),
		..Settings::default()
	};
	let stored = serde_json::to_value(&settings).unwrap();
	assert_eq!(stored["application"], "org.example.Editor");
	assert_eq!(stored["mode"], "defined_application");
	let restored: Settings = serde_json::from_value(stored).unwrap();
	assert_eq!(restored.application, "org.example.Editor");
	assert_eq!(Mode::from_setting(&restored.mode), Mode::DefinedApplication);
	assert_eq!(restored.programmed_window, None);
}

#[test]
fn programmed_press_assigns_on_hold_and_selects_only_a_remembered_window_on_tap() {
	let remembered = RememberedWindow {
		id: "w1".into(),
		app_id: "firefox".into(),
	};
	assert_eq!(
		classify_press(PressKind::Hold, Some(&remembered)),
		PressIntent::Assign
	);
	assert_eq!(classify_press(PressKind::Hold, None), PressIntent::Assign);
	assert_eq!(
		classify_press(PressKind::Tap, Some(&remembered)),
		PressIntent::Select(remembered)
	);
	assert_eq!(
		classify_press(PressKind::Tap, None),
		PressIntent::Unassigned
	);
}

#[test]
fn a_remembered_window_must_match_both_id_and_app_id() {
	let same = window("w1", "firefox", "DP-1", true, false);
	let remembered = RememberedWindow::from(&same);
	assert!(is_present(&remembered, std::slice::from_ref(&same)));
	assert!(!is_present(&remembered, &[]));
	assert!(!is_present(
		&remembered,
		&[window("w1", "other", "DP-1", true, false)]
	));
}

#[test]
fn fixed_output_is_selected_only_when_names_match() {
	let command = ScreencastCommand::Output {
		serial: 11,
		output: "DP-1".into(),
	};
	assert!(selected(
		&settings("specific_output", "DP-1"),
		None,
		Some(&command)
	));
	assert!(!selected(
		&settings("specific_output", "DP-2"),
		None,
		Some(&command)
	));
	assert!(!selected(
		&settings("follow_output", ""),
		None,
		Some(&command)
	));
}

#[test]
fn fixed_output_preview_is_stable_when_focus_moves() {
	let s = settings("specific_output", "DP-1");
	let outputs = [output("DP-1", true), output("DP-2", true)];
	let windows = [
		window("a", "browser", "DP-1", true, false),
		window("b", "chat", "DP-2", true, true),
	];
	let display = display_for(&s, None, None, &windows, &outputs);
	assert_eq!(
		display.main,
		Visual::Output {
			name: "DP-1".into(),
			apps: vec!["browser".into()]
		}
	);
	assert_eq!(display.inset, None);
}

#[test]
fn output_inventory_filters_hidden_and_other_monitors_and_deduplicates() {
	let windows = [
		window("1", "browser", "DP-1", true, false),
		window("2", "browser", "DP-1", true, false),
		window("3", "terminal", "DP-1", true, false),
		window("4", "private", "DP-1", false, false),
		window("5", "chat", "DP-2", true, true),
	];
	assert_eq!(output_apps("DP-1", &windows), vec!["browser", "terminal"]);
}

#[test]
fn missing_or_disabled_output_has_unknown_preview() {
	let s = settings("specific_output", "DP-1");
	assert_eq!(
		display_for(&s, None, None, &[], &[]).main,
		Visual::UnknownOutput
	);
	assert_eq!(
		display_for(&s, None, None, &[], &[output("DP-1", false)]).main,
		Visual::UnknownOutput
	);
}

#[test]
fn selected_window_shows_accepted_target_and_inset_previews_focus() {
	let s = Settings {
		primary_target: "current".into(),
		..settings("focused_window", "")
	};
	let windows = [
		window("a", "browser", "DP-1", true, false),
		window("b", "chat", "DP-1", true, true),
	];
	let command = ScreencastCommand::Window {
		serial: 4,
		identifier: "a".into(),
	};
	let display = display_for(&s, None, Some(&command), &windows, &[]);
	assert!(display.selected);
	assert!(!display.accepted_inset);
	assert!(!display.pinned);
	assert_eq!(
		display.main,
		Visual::Window {
			id: "a".into(),
			app_id: "browser".into()
		}
	);
	assert_eq!(
		display.inset,
		Some(Visual::Window {
			id: "b".into(),
			app_id: "chat".into()
		})
	);

	// The preference only swaps the two icons; neither disappears.
	let s = Settings {
		primary_target: "next".into(),
		..s
	};
	let swapped = display_for(&s, None, Some(&command), &windows, &[]);
	assert!(swapped.accepted_inset);
	assert!(!swapped.pinned);
	assert_eq!(swapped.main, display.inset.unwrap());
	assert_eq!(swapped.inset, Some(display.main));
}

#[test]
fn matching_focus_or_missing_accepted_target_has_no_redundant_inset() {
	let s = settings("focused_window", "");
	let windows = [window("a", "browser", "DP-1", true, true)];
	let command = ScreencastCommand::Window {
		serial: 4,
		identifier: "a".into(),
	};
	let display = display_for(&s, None, Some(&command), &windows, &[]);
	assert!(display.selected);
	assert!(!display.accepted_inset);
	assert_eq!(display.inset, None);
	assert!(!display.pinned);
	assert_eq!(display_for(&s, None, None, &windows, &[]).inset, None);
}

#[test]
fn remembered_target_is_only_selected_for_matching_id() {
	let s = settings("programmed_window", "");
	let remembered = RememberedWindow {
		id: "a".into(),
		app_id: "browser".into(),
	};
	let windows = [
		window("a", "browser", "DP-1", true, false),
		window("b", "chat", "DP-1", true, true),
	];
	let accepted = ScreencastCommand::Window {
		serial: 8,
		identifier: "a".into(),
	};
	let other = ScreencastCommand::Window {
		serial: 9,
		identifier: "b".into(),
	};
	let display = display_for(&s, Some(&remembered), Some(&accepted), &windows, &[]);
	assert!(display.selected && display.pinned);
	assert_eq!(
		display.main,
		Visual::Window {
			id: "a".into(),
			app_id: "browser".into()
		}
	);
	assert!(!selected(&s, Some(&remembered), Some(&other)));
	let unassigned = display_for(&s, None, Some(&accepted), &windows, &[]);
	assert_eq!(unassigned.main, Visual::UnknownWindow);
	assert!(!unassigned.pinned && !unassigned.selected);
}

#[test]
fn missing_remembered_window_is_not_replaced_by_focus() {
	let s = settings("programmed_window", "");
	let remembered = RememberedWindow {
		id: "gone".into(),
		app_id: "browser".into(),
	};
	let windows = [window("b", "chat", "DP-1", true, true)];
	let display = display_for(&s, Some(&remembered), None, &windows, &[]);
	assert_eq!(display.main, Visual::UnknownWindow);
	assert!(display.pinned);
}

#[test]
fn follow_and_clear_are_not_fixed_targets() {
	let follow = ScreencastCommand::FollowWindow { serial: 1 };
	let clear = ScreencastCommand::Clear { serial: 2 };
	assert!(selected(
		&settings("follow_window", ""),
		None,
		Some(&follow)
	));
	assert!(!selected(
		&settings("focused_window", ""),
		None,
		Some(&follow)
	));
	assert!(!selected(
		&settings("specific_output", "DP-1"),
		None,
		Some(&clear)
	));
}

#[test]
fn application_settings_use_defaults_and_round_trip() {
	let settings: Settings =
		serde_json::from_str(r#"{"mode":"defined_application","application":"org.example.App"}"#)
			.unwrap();
	assert_eq!(settings.application_source, "installed");
	assert_eq!(settings.custom_field, "app_id");
	assert!(settings.custom_pattern.is_empty());
	assert!(!settings.show_title);
	let custom = Settings {
		application_source: "custom".into(),
		custom_field: "process_name".into(),
		custom_pattern: "game*.exe".into(),
		custom_name: "My Game".into(),
		custom_case_sensitive: true,
		..settings
	};
	let restored: Settings =
		serde_json::from_value(serde_json::to_value(&custom).unwrap()).unwrap();
	assert_eq!(restored.custom_pattern, "game*.exe");
	assert_eq!(restored.custom_name, "My Game");
	assert!(restored.custom_case_sensitive);
}
