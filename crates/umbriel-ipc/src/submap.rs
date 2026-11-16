use crate::{Client, Error, Result};
use serde_json::json;

/// Maximum depth traversed when clearing nested submaps.
/// Umbriel currently has no atomic clear-all IPC operation.
pub const MAX_SUBMAP_DEPTH: usize = 64;

/// Whether a name is valid for Umbriel's `submap` action.
pub fn valid_submap_name(name: &str) -> bool {
	!name.is_empty() && name != "reset" && name != "disable" && !name.contains(']')
}

impl Client {
	/// The active layer, or None when the compositor is in its default context.
	/// Some("") is possible: Umbriel permits an unnamed layer on the stack.
	pub async fn active_submap(&self) -> Result<Option<String>> {
		let value = self.send(&json!({"cmd": "submap"})).await?;
		Ok(serde_json::from_value(value)?)
	}

	/// Push a named submap onto Umbriel's stack.
	pub async fn enter_submap(&self, name: &str) -> Result<()> {
		self.send(&json!({"cmd": "action", "action": "submap", "submap": name}))
			.await?;
		Ok(())
	}

	/// Pop one level from Umbriel's submap stack.
	pub async fn exit_submap(&self) -> Result<()> {
		self.send(&json!({"cmd": "action", "action": "submap", "submap": "reset"}))
			.await?;
		Ok(())
	}

	/// Return to default keybindings, popping each level until Umbriel reports null.
	/// The upper bound avoids indefinite loops if another client keeps pushing.
	pub async fn reset_submaps(&self) -> Result<()> {
		for _ in 0..MAX_SUBMAP_DEPTH {
			if self.active_submap().await?.is_none() {
				return Ok(());
			}
			self.exit_submap().await?;
		}
		if self.active_submap().await?.is_none() {
			Ok(())
		} else {
			Err(Error::SubmapDepthExceeded(MAX_SUBMAP_DEPTH))
		}
	}

	/// Replace the stack with exactly this path, starting from the default context.
	/// Each step is a separate IPC request; other Umbriel clients may interleave.
	pub async fn set_submap_path(&self, path: &[&str]) -> Result<()> {
		if path.is_empty()
			|| path.len() > MAX_SUBMAP_DEPTH
			|| path.iter().any(|name| !valid_submap_name(name))
		{
			return Err(Error::InvalidSubmapPath);
		}

		// Never push if clearing fails; that would unintentionally nest paths.
		self.reset_submaps().await?;

		let result = async {
			for &name in path {
				self.enter_submap(name).await?;
			}
			// Umbriel exposes only the top entry; this verifies that at least
			// the resulting active name is the expected destination.
			let actual = self.active_submap().await?;
			if actual.as_deref() != path.last().copied() {
				return Err(Error::SubmapMismatch);
			}
			Ok(())
		}
		.await;

		if let Err(error) = result {
			// Avoid leaving a partially applied path after a failed push.
			return match self.reset_submaps().await {
				Ok(()) => Err(error),
				Err(cleanup) => Err(Error::SubmapCleanup(format!(
					"{error}; cleanup also failed: {cleanup}"
				))),
			};
		}
		Ok(())
	}
}
