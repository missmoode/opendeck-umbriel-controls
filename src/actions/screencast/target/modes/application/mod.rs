mod custom;
mod picker;

use openaction::{Instance, OpenActionResult};
use umbriel_ipc::{Client, ScreencastCommand, Window};

use super::super::TargetAction;
use crate::{
	actions::screencast::model::{Display, Mode, Settings, Visual},
	icons::application::ApplicationIcons,
};

pub(in crate::actions::screencast::target) use custom::ProcessNames;
pub(in crate::actions::screencast::target) use picker::{send_choices, send_conversion};

pub(in crate::actions::screencast::target) fn needs_process_info(settings: &Settings) -> bool {
	custom::needs_process_info(settings)
}

pub(in crate::actions::screencast::target) fn process_names(windows: &[Window]) -> ProcessNames {
	custom::process_names(windows)
}

const RUNNING_PREFIX: &str = "app-id:";

enum Matcher<'a> {
	Installed(&'a str),
	Running(&'a str),
	Custom(custom::CustomMatcher),
}

impl<'a> Matcher<'a> {
	fn new(settings: &'a Settings) -> Result<Self, String> {
		match settings.application_source.as_str() {
			"installed" => {
				let selection = settings.application.trim();
				if let Some(id) = selection.strip_prefix(RUNNING_PREFIX) {
					Ok(Self::Running(id))
				} else {
					Ok(Self::Installed(selection))
				}
			}
			"custom" => Ok(Self::Custom(custom::CustomMatcher::from_settings(
				settings,
			)?)),
			_ => Err("Unknown application source".into()),
		}
	}

	fn matches(&self, apps: &ApplicationIcons, window: &Window, processes: &ProcessNames) -> bool {
		if window.id.is_empty() {
			return false;
		}
		match self {
			Self::Installed(id) => apps.matches_desktop_window(id, &window.app_id),
			Self::Running(app_id) => !app_id.is_empty() && window.app_id == *app_id,
			Self::Custom(matcher) => matcher.matches_window(window, processes),
		}
	}

	fn first<'w>(
		&self,
		apps: &ApplicationIcons,
		windows: &'w [Window],
		processes: &ProcessNames,
	) -> Option<&'w Window> {
		match self {
			Self::Installed(_) | Self::Running(_) => windows
				.iter()
				.filter(|window| self.matches(apps, window, processes))
				.min_by(|a, b| a.id.cmp(&b.id)),
			Self::Custom(matcher) => matcher.matching_window(windows, processes),
		}
	}
}

fn matching_window<'a>(
	apps: &ApplicationIcons,
	settings: &Settings,
	windows: &'a [Window],
	processes: &ProcessNames,
) -> Result<Option<&'a Window>, String> {
	Ok(Matcher::new(settings)?.first(apps, windows, processes))
}

pub(in crate::actions::screencast::target) fn display_for(
	settings: &Settings,
	command: Option<&ScreencastCommand>,
	windows: Option<&[Window]>,
	apps: &ApplicationIcons,
	processes: &ProcessNames,
) -> Display {
	let rule = Matcher::new(settings).ok();
	let matching = windows.and_then(|windows| {
		rule.as_ref()
			.and_then(|rule| rule.first(apps, windows, processes))
	});
	let running = windows.map(|_| matching.is_some());
	let selected = windows.is_some_and(|windows| {
		let Some(ScreencastCommand::Window { identifier, .. }) = command else {
			return false;
		};
		rule.as_ref().is_some_and(|rule| {
			windows
				.iter()
				.any(|window| window.id == *identifier && rule.matches(apps, window, processes))
		})
	});
	let running_id = if settings.application_source == "installed" {
		settings.application.trim().strip_prefix(RUNNING_PREFIX)
	} else {
		None
	};
	let main = if settings.application_source == "custom" || running_id.is_some() {
		Visual::CustomApplication {
			key: running_id
				.map_or_else(|| custom::cache_key(settings), |id| format!("running:{id}")),
			app_id: matching
				.map(|window| window.app_id.clone())
				.or_else(|| running_id.map(str::to_owned)),
			running,
		}
	} else {
		Visual::Application {
			desktop_id: settings.application.trim().to_owned(),
			running,
		}
	};
	Display {
		selected,
		accepted_inset: false,
		pinned: false,
		main,
		inset: None,
	}
}

fn same_matcher(a: &Settings, b: &Settings) -> bool {
	a.mode == b.mode
		&& a.application == b.application
		&& a.application_source == b.application_source
		&& a.custom_field == b.custom_field
		&& a.custom_pattern == b.custom_pattern
		&& a.custom_case_sensitive == b.custom_case_sensitive
}

impl TargetAction {
	pub(in crate::actions::screencast::target) async fn select_application(
		&self,
		instance: &Instance,
	) -> OpenActionResult<()> {
		let session = *self.session.borrow();
		let Some(button) = self.buttons.get(&instance.instance_id).await else {
			return Ok(());
		};
		if Mode::from_setting(&button.settings.mode) != Mode::DefinedApplication {
			return Ok(());
		}
		let client = match Client::from_environment() {
			Ok(client) => client,
			Err(error) => {
				eprintln!("Screencast application IPC unavailable: {error}");
				instance.show_alert().await?;
				return Ok(());
			}
		};
		let windows = client.list_windows().await;
		if !crate::actions::instance_is_current(instance).await || *self.session.borrow() != session
		{
			return Ok(());
		}
		let Some(current) = self.buttons.get(&instance.instance_id).await else {
			return Ok(());
		};
		if !same_matcher(&current.settings, &button.settings) {
			return Ok(());
		}
		match windows {
			Ok(windows) => {
				let processes = if needs_process_info(&current.settings) {
					let copy = windows.clone();
					tokio::task::spawn_blocking(move || process_names(&copy))
						.await
						.unwrap_or_default()
				} else {
					ProcessNames::new()
				};
				match matching_window(&self.applications, &current.settings, &windows, &processes) {
					Ok(Some(window)) => {
						if !crate::actions::instance_is_current(instance).await
							|| *self.session.borrow() != session
						{
							return Ok(());
						}
						if let Some(latest) = self.buttons.get(&instance.instance_id).await {
							if !same_matcher(&latest.settings, &current.settings) {
								return Ok(());
							}
						} else {
							return Ok(());
						}
						if let Err(error) = client.screencast_set_window(Some(&window.id)).await {
							eprintln!("Could not select screencast application: {error}");
							instance.show_alert().await?;
						} else {
							self.buttons.refresh(&instance.instance_id).await;
						}
					}
					Ok(None) => {
						instance.show_alert().await?;
					}
					Err(error) => {
						eprintln!("Invalid custom screencast matcher: {error}");
						instance.show_alert().await?;
					}
				}
			}
			Err(error) => {
				eprintln!("Could not list Umbriel windows: {error}");
				instance.show_alert().await?;
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests;
