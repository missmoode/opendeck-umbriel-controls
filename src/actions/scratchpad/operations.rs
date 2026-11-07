//! Scratchpad toggle and focused-window move operations.
use super::{Settings, selected_name};
use openaction::{Instance, OpenActionResult};
use umbriel_ipc::{Client, Error};

pub(super) async fn run_action(
	instance: &Instance,
	settings: &Settings,
	hold: bool,
) -> OpenActionResult<()> {
	let result = match Client::from_environment() {
		Ok(client) if hold => {
			move_focused_window(&client, selected_name(settings), settings.hide_on_remove).await
		}
		Ok(client) => client.toggle_scratchpad(selected_name(settings)).await,
		Err(error) => Err(error),
	};
	if let Err(error) = result {
		eprintln!("Scratchpad action failed: {error}");
		instance.show_alert().await?;
	}
	Ok(())
}
pub(super) async fn move_focused_window(
	client: &Client,
	scratchpad: Option<&str>,
	hide_on_remove: bool,
) -> umbriel_ipc::Result<()> {
	let focused = client.focused_window().await?;
	if focused.scratchpad.is_empty() {
		client.move_window_to_scratchpad(scratchpad).await?;
		// Unlike scratchpad-toggle, window-focus reveals a hidden pad without
		// accidentally hiding one that is already visible.
		return client.focus_window(&focused.id).await;
	}

	// Don't remove a window from a *different* scratchpad.
	let name = scratchpad.unwrap_or("default");
	if focused.scratchpad != name {
		return Err(Error::Rejected(format!(
			"Focused window belongs to scratchpad '{}', not '{name}'",
			focused.scratchpad
		)));
	}

	// The active workspace *under* the visible scratchpad is the destination,
	// not the window's saved restore workspace.
	let target = client
		.list_workspaces()
		.await?
		.into_iter()
		.find(|ws| ws.active && ws.output == focused.output)
		.map(|ws| ws.target())
		.ok_or_else(|| {
			Error::Rejected(format!(
				"No active workspace found on output '{}'",
				focused.output
			))
		})?;
	client.move_window_to_workspace(&target).await?;

	// Umbriel already hides a scratchpad when its last window moves out.
	// Only toggle when another member remains visible, or the empty pad
	// could reopen (and potentially trigger spawn_when_empty).
	if hide_on_remove
		&& client
			.list_windows()
			.await?
			.iter()
			.any(|window| window.scratchpad == name && window.visible)
	{
		client.toggle_scratchpad(scratchpad).await?;
	}
	Ok(())
}

#[cfg(test)]
mod move_tests {
	use super::*;
	use serde_json::{Value, json};
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::UnixListener;

	#[tokio::test]
	async fn scratchpad_move_reveals_and_hides_only_when_needed() {
		let directory = tempfile::tempdir().unwrap();
		let socket = directory.path().join("umbriel.sock");
		let listener = UnixListener::bind(&socket).unwrap();

		let server = tokio::spawn(async move {
			let exchanges = [
				(
					json!({"cmd":"focused-window"}),
					json!({"ok": {
						"id":"win-1", "app_id":"foot", "workspace":"old",
						"scratchpad":"music", "output":"DP-2"
					}}),
				),
				(
					json!({"cmd":"workspaces"}),
					json!({"ok": [
						{"id":"ws-1", "name":"1", "named":false, "index":1, "output":"DP-1", "active":true},
						{"id":"ws-2", "name":"CHAT", "named":true, "index":2, "output":"DP-2", "active":true}
					]}),
				),
				(
					json!({"cmd":"action", "action":"window-move-to-workspace", "workspace":"CHAT", "output":"DP-2"}),
					json!({"ok": null}),
				),
				(
					json!({"cmd":"windows"}),
					json!({"ok": [
						{"id":"win-3", "app_id":"foot", "workspace":"", "scratchpad":"music", "visible":true}
					]}),
				),
				(
					json!({"cmd":"action", "action":"scratchpad-toggle", "scratchpad":"music"}),
					json!({"ok": null}),
				),
				(
					json!({"cmd":"focused-window"}),
					json!({"ok": {
						"id":"win-2", "app_id":"foot", "workspace":"ws-2",
						"scratchpad":"", "output":"DP-2"
					}}),
				),
				(
					json!({"cmd":"action", "action":"window-move-to-scratchpad", "scratchpad":"music"}),
					json!({"ok": null}),
				),
				(
					json!({"cmd":"action", "action":"window-focus", "window":"win-2"}),
					json!({"ok": null}),
				),
				// Moving the final window out must not toggle an already-hidden pad.
				(
					json!({"cmd":"focused-window"}),
					json!({"ok": {
						"id":"win-2", "app_id":"foot", "workspace":"old",
						"scratchpad":"music", "output":"DP-2"
					}}),
				),
				(
					json!({"cmd":"workspaces"}),
					json!({"ok": [
						{"id":"ws-2", "name":"CHAT", "named":true, "index":2, "output":"DP-2", "active":true}
					]}),
				),
				(
					json!({"cmd":"action", "action":"window-move-to-workspace", "workspace":"CHAT", "output":"DP-2"}),
					json!({"ok": null}),
				),
				(json!({"cmd":"windows"}), json!({"ok": []})),
			];
			for (expected, response) in exchanges {
				let (stream, _) = listener.accept().await.unwrap();
				let mut peer = BufReader::new(stream);
				let mut line = String::new();
				peer.read_line(&mut line).await.unwrap();
				let request: Value = serde_json::from_str(&line).unwrap();
				assert_eq!(request, expected);
				peer.get_mut()
					.write_all(format!("{response}\n").as_bytes())
					.await
					.unwrap();
			}
		});

		let client = Client::new(socket);
		move_focused_window(&client, Some("music"), true)
			.await
			.unwrap();
		move_focused_window(&client, Some("music"), true)
			.await
			.unwrap();
		move_focused_window(&client, Some("music"), true)
			.await
			.unwrap();
		server.await.unwrap();
	}
}
