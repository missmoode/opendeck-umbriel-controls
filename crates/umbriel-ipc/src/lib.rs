//! An asynchronous, request/response client for Umbriel's IPC socket.
//!

mod actions;
mod client;
mod error;
mod outputs;
mod screencast;
mod subscription;
mod windows;
mod workspaces;

pub use client::Client;
pub use error::{Error, Result};
pub use outputs::Output;
pub use screencast::ScreencastCommand;
pub use subscription::{Event, Subscription};
pub use windows::Window;
pub use workspaces::{Workspace, WorkspaceSelector, WorkspaceTarget};

/// Send a one-shot request using the current Wayland session's socket.
pub async fn send(request: &serde_json::Value) -> Result<serde_json::Value> {
	Client::from_environment()?.send(request).await
}
