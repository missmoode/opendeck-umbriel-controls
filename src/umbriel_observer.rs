use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use umbriel_ipc::{Client, Output, Result, ScreencastCommand, Window, Workspace};

pub type WorkspaceSnapshot = Arc<Vec<Workspace>>;
pub type WorkspaceUpdates = watch::Receiver<Option<WorkspaceSnapshot>>;

pub type WindowSnapshot = Arc<Vec<Window>>;
pub type WindowUpdates = watch::Receiver<Option<WindowSnapshot>>;

pub type OutputSnapshot = Arc<Vec<Output>>;
pub type OutputUpdates = watch::Receiver<Option<OutputSnapshot>>;

pub type ScreencastUpdates = watch::Receiver<Option<ScreencastCommand>>;
pub type SessionUpdates = watch::Receiver<u64>;

/// One subscription for the whole plugin; separate channels per family.
pub struct UmbrielObserver {
	workspaces: WorkspaceUpdates,
	windows: WindowUpdates,
	outputs: OutputUpdates,
	screencast: ScreencastUpdates,
	session: SessionUpdates,
}

impl UmbrielObserver {
	pub fn start() -> Self {
		let (ws_tx, workspaces) = watch::channel(None::<WorkspaceSnapshot>);
		let (win_tx, windows) = watch::channel(None::<WindowSnapshot>);
		let (out_tx, outputs) = watch::channel(None::<OutputSnapshot>);
		let (cast_tx, screencast) = watch::channel(None::<ScreencastCommand>);
		let (session_tx, session) = watch::channel(0_u64);

		tokio::spawn(async move {
			loop {
				let result = match Client::from_environment() {
					Ok(client) => observe(&client, &ws_tx, &win_tx, &out_tx, &cast_tx).await,
					Err(error) => Err(error),
				};

				match result {
					Ok(()) => eprintln!("Umbriel subscription closed"),
					Err(error) => {
						eprintln!("Umbriel observer error: {error}")
					}
				}

				// The old session's data must not masquerade as current data.
				ws_tx.send_replace(None);
				win_tx.send_replace(None);
				out_tx.send_replace(None);
				cast_tx.send_replace(None);
				// A generation cannot be skipped even if receivers coalesce events.
				session_tx.send_modify(|generation| *generation = generation.wrapping_add(1));

				tokio::time::sleep(Duration::from_secs(2)).await;
			}
		});

		Self {
			workspaces,
			windows,
			outputs,
			screencast,
			session,
		}
	}

	pub fn workspaces(&self) -> WorkspaceUpdates {
		self.workspaces.clone()
	}

	pub fn windows(&self) -> WindowUpdates {
		self.windows.clone()
	}

	pub fn outputs(&self) -> OutputUpdates {
		self.outputs.clone()
	}

	pub fn screencast(&self) -> ScreencastUpdates {
		self.screencast.clone()
	}

	pub fn session(&self) -> SessionUpdates {
		self.session.clone()
	}
}

/// One connection attempt; parameterised by Client for fake-socket tests.
async fn observe(
	client: &Client,
	workspaces: &watch::Sender<Option<WorkspaceSnapshot>>,
	windows: &watch::Sender<Option<WindowSnapshot>>,
	outputs: &watch::Sender<Option<OutputSnapshot>>,
	screencast: &watch::Sender<Option<ScreencastCommand>>,
) -> Result<()> {
	let mut subscription = client
		.subscribe(&["workspaces", "windows", "outputs", "screencast"])
		.await?;

	while let Some(event) = subscription.next().await? {
		match event.event.as_str() {
			"workspaces" => {
				let snapshot: Vec<Workspace> = serde_json::from_value(event.data)?;

				workspaces.send_replace(Some(Arc::new(snapshot)));
			}
			"windows" => {
				let snapshot: Vec<Window> = serde_json::from_value(event.data)?;

				windows.send_replace(Some(Arc::new(snapshot)));
			}
			"outputs" => {
				let snapshot: Vec<Output> = serde_json::from_value(event.data)?;

				outputs.send_replace(Some(Arc::new(snapshot)));
			}
			"screencast" => {
				let command: ScreencastCommand = serde_json::from_value(event.data)?;

				screencast.send_replace(Some(command));
			}
			_ => {}
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::{Value, json};
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::UnixListener;

	#[tokio::test]
	async fn routes_all_families_to_separate_receivers() {
		let directory = tempfile::tempdir().unwrap();
		let socket = directory.path().join("umbriel.sock");
		let listener = UnixListener::bind(&socket).unwrap();

		let server = tokio::spawn(async move {
			let (stream, _) = listener.accept().await.unwrap();
			let mut peer = BufReader::new(stream);

			let mut request_line = String::new();
			peer.read_line(&mut request_line).await.unwrap();

			let request: Value = serde_json::from_str(&request_line).unwrap();

			// All four families must use the same subscription.
			assert_eq!(
				request["events"],
				json!(["workspaces", "windows", "outputs", "screencast"])
			);

			// Simulate four events arriving on one socket.
			let messages = format!(
				"{}\n{}\n{}\n{}\n",
				json!({
					"event": "workspaces",
					"data": [{
						"id": "ws-7",
						"name": "WEB",
						"named": true,
						"index": 2,
						"output": "DP-1",
						"active": true,
						"focused": true,
						"layout": "dwindle"
					}]
				}),
				json!({
					"event": "windows",
					"data": [{
						"id": "win-1",
						"app_id": "firefox",
						"workspace": "ws-7",
						"scratchpad": "",
						"active": true
					}]
				}),
				json!({
					"event": "outputs",
					"data": [{
						"name": "DP-1",
						"focused": true,
						"enabled": true
					}]
				}),
				json!({
					"event": "screencast",
					"data": {
						"serial": 13,
						"kind": "output",
						"output": "DP-1"
					}
				})
			);

			peer.get_mut().write_all(messages.as_bytes()).await.unwrap();
		});

		// Four independent receivers, using the actual observer logic.
		let (ws_tx, ws_rx) = watch::channel(None::<WorkspaceSnapshot>);
		let (win_tx, win_rx) = watch::channel(None::<WindowSnapshot>);
		let (out_tx, out_rx) = watch::channel(None::<OutputSnapshot>);
		let (cast_tx, cast_rx) = watch::channel(None::<ScreencastCommand>);

		observe(&Client::new(socket), &ws_tx, &win_tx, &out_tx, &cast_tx)
			.await
			.unwrap();

		// Workspace/window relationship.
		let workspaces = ws_rx.borrow().clone().unwrap();
		let windows = win_rx.borrow().clone().unwrap();

		assert_eq!(windows[0].workspace, workspaces[0].id);
		assert!(windows[0].active);

		// Output snapshot.
		let outputs = out_rx.borrow().clone().unwrap();

		assert_eq!(outputs[0].name, "DP-1");
		assert!(outputs[0].focused);

		// Accepted screencast command.
		let screencast = cast_rx.borrow().clone().unwrap();

		assert_eq!(
			screencast,
			ScreencastCommand::Output {
				serial: 13,
				output: "DP-1".into(),
			}
		);

		server.await.unwrap();
	}
}
