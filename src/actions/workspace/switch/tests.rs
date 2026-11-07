//! Visual and workspace matching regression tests.
use super::Settings;
use super::render::image_key;
use super::target::{display_label, target_is_active, workspace_apps};
use umbriel_ipc::{Window, Workspace};

fn sample(name: &str, named: bool, index: u32, output: &str) -> Workspace {
	Workspace {
		id: format!("{output}:{index}"),
		name: name.to_owned(),
		named,
		index,
		output: output.to_owned(),
		active: true,
		focused: false,
		layout: "scrolling".into(),
	}
}

fn settings(target: serde_json::Value) -> Settings {
	Settings {
		target: target.to_string(),
		..Settings::default()
	}
}

#[test]
fn missing_badge_setting_uses_the_same_default_as_new_buttons() {
	assert!(Settings::default().show_badge);
	assert!(
		serde_json::from_str::<Settings>(r#"{"target":"foo"}"#)
			.unwrap()
			.show_badge
	);
	assert!(
		!serde_json::from_str::<Settings>(r#"{"show_badge":false}"#)
			.unwrap()
			.show_badge
	);
}

#[test]
fn badge_visibility_changes_the_image_cache_key() {
	let with_badge = image_key(&Settings::default(), None, None, 0, 0);
	let settings = Settings {
		show_badge: false,
		..Settings::default()
	};
	assert_ne!(with_badge, image_key(&settings, None, None, 0, 0));
}

#[test]
fn numeric_position_matches_a_named_workspace_at_that_position() {
	let s = settings(serde_json::json!({
		"workspace": 2, "output": "DP-1"
	}));
	assert!(target_is_active(&s, &[sample("WEB", true, 2, "DP-1")]));
}

#[test]
fn numeric_name_does_not_match_a_numeric_position() {
	let s = settings(serde_json::json!({
		"workspace": "2", "output": "DP-1"
	}));
	assert!(!target_is_active(&s, &[sample("2", false, 2, "DP-1")]));
	assert!(target_is_active(&s, &[sample("2", true, 1, "DP-1")]));
}

#[test]
fn another_output_does_not_count() {
	let s = settings(serde_json::json!({
		"workspace": "WEB", "output": "DP-1"
	}));
	assert!(!target_is_active(&s, &[sample("WEB", true, 1, "DP-2")]));
}

#[test]
fn custom_label_overrides_target() {
	let mut s = settings(serde_json::json!({
		"workspace": "WEB", "output": "DP-1"
	}));
	assert_eq!(display_label(&s), "WEB");

	s.label = "Browser".to_owned();
	assert_eq!(display_label(&s), "Browser");
}

#[test]
fn app_membership_uses_workspace_id_and_deduplicates() {
	let settings = settings(serde_json::json!({
		"workspace": "WEB", "output": "DP-1"
	}));
	let mut workspace = sample("WEB", true, 2, "DP-1");
	workspace.id = "ws-7".into();
	let windows = vec![
		Window {
			id: "a".into(),
			app_id: "firefox".into(),
			title: String::new(),
			pid: None,
			workspace: "ws-7".into(),
			scratchpad: "".into(),
			active: false,
			output: "DP-1".into(),
			visible: true,
		},
		Window {
			id: "b".into(),
			app_id: "firefox".into(),
			title: String::new(),
			pid: None,
			workspace: "ws-7".into(),
			scratchpad: "".into(),
			active: false,
			output: "DP-1".into(),
			visible: true,
		},
		Window {
			id: "c".into(),
			app_id: "terminal".into(),
			title: String::new(),
			pid: None,
			workspace: "ws-7".into(),
			scratchpad: "".into(),
			active: false,
			output: "DP-1".into(),
			visible: true,
		},
		Window {
			id: "d".into(),
			app_id: "chat".into(),
			title: String::new(),
			pid: None,
			workspace: "elsewhere".into(),
			scratchpad: "".into(),
			active: false,
			output: "DP-1".into(),
			visible: true,
		},
	];
	assert_eq!(
		workspace_apps(&settings, &[workspace], &windows),
		vec!["firefox", "terminal"]
	);
}

#[test]
fn missing_workspace_yields_empty_icon_inventory() {
	let settings = settings(serde_json::json!({
		"workspace": 7, "output": "DP-2"
	}));
	assert!(workspace_apps(&settings, &[], &[]).is_empty());
}
