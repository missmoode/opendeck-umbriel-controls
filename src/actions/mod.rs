use crate::{
	icons::application::ApplicationIcons, themes::ThemeManager, umbriel_observer::UmbrielObserver,
};
use std::sync::Arc;

mod buttons;
pub mod ipc;
mod press;
mod render_queue;
pub mod scratchpad;
pub mod screencast;
mod titles;
pub mod ui;
pub mod workspace;

pub async fn register(observer: &UmbrielObserver, themes: Arc<ThemeManager>) {
	let icons = Arc::new(ApplicationIcons::new());
	let ui = ui::ActionUi::start(Arc::clone(&themes));
	openaction::global_events::set_global_event_handler(Box::leak(Box::new(ui::GlobalEvents(
		Arc::clone(&ui),
	))));
	ipc::register(Arc::clone(&ui)).await;
	scratchpad::register(
		observer.windows(),
		Arc::clone(&icons),
		Arc::clone(&themes),
		Arc::clone(&ui),
	)
	.await;
	workspace::register(
		observer.workspaces(),
		observer.windows(),
		Arc::clone(&icons),
		Arc::clone(&themes),
		Arc::clone(&ui),
	)
	.await;
	screencast::register(observer, Arc::clone(&icons), themes, ui).await;
}

/// A replacement action can reuse the same OpenDeck instance ID. Check that
/// an async render still belongs to the original instance before sending it.
pub(crate) async fn instance_is_current(instance: &openaction::Instance) -> bool {
	openaction::get_instance(instance.instance_id.clone())
		.await
		.is_some_and(|current| std::ptr::eq(current.as_ref(), instance))
}
