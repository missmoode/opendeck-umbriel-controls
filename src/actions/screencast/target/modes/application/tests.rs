use super::picker::grouped_choices;
use super::*;
use freedesktop_desktop_entry::DesktopEntry;
use serde_json::json;
use std::path::Path;

fn window(id: &str, app_id: &str, active: bool) -> Window {
	serde_json::from_value(json!({
		"id": id, "app_id": app_id, "workspace": "a", "scratchpad": "",
		"active": active, "visible": false
	}))
	.unwrap()
}

fn application(dir: &Path, id: &str, wm_class: Option<&str>) -> DesktopEntry {
	let path = dir.join(format!("{id}.desktop"));
	let wm = wm_class.map_or(String::new(), |class| format!("StartupWMClass={class}\n"));
	std::fs::write(
		&path,
		format!("[Desktop Entry]\nType=Application\nName=Sample\n{wm}"),
	)
	.unwrap();
	DesktopEntry::from_path(&path, Some(&[] as &[&str])).unwrap()
}

#[test]
fn app_target_is_stable_without_a_pin_or_inset() {
	let dir = tempfile::tempdir().unwrap();
	let apps = ApplicationIcons::with_test_entries(vec![application(
		dir.path(),
		"org.example.Editor",
		Some("editor-window"),
	)]);
	let settings = Settings {
		mode: "defined_application".into(),
		application: "org.example.Editor".into(),
		..Settings::default()
	};
	let windows = [
		window("z", "editor-window", false),
		window("b", "org.example.Editor", false),
		window("a", "other-app", true),
	];
	// All windows are on inactive workspaces; focus must not affect matching.
	assert_eq!(
		matching_window(&apps, &settings, &windows, &ProcessNames::new())
			.unwrap()
			.unwrap()
			.id,
		"b"
	);
	let selected = ScreencastCommand::Window {
		serial: 1,
		identifier: "z".into(),
	};
	let display = display_for(
		&settings,
		Some(&selected),
		Some(&windows),
		&apps,
		&ProcessNames::new(),
	);
	assert!(display.selected);
	assert!(!display.pinned && !display.accepted_inset && display.inset.is_none());
	assert_eq!(
		display.main,
		Visual::Application {
			desktop_id: "org.example.Editor".into(),
			running: Some(true)
		}
	);
	let unrelated = ScreencastCommand::Window {
		serial: 2,
		identifier: "a".into(),
	};
	let display = display_for(
		&settings,
		Some(&unrelated),
		Some(&windows),
		&apps,
		&ProcessNames::new(),
	);
	assert!(!display.selected);
	assert_eq!(
		display.main,
		Visual::Application {
			desktop_id: "org.example.Editor".into(),
			running: Some(true)
		}
	);
}

#[test]
fn a_missing_application_is_never_replaced_by_the_focused_window() {
	let apps = ApplicationIcons::with_test_entries(vec![]);
	let settings = Settings {
		mode: "defined_application".into(),
		application: "org.example.Gone".into(),
		..Settings::default()
	};
	let windows = [window("a", "something-else", true)];
	assert!(
		matching_window(&apps, &settings, &windows, &ProcessNames::new())
			.unwrap()
			.is_none()
	);
	let display = display_for(&settings, None, Some(&windows), &apps, &ProcessNames::new());
	assert!(!display.selected && !display.pinned && display.inset.is_none());
	assert_eq!(
		display.main,
		Visual::Application {
			desktop_id: "org.example.Gone".into(),
			running: Some(false)
		}
	);
}
#[test]
fn custom_application_has_no_pin_or_inset_and_disconnect_does_not_mark_absent() {
	let apps = ApplicationIcons::with_test_entries(vec![]);
	let settings = Settings {
		mode: "defined_application".into(),
		application_source: "custom".into(),
		custom_field: "app_id".into(),
		custom_pattern: "steam_app_*".into(),
		custom_name: "My Game".into(),
		..Settings::default()
	};
	let windows = [
		window("z", "steam_app_42", false),
		window("a", "other", true),
	];
	let display = display_for(&settings, None, Some(&windows), &apps, &ProcessNames::new());
	assert!(!display.selected && !display.pinned && display.inset.is_none());
	assert_eq!(
		display.main,
		Visual::CustomApplication {
			key: custom::cache_key(&settings),
			app_id: Some("steam_app_42".into()),
			running: Some(true),
		}
	);
	let gone = display_for(&settings, None, Some(&[]), &apps, &ProcessNames::new());
	assert_eq!(
		gone.main,
		Visual::CustomApplication {
			key: custom::cache_key(&settings),
			app_id: None,
			running: Some(false),
		}
	);
	let unknown = display_for(&settings, None, None, &apps, &ProcessNames::new());
	assert_eq!(
		unknown.main,
		Visual::CustomApplication {
			key: custom::cache_key(&settings),
			app_id: None,
			running: None,
		}
	);
}

#[test]
fn unknown_source_fails_closed() {
	let apps = ApplicationIcons::with_test_entries(vec![]);
	let settings = Settings {
		mode: "defined_application".into(),
		application_source: "unexpected".into(),
		..Settings::default()
	};
	assert!(
		matching_window(
			&apps,
			&settings,
			&[window("a", "browser", true)],
			&ProcessNames::new()
		)
		.is_err()
	);
}

#[test]
fn grouped_picker_moves_running_desktop_apps_to_the_top_without_duplicates() {
	let dir = tempfile::tempdir().unwrap();
	let entries = vec![
		application(dir.path(), "org.example.Editor", Some("editor-window")),
		application(dir.path(), "org.example.Other", None),
	];
	let apps = ApplicationIcons::with_test_entries(entries);
	let installed = vec![
		("org.example.Editor".into(), "Editor".into()),
		("org.example.Other".into(), "Other".into()),
	];
	let windows = [
		window("a", "editor-window", false),
		window("b", "editor-window", false),
		window("c", "steam_app_1234", false),
	];
	let groups = grouped_choices(&apps, &installed, &windows, "");
	assert_eq!(groups.len(), 2);
	assert_eq!(groups[0]["label"], "Running applications");
	let top = groups[0]["children"].as_array().unwrap();
	assert_eq!(top.len(), 2);
	assert!(top.iter().any(|app| app["value"] == "org.example.Editor"));
	assert!(
		top.iter()
			.any(|app| app["value"] == "app-id:steam_app_1234")
	);
	assert_eq!(groups[1]["label"], "Other installed applications");
	assert_eq!(groups[1]["children"].as_array().unwrap().len(), 1);
	assert_eq!(groups[1]["children"][0]["value"], "org.example.Other");
}

#[test]
fn running_only_choice_is_an_exact_app_id_target_without_a_desktop_file() {
	let apps = ApplicationIcons::with_test_entries(vec![]);
	let settings = Settings {
		mode: "defined_application".into(),
		application: "app-id:steam_app_1234".into(),
		..Settings::default()
	};
	let windows = [
		window("b", "steam_app_1234", false),
		window("a", "steam_app_123", true),
	];
	assert_eq!(
		matching_window(&apps, &settings, &windows, &ProcessNames::new())
			.unwrap()
			.unwrap()
			.id,
		"b"
	);
	let display = display_for(&settings, None, Some(&windows), &apps, &ProcessNames::new());
	assert_eq!(
		display.main,
		Visual::CustomApplication {
			key: "running:steam_app_1234".into(),
			app_id: Some("steam_app_1234".into()),
			running: Some(true),
		}
	);
	assert!(display.inset.is_none() && !display.pinned);
	let gone = display_for(&settings, None, Some(&[]), &apps, &ProcessNames::new());
	assert_eq!(
		gone.main,
		Visual::CustomApplication {
			key: "running:steam_app_1234".into(),
			app_id: Some("steam_app_1234".into()),
			running: Some(false),
		}
	);
	let saved = grouped_choices(&apps, &[], &[], &settings.application);
	assert_eq!(saved[0]["label"], "Saved selection");
	assert_eq!(saved[0]["children"][0]["value"], "app-id:steam_app_1234");
}

#[test]
fn conversion_prefers_actual_running_id_and_works_when_app_is_closed() {
	let dir = tempfile::tempdir().unwrap();
	let apps = ApplicationIcons::with_test_entries(vec![application(
		dir.path(),
		"org.example.Editor",
		Some("editor-window"),
	)]);
	let converted = super::picker::conversion_for(
		&apps,
		"org.example.Editor",
		&[window("a", "editor-window", false)],
	)
	.unwrap();
	assert_eq!(converted["pattern"], "editor-window");
	assert_eq!(converted["name"], "Sample");
	let offline = super::picker::conversion_for(&apps, "org.example.Editor", &[]).unwrap();
	assert_eq!(offline["pattern"], "org.example.Editor");
	let running = super::picker::conversion_for(&apps, "app-id:steam_app_78", &[]).unwrap();
	assert_eq!(running["pattern"], "steam_app_78");
	assert!(super::picker::conversion_for(&apps, "", &[]).is_none());
}
