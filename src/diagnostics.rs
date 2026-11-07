use crate::umbriel_observer::UmbrielObserver;

pub fn start(observer: &UmbrielObserver) {
	let mut diagnostic = observer.workspaces();

	tokio::spawn(async move {
		while diagnostic.changed().await.is_ok() {
			let snapshot = diagnostic.borrow_and_update().clone();

			if let Some(workspaces) = snapshot {
				let active: Vec<_> = workspaces
					.iter()
					.filter(|workspace| workspace.active)
					.map(|workspace| format!("{}: {}", workspace.output, workspace.name))
					.collect();

				eprintln!("Umbriel Controls active workspaces: {}", active.join(", "));
			} else {
				eprintln!("Umbriel Controls workspace state unavailable");
			}
		}
	});
}
