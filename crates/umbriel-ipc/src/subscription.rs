use crate::{Client, Error, Result, Workspace};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::time::timeout;

/// One event delivered by Umbriel's subscription socket.
#[derive(Debug, Deserialize)]
pub struct Event {
	pub event: String,
	pub data: Value,
}

impl Event {
	/// Decode workspace snapshots, ignoring other event families.
	pub fn into_workspaces(self) -> Result<Option<Vec<Workspace>>> {
		if self.event != "workspaces" {
			return Ok(None);
		}

		Ok(Some(serde_json::from_value(self.data)?))
	}

	/// Decode a window event, ignore unrelated families.
	pub fn into_windows(self) -> Result<Option<Vec<crate::Window>>> {
		if self.event != "windows" {
			return Ok(None);
		}
		Ok(Some(serde_json::from_value(self.data)?))
	}
}

/// A single persistent socket connection, owned by one reader.
pub struct Subscription {
	lines: Lines<BufReader<UnixStream>>,
}

impl Subscription {
	/// Read one event. Ok(None) means the socket reached EOF.
	pub async fn next(&mut self) -> Result<Option<Event>> {
		let Some(line) = self.lines.next_line().await? else {
			return Ok(None);
		};

		let response: Value = serde_json::from_str(&line)?;

		// An invalid subscription can produce an error line.
		if let Some(error) = response.get("err") {
			let message = error
				.as_str()
				.map(str::to_owned)
				.unwrap_or_else(|| error.to_string());
			return Err(Error::Rejected(message));
		}

		// For events, Umbriel uses {"event": ..., "data": ...}.
		Ok(Some(serde_json::from_value(response)?))
	}
}

impl Client {
	/// Subscribe to one or more Umbriel event families.
	pub async fn subscribe(&self, events: &[&str]) -> Result<Subscription> {
		if events.is_empty() {
			return Err(Error::EmptySubscription);
		}

		// Connect and write are bounded. Waiting for events is not.
		let mut stream = self.connect().await?;
		let mut request = serde_json::to_vec(&json!({
			"cmd": "subscribe",
			"events": events,
		}))?;
		request.push(b'\n');

		timeout(self.timeout(), stream.write_all(&request))
			.await
			.map_err(|_| Error::Timeout(self.timeout()))??;

		Ok(Subscription {
			lines: BufReader::new(stream).lines(),
		})
	}

	/// A convenient name for our immediate use case.
	pub async fn subscribe_workspaces(&self) -> Result<Subscription> {
		self.subscribe(&["workspaces"]).await
	}
}
