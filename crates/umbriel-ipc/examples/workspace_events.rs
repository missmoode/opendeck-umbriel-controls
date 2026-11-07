use umbriel_ipc::{Client, Result};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	let client = Client::from_environment()?;
	let mut events = client.subscribe_workspaces().await?;

	println!("Subscribed. Switch workspaces; Ctrl+C to stop.");

	while let Some(event) = events.next().await? {
		if let Some(workspaces) = event.into_workspaces()? {
			let active: Vec<String> = workspaces
				.iter()
				.filter(|workspace| workspace.active)
				.map(|workspace| format!("{}: {}", workspace.output, workspace.name))
				.collect();

			println!("Active: {}", active.join(", "));
		}
	}

	println!("Umbriel disconnected.");
	Ok(())
}
