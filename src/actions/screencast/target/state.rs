use openaction::{Instance, OpenActionResult};

use super::{
	super::model::{Mode, RememberedWindow, Settings},
	TargetAction,
};

/// Per-key view and press state. The programmed target belongs to OpenDeck settings.
#[derive(Clone)]
pub(super) struct ButtonState {
	pub(super) settings: Settings,
	pub(super) session: u64,
}

impl ButtonState {
	pub(super) fn new(mut settings: Settings, session: u64) -> Self {
		if Mode::from_setting(&settings.mode) != Mode::ProgrammedWindow {
			settings.programmed_window = None;
		}
		Self { settings, session }
	}

	pub(super) fn update_session(&mut self, session: u64) -> bool {
		if self.session == session {
			return false;
		}
		self.session = session;
		true
	}

	/// Preserve the runtime assignment across unrelated inspector edits.
	/// A mode change deliberately forgets a programmed window.
	pub(super) fn update_settings(&mut self, incoming: &Settings) -> bool {
		let mut next = incoming.clone();
		if self.settings.mode == incoming.mode
			&& Mode::from_setting(&incoming.mode) == Mode::ProgrammedWindow
		{
			next.programmed_window = self.settings.programmed_window.clone();
		} else {
			next.programmed_window = None;
		}
		let needs_save = next.programmed_window != incoming.programmed_window;
		self.settings = next;
		needs_save
	}

	/// Only a clear needs to match the old target; a hold can reprogram it.
	pub(super) fn set_programmed_window(
		&mut self,
		session: u64,
		expected: Option<&RememberedWindow>,
		replacement: Option<RememberedWindow>,
	) -> bool {
		if self.session != session
			|| Mode::from_setting(&self.settings.mode) != Mode::ProgrammedWindow
			|| (replacement.is_none() && self.settings.programmed_window.as_ref() != expected)
			|| self.settings.programmed_window == replacement
		{
			return false;
		}
		self.settings.programmed_window = replacement;
		true
	}
}

impl TargetAction {
	pub(super) async fn record_settings(
		&self,
		instance: &Instance,
		settings: &Settings,
		appearing: bool,
	) -> OpenActionResult<()> {
		let _write = self.settings_writes.lock().await;
		self.pressed.cancel(&instance.instance_id).await;
		let session = *self.session.borrow();
		let mut corrected = None;
		if appearing {
			let button = ButtonState::new(settings.clone(), session);
			if button.settings.programmed_window != settings.programmed_window {
				corrected = Some(button.settings.clone());
			}
			self.buttons.put(&instance.instance_id, button).await;
		} else {
			self.buttons
				.upsert(
					&instance.instance_id,
					|| ButtonState::new(settings.clone(), session),
					|button| {
						button.update_session(session);
						if button.update_settings(settings) {
							corrected = Some(button.settings.clone());
						}
					},
				)
				.await;
		}
		if let Some(settings) = corrected
			&& crate::actions::instance_is_current(instance).await
		{
			instance.set_settings(&settings).await?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn saved(id: &str) -> RememberedWindow {
		RememberedWindow {
			id: id.into(),
			app_id: "browser".into(),
		}
	}

	fn programmed(id: &str) -> Settings {
		Settings {
			mode: "programmed_window".into(),
			programmed_window: Some(saved(id)),
			..Settings::default()
		}
	}

	#[test]
	fn session_reconnect_keeps_assignment_until_live_inventory_invalidates_it() {
		let mut button = ButtonState::new(programmed("w1"), 10);
		assert!(!button.update_session(10));
		assert!(button.update_session(11));
		assert_eq!(button.settings.programmed_window, Some(saved("w1")));
		assert!(!button.set_programmed_window(10, Some(&saved("w1")), None));
		assert!(button.set_programmed_window(11, Some(&saved("w1")), None));
		assert_eq!(button.settings.programmed_window, None);
	}

	#[test]
	fn stale_clear_cannot_overwrite_reprogrammed_window() {
		let mut button = ButtonState::new(programmed("w1"), 1);
		assert!(button.set_programmed_window(1, Some(&saved("w1")), Some(saved("w2"))));
		assert!(!button.set_programmed_window(1, Some(&saved("w1")), None));
		assert_eq!(button.settings.programmed_window, Some(saved("w2")));
		assert!(button.set_programmed_window(1, Some(&saved("w1")), Some(saved("w3"))));
		assert_eq!(button.settings.programmed_window, Some(saved("w3")));
		assert!(!button.set_programmed_window(1, Some(&saved("w2")), Some(saved("w3"))));
	}

	#[test]
	fn palette_edit_keeps_pin_but_switching_modes_clears_it() {
		let mut button = ButtonState::new(programmed("w1"), 1);
		assert!(!button.update_settings(&programmed("w1")));
		let mut palette = programmed("old");
		palette.palette = "builtin:blue".into();
		assert!(button.update_settings(&palette));
		assert_eq!(button.settings.palette, "builtin:blue");
		assert_eq!(button.settings.programmed_window, Some(saved("w1")));

		let mut focused = palette;
		focused.mode = "focused_window".into();
		assert!(button.update_settings(&focused));
		assert_eq!(button.settings.programmed_window, None);
		assert!(!button.set_programmed_window(1, None, Some(saved("w3"))));
	}
}
