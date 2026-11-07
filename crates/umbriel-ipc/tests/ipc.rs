use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use umbriel_ipc::{Client, Error};

async fn exchange(request: Value, response: &'static str) -> (Value, umbriel_ipc::Result<Value>) {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut reader = BufReader::new(stream);
		let mut line = String::new();
		reader.read_line(&mut line).await.unwrap();
		let received: Value = serde_json::from_str(&line).unwrap();
		reader
			.get_mut()
			.write_all(response.as_bytes())
			.await
			.unwrap();
		received
	});

	let client = Client::new(socket);
	let result = client.send(&request).await;
	let received = server.await.unwrap();
	(received, result)
}

#[tokio::test]
async fn sends_json_and_returns_ok_value() {
	let request = json!({"cmd": "workspaces"});
	let (received, result) = exchange(request.clone(), "{\"ok\":[{\"name\":\"1\"}]}\n").await;

	assert_eq!(received, request);
	assert_eq!(result.unwrap(), json!([{"name": "1"}]));
}

#[tokio::test]
async fn converts_umbriel_errors() {
	let (_, result) = exchange(
		json!({"cmd": "action", "action": "scratchpad-toggle"}),
		"{\"err\":\"unknown scratchpad\"}\n",
	)
	.await;

	assert!(matches!(
		result,
		Err(Error::Rejected(message)) if message == "unknown scratchpad"
	));
}

#[tokio::test]
async fn reports_invalid_json() {
	let (_, result) = exchange(json!({"cmd": "workspaces"}), "not json\n").await;
	assert!(matches!(result, Err(Error::Json(_))));
}

#[tokio::test]
async fn handles_empty_response() {
	let (_, result) = exchange(json!({"cmd": "workspaces"}), "").await;
	assert!(matches!(result, Err(Error::EmptyResponse)));
}

#[tokio::test]
async fn focused_window_action_omits_target_field() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut peer = BufReader::new(stream);
		let mut line = String::new();
		peer.read_line(&mut line).await.unwrap();
		peer.get_mut().write_all(b"{\"ok\":null}\n").await.unwrap();
		serde_json::from_str::<Value>(&line).unwrap()
	});

	Client::new(socket)
		.screencast_set_window(None)
		.await
		.unwrap();
	let received = server.await.unwrap();
	assert_eq!(
		received,
		json!({
			"cmd": "action",
			"action": "screencast-set-window",
		})
	);
	assert!(received.get("window").is_none());
}

#[tokio::test]
async fn explicit_window_action_sends_the_saved_identifier() {
	let directory = tempfile::tempdir().unwrap();
	let socket = directory.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();

	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut peer = BufReader::new(stream);
		let mut line = String::new();
		peer.read_line(&mut line).await.unwrap();
		peer.get_mut().write_all(b"{\"ok\":null}\n").await.unwrap();
		serde_json::from_str::<Value>(&line).unwrap()
	});

	Client::new(socket)
		.screencast_set_window(Some("window-123"))
		.await
		.unwrap();
	assert_eq!(
		server.await.unwrap(),
		json!({
			"cmd": "action", "action": "screencast-set-window", "window": "window-123"
		})
	);
}
