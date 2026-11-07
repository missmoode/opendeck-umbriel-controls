mod clear;
mod icon;
pub mod model;
mod target;

use crate::{
	actions::ui::ActionUi, icons::application::ApplicationIcons, themes::ThemeManager,
	umbriel_observer::UmbrielObserver,
};
use std::sync::Arc;

pub async fn register(
	observer: &UmbrielObserver,
	icons: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	ui: Arc<ActionUi>,
) {
	target::register(
		observer.windows(),
		observer.outputs(),
		observer.screencast(),
		observer.session(),
		icons,
		Arc::clone(&themes),
		Arc::clone(&ui),
	)
	.await;
	clear::register(themes, ui).await;
}
