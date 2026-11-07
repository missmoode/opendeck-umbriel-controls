//! Workspace destination interpretation and window membership.
use super::Settings;
use umbriel_ipc::{Window, Workspace, WorkspaceSelector, WorkspaceTarget};

pub(super) fn display_label(settings: &Settings) -> String {
	if !settings.label.trim().is_empty() {
		return settings.label.trim().to_owned();
	}

	let Ok(target) = serde_json::from_str::<WorkspaceTarget>(&settings.target) else {
		return "?".to_owned();
	};

	match target.workspace {
		WorkspaceSelector::Index(index) => index.to_string(),
		WorkspaceSelector::Name(name) => name,
	}
}

pub(super) fn matches_target(workspace: &Workspace, target: &WorkspaceTarget) -> bool {
	workspace.output == target.output
		&& match &target.workspace {
			WorkspaceSelector::Index(index) => workspace.index == *index,
			WorkspaceSelector::Name(name) => workspace.named && workspace.name == name.as_str(),
		}
}

pub(super) fn target_is_active(settings: &Settings, workspaces: &[Workspace]) -> bool {
	let Ok(target) = serde_json::from_str::<WorkspaceTarget>(&settings.target) else {
		return false;
	};

	workspaces
		.iter()
		.any(|workspace| workspace.active && matches_target(workspace, &target))
}

/// The target's on-screen name is not its stable membership ID.
pub(super) fn selected_workspace_id<'a>(
	settings: &Settings,
	workspaces: &'a [Workspace],
) -> Option<&'a str> {
	let target: WorkspaceTarget = serde_json::from_str(&settings.target).ok()?;
	workspaces
		.iter()
		.find(|ws| matches_target(ws, &target))
		.map(|ws| ws.id.as_str())
}

pub(super) fn workspace_apps(
	settings: &Settings,
	workspaces: &[Workspace],
	windows: &[Window],
) -> Vec<String> {
	let Some(id) = selected_workspace_id(settings, workspaces) else {
		return Vec::new();
	};
	let mut apps: Vec<_> = windows
		.iter()
		.filter(|window| window.workspace == id && window.scratchpad.is_empty())
		.filter(|window| !window.app_id.is_empty())
		.map(|window| window.app_id.clone())
		.collect();
	apps.sort();
	apps.dedup();
	apps.truncate(4);
	apps
}
