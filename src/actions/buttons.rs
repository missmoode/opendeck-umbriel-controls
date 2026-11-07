//! Per-action button state and targeted image invalidation.
use std::{
	collections::HashMap,
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::Duration,
};

use openaction::Instance;
use tokio::sync::{Mutex, watch};

struct Entry<T> {
	value: T,
	generation: u64,
}

pub(crate) struct Button<T> {
	pub id: String,
	pub value: T,
	pub generation: u64,
}

pub(crate) struct TrackedButtons<T> {
	entries: Arc<Mutex<HashMap<String, Entry<T>>>>,
	wake: watch::Sender<u64>,
	next_generation: Arc<AtomicU64>,
}

impl<T> Clone for TrackedButtons<T> {
	fn clone(&self) -> Self {
		Self {
			entries: Arc::clone(&self.entries),
			wake: self.wake.clone(),
			next_generation: Arc::clone(&self.next_generation),
		}
	}
}

impl<T> TrackedButtons<T> {
	pub fn new() -> (Self, watch::Receiver<u64>) {
		let (wake, receiver) = watch::channel(0);
		(
			Self {
				entries: Arc::new(Mutex::new(HashMap::new())),
				wake,
				next_generation: Arc::new(AtomicU64::new(0)),
			},
			receiver,
		)
	}

	pub fn subscribe(&self) -> watch::Receiver<u64> {
		self.wake.subscribe()
	}

	fn generation(&self) -> u64 {
		self.next_generation
			.fetch_add(1, Ordering::Relaxed)
			.wrapping_add(1)
	}

	fn notify(&self) {
		self.wake
			.send_modify(|revision| *revision = revision.wrapping_add(1));
	}

	pub async fn put(&self, id: &str, value: T) {
		let mut entries = self.entries.lock().await;
		let generation = self.generation();
		entries.insert(id.to_owned(), Entry { value, generation });
		drop(entries);
		self.notify();
	}

	/// Update existing state without discarding action-specific runtime fields.
	pub async fn upsert(&self, id: &str, create: impl FnOnce() -> T, update: impl FnOnce(&mut T)) {
		let mut entries = self.entries.lock().await;
		match entries.get_mut(id) {
			Some(entry) => {
				update(&mut entry.value);
				entry.generation = self.generation();
			}
			None => {
				entries.insert(
					id.to_owned(),
					Entry {
						value: create(),
						generation: self.generation(),
					},
				);
			}
		}
		drop(entries);
		self.notify();
	}

	/// The closure reports whether it changed the stored state and needs a new image.
	pub async fn change(&self, id: &str, update: impl FnOnce(&mut T) -> bool) -> bool {
		let mut entries = self.entries.lock().await;
		let changed = entries.get_mut(id).is_some_and(|entry| {
			if update(&mut entry.value) {
				entry.generation = self.generation();
				true
			} else {
				false
			}
		});
		drop(entries);
		if changed {
			self.notify();
		}
		changed
	}

	pub async fn get(&self, id: &str) -> Option<T>
	where
		T: Clone,
	{
		self.entries
			.lock()
			.await
			.get(id)
			.map(|entry| entry.value.clone())
	}

	pub async fn remove(&self, id: &str) {
		if self.entries.lock().await.remove(id).is_some() {
			self.notify();
		}
	}

	/// A worker's obsolete snapshot must not remove a newer occupant of the slot.
	pub async fn remove_if_generation(&self, id: &str, generation: u64) {
		let mut entries = self.entries.lock().await;
		if entries
			.get(id)
			.is_some_and(|entry| entry.generation == generation)
		{
			entries.remove(id);
			drop(entries);
			self.notify();
		}
	}

	pub async fn snapshot(&self) -> Vec<Button<T>>
	where
		T: Clone,
	{
		self.entries
			.lock()
			.await
			.iter()
			.map(|(id, entry)| Button {
				id: id.clone(),
				value: entry.value.clone(),
				generation: entry.generation,
			})
			.collect()
	}

	/// Verify that neither the SDK instance nor its settings changed during rendering.
	pub async fn may_render(&self, instance: &Instance, generation: u64) -> bool {
		super::instance_is_current(instance).await
			&& self
				.entries
				.lock()
				.await
				.get(&instance.instance_id)
				.is_some_and(|entry| entry.generation == generation)
	}

	pub async fn refresh(&self, id: &str) {
		let mut entries = self.entries.lock().await;
		let Some(entry) = entries.get_mut(id) else {
			return;
		};
		entry.generation = self.generation();
		drop(entries);
		self.notify();
	}

	/// OpenDeck can install a key in its editor after the first image arrives.
	/// Only retry the original key, not every key of this action type.
	pub async fn retry_initial_icon(&self, instance: &Instance)
	where
		T: Send + 'static,
	{
		let id = instance.instance_id.clone();
		let Some(original) = openaction::get_instance(id.clone()).await else {
			return;
		};
		if !std::ptr::eq(original.as_ref(), instance) {
			return;
		}
		let buttons = self.clone();
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			if super::instance_is_current(&original).await {
				buttons.refresh(&id).await;
			}
		});
	}
}

/// Tracks successful renders independently for each button and render model.
pub(crate) struct Rendered<K>(HashMap<String, (u64, K)>);

impl<K: PartialEq> Rendered<K> {
	pub fn new() -> Self {
		Self(HashMap::new())
	}

	pub fn needs_render(&self, id: &str, generation: u64, key: &K) -> bool {
		!self
			.0
			.get(id)
			.is_some_and(|(rendered_generation, rendered_key)| {
				*rendered_generation == generation && rendered_key == key
			})
	}

	pub fn mark_rendered(&mut self, id: String, generation: u64, key: K) {
		self.0.insert(id, (generation, key));
	}

	pub fn forget(&mut self, id: &str) {
		self.0.remove(id);
	}

	pub fn retain<T>(&mut self, buttons: &[Button<T>]) {
		self.0
			.retain(|id, _| buttons.iter().any(|button| &button.id == id));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn targeted_changes_and_replacement_keep_distinct_generations() {
		let (buttons, _updates) = TrackedButtons::new();
		buttons.put("a", 1).await;
		buttons.put("b", 2).await;
		let before = buttons.snapshot().await;
		let a = before
			.iter()
			.find(|entry| entry.id == "a")
			.unwrap()
			.generation;
		let b = before
			.iter()
			.find(|entry| entry.id == "b")
			.unwrap()
			.generation;

		buttons.put("a", 3).await;
		let changed = buttons.snapshot().await;
		assert_ne!(
			changed
				.iter()
				.find(|entry| entry.id == "a")
				.unwrap()
				.generation,
			a
		);
		assert_eq!(
			changed
				.iter()
				.find(|entry| entry.id == "b")
				.unwrap()
				.generation,
			b
		);

		buttons.remove("a").await;
		buttons.put("a", 4).await;
		buttons.remove_if_generation("a", a).await;
		let replacement = buttons.snapshot().await;
		let new_a = replacement.iter().find(|entry| entry.id == "a").unwrap();
		assert_eq!(new_a.value, 4);
		assert_ne!(new_a.generation, a);
	}
}
