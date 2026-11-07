use crate::{Error, Result};
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct Client {
	socket_path: PathBuf,
	timeout: Duration,
}

impl Client {
	pub fn new(socket_path: impl Into<PathBuf>) -> Self {
		Self {
			socket_path: socket_path.into(),
			timeout: Duration::from_secs(3),
		}
	}

	pub fn from_environment() -> Result<Self> {
		if let Some(path) = env::var_os("UMBRIEL_SOCKET")
			&& !path.is_empty()
		{
			return Ok(Self::new(path));
		}

		let runtime_dir =
			env::var_os("XDG_RUNTIME_DIR").ok_or(Error::MissingEnvironment("XDG_RUNTIME_DIR"))?;
		let display = env::var("WAYLAND_DISPLAY")
			.map_err(|_| Error::MissingEnvironment("WAYLAND_DISPLAY"))?;

		if display.is_empty() {
			return Err(Error::MissingEnvironment("WAYLAND_DISPLAY"));
		}

		let path = Path::new(&runtime_dir).join(format!("umbriel-{display}.sock"));

		Ok(Self::new(path))
	}

	pub async fn send(&self, request: &Value) -> Result<Value> {
		match timeout(self.timeout, self.send_once(request)).await {
			Ok(result) => result,
			Err(_) => Err(Error::Timeout(self.timeout)),
		}
	}

	async fn send_once(&self, request: &Value) -> Result<Value> {
		let mut stream = UnixStream::connect(&self.socket_path).await?;

		// Umbriel uses one JSON object per line.
		let mut bytes = serde_json::to_vec(request)?;
		bytes.push(b'\n');
		stream.write_all(&bytes).await?;

		let mut reader = BufReader::new(stream);
		let mut line = String::new();
		let bytes_read = reader.read_line(&mut line).await?;
		if bytes_read == 0 {
			return Err(Error::EmptyResponse);
		}

		let response: Value = serde_json::from_str(&line)?;

		if let Some(err) = response.get("err") {
			let message = err
				.as_str()
				.map(str::to_owned)
				.unwrap_or_else(|| err.to_string());
			return Err(Error::Rejected(message));
		}

		response.get("ok").cloned().ok_or(Error::InvalidResponse)
	}

	pub(crate) async fn connect(&self) -> Result<UnixStream> {
		timeout(self.timeout, UnixStream::connect(&self.socket_path))
			.await
			.map_err(|_| Error::Timeout(self.timeout))?
			.map_err(Error::from)
	}

	pub(crate) fn timeout(&self) -> Duration {
		self.timeout
	}
}
