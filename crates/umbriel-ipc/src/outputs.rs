use crate::{Client, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Output {
	pub name: String,
	#[serde(default)]
	pub focused: bool,
	#[serde(default)]
	pub enabled: bool,
}

impl Client {
	/// Query all connected outputs, including their focused state.
	pub async fn list_outputs(&self) -> Result<Vec<Output>> {
		let value = self.send(&json!({"cmd": "outputs"})).await?;
		Ok(serde_json::from_value(value)?)
	}
}
