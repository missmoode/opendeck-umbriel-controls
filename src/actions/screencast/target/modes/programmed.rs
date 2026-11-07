use openaction::{Action, Instance, OpenActionResult};
use umbriel_ipc::{Client, Window};

use super::super::TargetAction;
use crate::actions::{
	press::{PressGuard, PressKind},
	screencast::model::{self, Mode, PressIntent, RememberedWindow},
};

impl TargetAction {
	pub(in crate::actions::screencast::target) async fn store_programmed_window(
		&self,
		instance: &Instance,
		session: u64,
		expected: Option<&RememberedWindow>,
		replacement: Option<RememberedWindow>,
		guard: Option<&PressGuard>,
	) -> OpenActionResult<()> {
		let _write = self.settings_writes.lock().await;
		if let Some(guard) = guard
			&& !guard.is_current().await
		{
			return Ok(());
		}
		if *self.session.borrow() != session || !crate::actions::instance_is_current(instance).await
		{
			return Ok(());
		}
		let mut saved = None;
		self.buttons
			.change(&instance.instance_id, |button| {
				if !button.set_programmed_window(session, expected, replacement) {
					return false;
				}
				saved = Some(button.settings.clone());
				true
			})
			.await;
		if let Some(settings) = saved
			&& *self.session.borrow() == session
			&& crate::actions::instance_is_current(instance).await
		{
			instance.set_settings(&settings).await?;
		}
		Ok(())
	}

	pub(in crate::actions::screencast::target) async fn programmable_press(
		&self,
		instance: &Instance,
		kind: PressKind,
		guard: &PressGuard,
	) -> OpenActionResult<()> {
		if !crate::actions::instance_is_current(instance).await || !guard.is_current().await {
			return Ok(());
		}
		let session = *self.session.borrow();
		if self
			.buttons
			.change(&instance.instance_id, |button| {
				button.update_session(session)
			})
			.await
		{
			// The press began in a previous Umbriel session.
			return Ok(());
		}
		let Some(button) = self.buttons.get(&instance.instance_id).await else {
			return Ok(());
		};
		if Mode::from_setting(&button.settings.mode) != Mode::ProgrammedWindow {
			return Ok(());
		}
		let intent = model::classify_press(kind, button.settings.programmed_window.as_ref());

		if intent == PressIntent::Unassigned {
			// A tap cannot silently fall back to sharing the focused window.
			instance.show_alert().await?;
			return Ok(());
		}

		let client = match Client::from_environment() {
			Ok(client) => client,
			Err(error) => {
				eprintln!("Screencast preset IPC unavailable: {error}");
				instance.show_alert().await?;
				return Ok(());
			}
		};

		let result = match intent {
			PressIntent::Assign => {
				let focused = client.focused_window().await;
				if !crate::actions::instance_is_current(instance).await || !guard.is_current().await
				{
					return Ok(());
				}
				match focused {
					Ok(window) if !window.id.is_empty() && *self.session.borrow() == session => {
						let remembered = RememberedWindow::from(&window);
						if let Err(error) = self
							.store_programmed_window(
								instance,
								session,
								button.settings.programmed_window.as_ref(),
								Some(remembered),
								Some(guard),
							)
							.await
						{
							eprintln!("Could not save programmed screencast target: {error}");
							instance.show_alert().await?;
						}
						Ok(())
					}
					Ok(_) => {
						instance.show_alert().await?;
						return Ok(());
					}
					Err(error) => Err(error),
				}
			}
			PressIntent::Select(remembered) => {
				let windows = client.list_windows().await;
				if !crate::actions::instance_is_current(instance).await || !guard.is_current().await
				{
					return Ok(());
				}
				match windows {
					Ok(windows)
						if *self.session.borrow() == session
							&& model::is_present(&remembered, &windows) =>
					{
						client.screencast_set_window(Some(&remembered.id)).await
					}
					Ok(_) => {
						// The live inventory confirmed that the old target is gone.
						if let Err(error) = self
							.store_programmed_window(
								instance,
								session,
								Some(&remembered),
								None,
								Some(guard),
							)
							.await
						{
							eprintln!("Could not clear missing screencast target: {error}");
						}
						instance.show_alert().await?;
						return Ok(());
					}
					Err(error) => Err(error),
				}
			}
			PressIntent::Unassigned => unreachable!(),
		};
		if let Err(error) = result {
			eprintln!("Screencast preset action failed: {error}");
			instance.show_alert().await?;
		}
		Ok(())
	}
	pub(in crate::actions::screencast::target) async fn key_down_programmed(
		&self,
		instance: &Instance,
	) -> OpenActionResult<()> {
		let session = *self.session.borrow();
		let Some(button) = self.buttons.get(&instance.instance_id).await else {
			return Ok(());
		};
		if button.session != session {
			self.buttons
				.change(&instance.instance_id, |button| {
					button.update_session(session)
				})
				.await;
		}

		let action = self.clone();
		self.pressed
			.arm(
				instance,
				model::HOLD_THRESHOLD,
				move |original, guard| async move {
					let current_session = *action.session.borrow();
					if current_session == session
						&& let Err(error) = action
							.programmable_press(&original, PressKind::Hold, &guard)
							.await
					{
						eprintln!("Screencast hold failed: {error}");
					}
				},
			)
			.await;
		Ok(())
	}

	pub(in crate::actions::screencast::target) async fn key_up_programmed(
		&self,
		instance: &Instance,
	) -> OpenActionResult<()> {
		if let Some((kind, guard)) = self
			.pressed
			.up(&instance.instance_id, model::HOLD_THRESHOLD)
			.await
		{
			self.programmable_press(instance, kind, &guard).await?;
		}
		Ok(())
	}

	pub(in crate::actions::screencast::target) async fn invalidate_closed(
		&self,
		windows: &[Window],
		current_session: u64,
	) {
		// Only an observed inventory warrants checking for a closed target.
		// Confirm live: the observer snapshot can lag a new assignment.
		let missing = self
			.buttons
			.snapshot()
			.await
			.into_iter()
			.filter_map(|button| {
				let remembered = button.value.settings.programmed_window.as_ref()?;
				(Mode::from_setting(&button.value.settings.mode) == Mode::ProgrammedWindow
					&& !model::is_present(remembered, windows))
				.then(|| (button.id, remembered.clone()))
			})
			.collect::<Vec<_>>();
		if !missing.is_empty()
			&& let Ok(client) = Client::from_environment()
			&& let Ok(live) = client.list_windows().await
		{
			for (id, remembered) in missing {
				if model::is_present(&remembered, &live) {
					continue;
				}
				let instance = match openaction::get_instance(id).await {
					Some(instance) if instance.action_uuid == TargetAction::UUID => instance,
					_ => continue,
				};
				if let Err(error) = self
					.store_programmed_window(
						&instance,
						current_session,
						Some(&remembered),
						None,
						None,
					)
					.await
				{
					eprintln!("Could not clear closed screencast target: {error}");
				}
			}
		}
	}
}
