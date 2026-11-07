//! Tap/hold commands. Does not own render state or button lifetimes.
use super::{HoldAction, Settings, target};
use openaction::{Instance, OpenActionResult};
use umbriel_ipc::{Client, WorkspaceSelector, WorkspaceTarget};

pub(super) async fn run_workspace_action(
	instance: &Instance,
	settings: &Settings,
	hold: bool,
) -> OpenActionResult<()> {
	if settings.target.is_empty() {
		instance.show_alert().await?;
		return Ok(());
	}

	let target: WorkspaceTarget = match serde_json::from_str(&settings.target) {
		Ok(target) => target,
		Err(error) => {
			eprintln!("Invalid saved workspace: {error}");
			instance.show_alert().await?;
			return Ok(());
		}
	};

	let result = match Client::from_environment() {
		Ok(client) => {
			// A missing numeric position would otherwise select the last dynamic workspace.
			// Named workspaces are resolved by Umbriel, including evacuated workspaces.
			if matches!(&target.workspace, WorkspaceSelector::Index(_)) {
				match client.list_workspaces().await {
					Ok(workspaces) => {
						if !workspaces
							.iter()
							.any(|workspace| target::matches_target(workspace, &target))
						{
							eprintln!("Workspace destination is unavailable: {}", settings.target);
							instance.show_alert().await?;
							return Ok(());
						}
					}
					Err(error) => {
						eprintln!("Cannot check workspace destination: {error}");
						instance.show_alert().await?;
						return Ok(());
					}
				}
			}

			match (hold, settings.hold_action) {
				(false, _) => client.switch_workspace(&target).await,
				(true, HoldAction::MoveWindow) => client.move_window_to_workspace(&target).await,
				(true, HoldAction::MoveWindowSilent) => {
					client.move_window_to_workspace_silent(&target).await
				}
				(true, HoldAction::MoveColumn) => client.move_column_to_workspace(&target).await,
				(true, HoldAction::Disabled) => return Ok(()),
			}
		}
		Err(error) => Err(error),
	};

	if let Err(error) = result {
		eprintln!("Workspace action failed: {error}");
		instance.show_alert().await?;
	}

	Ok(())
}
