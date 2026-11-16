use std::io;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("missing environment variable: {0}")]
	MissingEnvironment(&'static str),

	#[error("socket I/O error: {0}")]
	Io(#[from] io::Error),

	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),

	#[error("Umbriel did not reply")]
	EmptyResponse,

	#[error("invalid Umbriel response: expected 'ok' or 'err'")]
	InvalidResponse,

	#[error("Umbriel rejected the request: {0}")]
	Rejected(String),

	#[error("IPC request timed out after {0:?}")]
	Timeout(Duration),

	#[error("invalid submap path")]
	InvalidSubmapPath,

	#[error("submap stack exceeds the maximum supported depth ({0})")]
	SubmapDepthExceeded(usize),

	#[error("submap did not match the requested destination")]
	SubmapMismatch,

	#[error("submap operation failed: {0}")]
	SubmapCleanup(String),

	#[error("subscription must include at least one event family")]
	EmptySubscription,
}

pub type Result<T> = std::result::Result<T, Error>;
