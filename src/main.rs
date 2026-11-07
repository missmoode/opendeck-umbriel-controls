mod actions;
mod icons;
mod themes;
mod umbriel_observer;

#[cfg(feature = "diagnostics")]
mod diagnostics;

use openaction::OpenActionResult;

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	let themes = themes::ThemeManager::start();
	let observer = umbriel_observer::UmbrielObserver::start();

	#[cfg(feature = "diagnostics")]
	diagnostics::start(&observer);

	actions::register(&observer, themes).await;
	openaction::run(std::env::args().collect()).await
}
