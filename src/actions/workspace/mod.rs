mod layout;
mod switch;
use crate::umbriel_observer::{WindowUpdates, WorkspaceUpdates};
use crate::{actions::ui::ActionUi, icons::application::ApplicationIcons, themes::ThemeManager};
use std::sync::Arc;

pub async fn register(
	workspaces: WorkspaceUpdates,
	windows: WindowUpdates,
	icons: Arc<ApplicationIcons>,
	themes: Arc<ThemeManager>,
	ui: Arc<ActionUi>,
) {
	layout::register(workspaces.clone(), Arc::clone(&themes), Arc::clone(&ui)).await;
	switch::register(workspaces, windows, icons, themes, ui).await;
}
