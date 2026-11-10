use crate::umbriel_observer::UmbrielObserver;

pub fn start(observer: &UmbrielObserver) {
	let mut diagnostic = observer.workspaces();

	tokio::spawn(async move {
		let mut previous_message = None::<String>;
		while diagnostic.changed().await.is_ok() {
			let snapshot = diagnostic.borrow_and_update().clone();

			let message = if let Some(workspaces) = snapshot {
				let active: Vec<_> = workspaces
					.iter()
					.filter(|workspace| workspace.active)
					.map(|workspace| format!("{}: {}", workspace.output, workspace.name))
					.collect();

				format!("Umbriel Controls active workspaces: {}", active.join(", "))
			} else {
				"Umbriel Controls workspace state unavailable".to_owned()
			};
			if previous_message.as_ref() != Some(&message) {
				eprintln!("{message}");
				previous_message = Some(message);
			}
		}
	});
}
