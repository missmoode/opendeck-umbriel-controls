use serde_json::{Value, json};
use tokio::{
	io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
	net::UnixListener,
};
use umbriel_ipc::Client;

async fn exchange<F, Fut>(response: &'static str, run: F) -> Value
where
	F: FnOnce(Client) -> Fut,
	Fut: std::future::Future<Output = ()>,
{
	let dir = tempfile::tempdir().unwrap();
	let socket = dir.path().join("umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();
	let server = tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let mut peer = BufReader::new(stream);
		let mut line = String::new();
		peer.read_line(&mut line).await.unwrap();
		peer.get_mut().write_all(response.as_bytes()).await.unwrap();
		serde_json::from_str::<Value>(&line).unwrap()
	});
	run(Client::new(socket)).await;
	server.await.unwrap()
}

#[tokio::test]
async fn queries_null_and_named_submaps() {
	let request = exchange("{\"ok\":null}\n", |client| async move {
		assert_eq!(client.active_submap().await.unwrap(), None);
	})
	.await;
	assert_eq!(request, json!({"cmd": "submap"}));

	let request = exchange("{\"ok\":\"screenshots\"}\n", |client| async move {
		assert_eq!(
			client.active_submap().await.unwrap().as_deref(),
			Some("screenshots")
		);
	})
	.await;
	assert_eq!(request, json!({"cmd": "submap"}));
}

#[tokio::test]
async fn pushes_submap_and_resets_one_level() {
	let entered = exchange("{\"ok\":null}\n", |client| async move {
		client.enter_submap("screenshots").await.unwrap();
	})
	.await;
	assert_eq!(
		entered,
		json!({"cmd": "action", "action": "submap", "submap": "screenshots"})
	);

	let exited = exchange("{\"ok\":null}\n", |client| async move {
		client.exit_submap().await.unwrap();
	})
	.await;
	assert_eq!(
		exited,
		json!({"cmd": "action", "action": "submap", "submap": "reset"})
	);
}

use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;
use umbriel_ipc::{Error, MAX_SUBMAP_DEPTH};

type Stack = Arc<Mutex<Vec<String>>>;

// Umbriel accepts one command per new socket connection. This mock uses the
// same command protocol so the tests exercise the full reset/rebuild sequence.
fn mock_submaps(
	initial: &[&str],
	fail_on_enter: Option<&'static str>,
) -> (tempfile::TempDir, Client, Stack, StackRequests, JoinHandle<()>) {
	let dir = tempfile::tempdir().unwrap();
	let socket = dir.path().join("mock-umbriel.sock");
	let listener = UnixListener::bind(&socket).unwrap();
	let stack: Stack = Arc::new(Mutex::new(
		initial.iter().map(|name| (*name).into()).collect(),
	));
	let requests = Arc::new(Mutex::new(Vec::new()));
	let stack_for_server = Arc::clone(&stack);
	let requests_for_server = Arc::clone(&requests);
	let task = tokio::spawn(async move {
		loop {
			let (stream, _) = listener.accept().await.unwrap();
			let mut peer = BufReader::new(stream);
			let mut line = String::new();
			peer.read_line(&mut line).await.unwrap();
			let request: Value = serde_json::from_str(&line).unwrap();
			requests_for_server.lock().unwrap().push(request.clone());
			let response = match request.get("cmd").and_then(Value::as_str) {
				Some("submap") => {
					let current = stack_for_server.lock().unwrap().last().cloned();
					json!({"ok": current})
				},
				Some("action") => {
					let name = request["submap"].as_str().unwrap();
					if name == "reset" {
						let popped = stack_for_server.lock().unwrap().pop();
						if popped.is_some() { json!({"ok":null}) }
						else { json!({"err":"no submap to exit"}) }
					} else if Some(name) == fail_on_enter {
						json!({"err":"simulated push failure"})
					} else {
						stack_for_server.lock().unwrap().push(name.to_owned());
						json!({"ok":null})
					}
				}
				_ => json!({"err":"unknown command"}),
			};
			peer.get_mut().write_all(format!("{response}\n").as_bytes()).await.unwrap();
		}
	});
	(dir, Client::new(socket), stack, requests, task)
}

type StackRequests = Arc<Mutex<Vec<Value>>>;

#[tokio::test]
async fn reset_clears_all_layers_including_unnamed() {
	let (_dir, client, stack, requests, task) = mock_submaps(&["gaming", "", "audio"], None);
	client.reset_submaps().await.unwrap();
	assert!(stack.lock().unwrap().is_empty());
	let pops = requests.lock().unwrap().iter().filter(|r| r.get("submap") == Some(&json!("reset"))).count();
	assert_eq!(pops, 3);
	// Resetting again while already at the default context makes no changes.
	client.reset_submaps().await.unwrap();
	assert!(stack.lock().unwrap().is_empty());
	task.abort();
}

#[tokio::test]
async fn set_replaces_every_layer_and_repeated_press_does_not_nest() {
	let (_dir, client, stack, _requests, task) = mock_submaps(&["other", "previous"], None);
	client.set_submap_path(&["gaming", "audio"]).await.unwrap();
	assert_eq!(*stack.lock().unwrap(), vec!["gaming", "audio"]);
	client.set_submap_path(&["gaming", "audio"]).await.unwrap();
	assert_eq!(*stack.lock().unwrap(), vec!["gaming", "audio"]);
	task.abort();
}

#[tokio::test]
async fn invalid_path_does_not_clear_existing_stack() {
	let (_dir, client, stack, requests, task) = mock_submaps(&["existing"], None);
	assert!(matches!(client.set_submap_path(&["valid", "reset"]).await, Err(Error::InvalidSubmapPath)));
	assert!(matches!(
		client.set_submap_path(&[]).await,
		Err(Error::InvalidSubmapPath)
	));
	assert!(requests.lock().unwrap().is_empty());
	assert_eq!(*stack.lock().unwrap(), vec!["existing"]);
	task.abort();
}

#[tokio::test]
async fn unsuccessful_push_clears_partial_path() {
	let (_dir, client, stack, _requests, task) = mock_submaps(&["old"], Some("broken"));
	assert!(client.set_submap_path(&["new", "broken"]).await.is_err());
	assert!(stack.lock().unwrap().is_empty());
	task.abort();
}

#[tokio::test]
async fn excessive_depth_aborts_without_pushing_new_path() {
	let initial = vec!["layer"; MAX_SUBMAP_DEPTH + 1];
	let (_dir, client, stack, requests, task) = mock_submaps(&initial, None);
	assert!(matches!(
		client.set_submap_path(&["new"]).await,
		Err(Error::SubmapDepthExceeded(_))
	));
	assert_eq!(stack.lock().unwrap().as_slice(), ["layer"]);
	assert!(!requests.lock().unwrap().iter().any(|r| r.get("submap") == Some(&json!("new"))));
	task.abort();
}
