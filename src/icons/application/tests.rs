//! Application resolution and refresh regression tests.
use super::*;

fn entry(dir: &Path, filename: &str, name: &str, wm_class: Option<&str>) -> DesktopEntry {
	let path = dir.join(format!("{filename}.desktop"));
	let wm = wm_class.map_or(String::new(), |wm| format!("StartupWMClass={wm}\n"));
	std::fs::write(
		&path,
		format!("[Desktop Entry]\nType=Application\nName={name}\n{wm}"),
	)
	.unwrap();
	DesktopEntry::from_path(&path, Some(&[] as &[&str])).unwrap()
}

#[test]
fn matching_prefers_exact_desktop_id_and_rejects_ambiguous_class() {
	let dir = tempfile::tempdir().unwrap();
	let main = entry(
		dir.path(),
		"org.example.Editor",
		"Editor",
		Some("editor-window"),
	);
	let other = entry(
		dir.path(),
		"org.example.EditorBeta",
		"Editor Beta",
		Some("editor-window"),
	);
	let entries = vec![main, other];
	assert!(matches_desktop_window(
		&entries,
		"org.example.Editor",
		"org.example.Editor"
	));
	assert!(matches_desktop_window(
		&entries,
		"org.example.EditorBeta",
		"org.example.EditorBeta"
	));
	assert!(!matches_desktop_window(
		&entries,
		"org.example.Editor",
		"editor-window"
	));
	assert!(!matches_desktop_window(
		&entries,
		"org.example.Editor",
		"editor"
	));
	assert!(!matches_desktop_window(&entries, "org.example.Editor", ""));
}

#[test]
fn matching_uses_unique_startup_class_and_choices_are_sorted() {
	let dir = tempfile::tempdir().unwrap();
	let alpha = entry(
		dir.path(),
		"org.example.alpha",
		"Alpha",
		Some("alpha-window"),
	);
	let zebra = entry(dir.path(), "org.example.zebra", "Zebra", None);
	let entries = vec![zebra, alpha];
	assert!(matches_desktop_window(
		&entries,
		"org.example.alpha",
		"ALPHA-WINDOW"
	));
	assert_eq!(
		application_choices(&entries),
		vec![
			("org.example.alpha".into(), "Alpha".into()),
			("org.example.zebra".into(), "Zebra".into())
		]
	);
}

#[test]
fn desktop_file_icon_name_can_differ_from_app_id() {
	let directory = tempfile::tempdir().unwrap();
	let file = directory.path().join("org.example.Writer.desktop");
	std::fs::write(
		&file,
		"[Desktop Entry]\nType=Application\nName=Writer\nIcon=writer-logo\n",
	)
	.unwrap();
	let locales: &[&str] = &[];
	let entry = DesktopEntry::from_path(&file, Some(locales)).unwrap();
	assert_eq!(
		icon_name_for(&[entry], "org.example.Writer"),
		Some("writer-logo")
	);
}
#[test]
fn catalogue_refresh_invalidates_icon_cache_and_notifies_renderers() {
	let resolver = ApplicationIcons::with_test_entries(vec![]);
	let mut updates = resolver.subscribe();
	resolver
		.cache
		.lock()
		.unwrap()
		.images
		.insert("test".into(), None);
	resolver.replace_entries(vec![]);
	assert!(updates.has_changed().unwrap());
	assert_eq!(*updates.borrow_and_update(), 1);
	assert!(resolver.cache.lock().unwrap().images.is_empty());
}

#[test]
fn lookup_started_before_refresh_cannot_restore_an_old_cache_entry() {
	let resolver = ApplicationIcons::with_test_entries(vec![]);
	let mut attempts = 0;
	let resolved = resolver.cached_icon("game", || {
		attempts += 1;
		if attempts == 1 {
			resolver.replace_entries(vec![]);
			Some("outdated".to_owned())
		} else {
			Some("current".to_owned())
		}
	});
	assert_eq!(attempts, 2);
	assert_eq!(resolved.as_deref(), Some("current"));
	assert_eq!(
		resolver.cached_icon("game", || panic!("cache miss")),
		resolved
	);
}

#[test]
fn custom_icon_cache_survives_a_window_disappearing() {
	let resolver = ApplicationIcons::with_test_entries(vec![]);
	resolver
		.custom_cache
		.lock()
		.unwrap()
		.insert("my-rule".into(), "data:image/png;base64,AAAA".into());
	assert_eq!(
		resolver.for_custom("my-rule", None),
		Some("data:image/png;base64,AAAA".into())
	);
	assert_eq!(resolver.for_custom("other-rule", None), None);
}
