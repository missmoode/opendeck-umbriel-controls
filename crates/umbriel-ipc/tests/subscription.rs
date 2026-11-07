use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use umbriel_ipc::{Client, Error};

fn workspace_event(active: bool) -> String {
	let event = json!({
		"event": "workspaces",
		"data": [{
			"id": "workspace-1",
			"name": "WEB",
			"named": true,
			"index": 1,
			"output": "DP-1",
			"active": active,
		}],
	});

	format!("{event}\n")
}

#[tokio::test]
async fn subscription_receives_initial_and_updated_snapshots() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut peer = BufReader::new(stream);

		let mut request_line = String::new();
		peer.read_line(&mut request_line).await.unwrap();
		let request: Value = serde_json::from_str(&request_line).unwrap();
		assert_eq!(
			request,
			json!({
				"cmd": "subscribe",
				"events": ["workspaces"],
			})
		);

		peer.get_mut()
			.write_all(workspace_event(false).as_bytes())
			.await
			.unwrap();

		// An idle period is not a disconnection.
		tokio::time::sleep(Duration::from_millis(120)).await;

		peer.get_mut()
			.write_all(workspace_event(true).as_bytes())
			.await
			.unwrap();
		// Dropping peer closes the socket.
	});

	let client = Client::new(socket);
	let mut subscription = client.subscribe_workspaces().await.unwrap();

	let first = subscription.next().await.unwrap().unwrap();
	let first = first.into_workspaces().unwrap().unwrap();
	assert!(!first[0].active);
	assert_eq!(first[0].name, "WEB");

	let second = subscription.next().await.unwrap().unwrap();
	let second = second.into_workspaces().unwrap().unwrap();
	assert!(second[0].active);

	assert!(subscription.next().await.unwrap().is_none());
	server.await.unwrap();
}

#[tokio::test]
async fn subscription_reports_rejection() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (mut stream, _) = listener.accept().await.unwrap();
		let mut bytes = [0u8; 512];
		let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut bytes)
			.await
			.unwrap();
		stream
			.write_all(b"{\"err\":\"unknown subscription event\"}\n")
			.await
			.unwrap();
	});

	let client = Client::new(socket);
	let mut subscription = client.subscribe(&["not-a-family"]).await.unwrap();
	let error = subscription.next().await.unwrap_err();

	assert!(matches!(
		error,
		Error::Rejected(message) if message == "unknown subscription event"
	));
	server.await.unwrap();
}

#[tokio::test]
async fn can_subscribe_again_after_eof() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		for active in [false, true] {
			let (stream, _) = listener.accept().await.unwrap();
			let mut peer = BufReader::new(stream);
			let mut request_line = String::new();
			peer.read_line(&mut request_line).await.unwrap();
			peer.get_mut()
				.write_all(workspace_event(active).as_bytes())
				.await
				.unwrap();
			// This connection closes; the listener remains available.
		}
	});

	let client = Client::new(socket);
	let mut first = client.subscribe_workspaces().await.unwrap();
	assert!(
		!first
			.next()
			.await
			.unwrap()
			.unwrap()
			.into_workspaces()
			.unwrap()
			.unwrap()[0]
			.active
	);
	assert!(first.next().await.unwrap().is_none());

	let mut second = client.subscribe_workspaces().await.unwrap();
	assert!(
		second
			.next()
			.await
			.unwrap()
			.unwrap()
			.into_workspaces()
			.unwrap()
			.unwrap()[0]
			.active
	);

	server.await.unwrap();
}

#[tokio::test]
async fn empty_subscription_is_rejected_locally() {
	let client = Client::new("/unused/socket");
	let result = client.subscribe(&[]).await;
	assert!(matches!(result, Err(Error::EmptySubscription)));
}

#[tokio::test]
async fn one_subscription_receives_both_event_families() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut peer = BufReader::new(stream);
		let mut request_line = String::new();
		peer.read_line(&mut request_line).await.unwrap();
		let request: Value = serde_json::from_str(&request_line).unwrap();
		assert_eq!(
			request,
			json!({
				"cmd": "subscribe",
				"events": ["workspaces", "windows"]
			})
		);

		peer.get_mut()
			.write_all(
				format!(
					"{}\n",
					json!({
						"event": "workspaces",
						"data": [{
							"id": "ws-7", "name": "WEB", "named": true,
							"index": 2, "output": "DP-1", "active": true
						}]
					})
				)
				.as_bytes(),
			)
			.await
			.unwrap();

		peer.get_mut()
			.write_all(
				format!(
					"{}\n",
					json!({
						"event": "windows",
						"data": [{
							"id": "top-42", "app_id": "firefox",
							"workspace": "ws-7", "scratchpad": ""
						}]
					})
				)
				.as_bytes(),
			)
			.await
			.unwrap();
	});

	let client = Client::new(socket);
	let mut subscription = client.subscribe(&["workspaces", "windows"]).await.unwrap();
	let ws = subscription
		.next()
		.await
		.unwrap()
		.unwrap()
		.into_workspaces()
		.unwrap()
		.unwrap();
	let wins = subscription
		.next()
		.await
		.unwrap()
		.unwrap()
		.into_windows()
		.unwrap()
		.unwrap();

	assert_eq!(wins[0].workspace, ws[0].id);
	server.await.unwrap();
}
