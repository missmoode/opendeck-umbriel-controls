//! Per-key tap/hold timing and press identity, independent of IPC commands.
use std::{
	collections::HashMap,
	future::Future,
	sync::Arc,
	time::{Duration, Instant},
};

use openaction::Instance;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressKind {
	Tap,
	Hold,
}

#[derive(Default)]
struct PressState {
	next_generation: u64,
	slots: HashMap<String, PressSlot>,
}

struct PressSlot {
	generation: u64,
	pressed: Option<TimedPress>,
}

struct TimedPress {
	started: Instant,
	fired: bool,
}

/// Identifies one press, including asynchronous work that outlives key release.
/// A new press or explicit cancellation makes an older guard stale.
#[derive(Clone)]
pub struct PressGuard {
	presses: TimedPresses,
	id: String,
	generation: u64,
}

impl PressGuard {
	pub async fn is_current(&self) -> bool {
		self.presses
			.0
			.lock()
			.await
			.slots
			.get(&self.id)
			.is_some_and(|slot| slot.generation == self.generation)
	}
}

/// A fired hold remains current after release until a new press or cancellation.
/// Timer callbacks and release callbacks receive the same freshness guard.
#[derive(Clone, Default)]
pub struct TimedPresses(Arc<Mutex<PressState>>);

impl TimedPresses {
	/// Arm a hold for the original OpenDeck key instance. Releasing early cancels
	/// the timer; a fired hold can finish asynchronously after release.
	pub async fn arm<F, Fut>(&self, instance: &Instance, threshold: Duration, on_hold: F)
	where
		F: FnOnce(Arc<Instance>, PressGuard) -> Fut + Send + 'static,
		Fut: Future<Output = ()> + Send + 'static,
	{
		let id = instance.instance_id.clone();
		let Some((guard, started)) = self.down(&id).await else {
			return;
		};
		let original = openaction::get_instance(id).await;
		let Some(original) = original.filter(|original| std::ptr::eq(original.as_ref(), instance))
		else {
			self.cancel_if_current(&guard).await;
			return;
		};
		let pressed = self.clone();
		tokio::spawn(async move {
			tokio::time::sleep_until(tokio::time::Instant::from_std(started + threshold)).await;
			if pressed.hold(&guard).await && super::instance_is_current(&original).await {
				on_hold(original, guard).await;
			}
		});
	}

	/// Return the new press identity, unless the key is already down.
	async fn down(&self, id: &str) -> Option<(PressGuard, Instant)> {
		let mut state = self.0.lock().await;
		if state
			.slots
			.get(id)
			.is_some_and(|slot| slot.pressed.is_some())
		{
			return None;
		}
		state.next_generation = state.next_generation.wrapping_add(1);
		let generation = state.next_generation;
		let started = Instant::now();
		state.slots.insert(
			id.to_owned(),
			PressSlot {
				generation,
				pressed: Some(TimedPress {
					started,
					fired: false,
				}),
			},
		);
		Some((
			PressGuard {
				presses: self.clone(),
				id: id.to_owned(),
				generation,
			},
			started,
		))
	}

	/// Claim the hold once, only for its own still-depressed key.
	async fn hold(&self, guard: &PressGuard) -> bool {
		let mut state = self.0.lock().await;
		let Some(slot) = state.slots.get_mut(&guard.id) else {
			return false;
		};
		if slot.generation != guard.generation {
			return false;
		}
		let Some(press) = slot.pressed.as_mut() else {
			return false;
		};
		if press.fired {
			return false;
		}
		press.fired = true;
		true
	}

	/// A fired hold is already being handled by its timer, not by release.
	/// Keep its guard valid until another press begins or the key is cancelled.
	pub async fn up(&self, id: &str, threshold: Duration) -> Option<(PressKind, PressGuard)> {
		let mut state = self.0.lock().await;
		let slot = state.slots.get_mut(id)?;
		let press = slot.pressed.take()?;
		let kind = if press.fired {
			return None;
		} else if press.started.elapsed() >= threshold {
			PressKind::Hold
		} else {
			PressKind::Tap
		};
		Some((
			kind,
			PressGuard {
				presses: self.clone(),
				id: id.to_owned(),
				generation: slot.generation,
			},
		))
	}

	async fn cancel_if_current(&self, guard: &PressGuard) {
		let mut state = self.0.lock().await;
		if state
			.slots
			.get(&guard.id)
			.is_some_and(|slot| slot.generation == guard.generation)
		{
			state.slots.remove(&guard.id);
		}
	}

	/// Invalidates both a held key and any in-flight callback from its last press.
	pub async fn cancel(&self, id: &str) {
		self.0.lock().await.slots.remove(id);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn timer_claims_hold_once_and_release_does_not_invalidate_fired_callback() {
		let presses = TimedPresses::default();
		let (first, _) = presses.down("key").await.unwrap();
		assert!(presses.hold(&first).await);
		assert!(!presses.hold(&first).await);
		assert!(presses.up("key", Duration::ZERO).await.is_none());
		assert!(first.is_current().await);

		let (second, _) = presses.down("key").await.unwrap();
		assert!(!first.is_current().await);
		assert!(!presses.hold(&first).await);
		assert!(second.is_current().await);
		assert_eq!(
			presses
				.up("key", Duration::from_secs(60))
				.await
				.map(|(kind, _)| kind),
			Some(PressKind::Tap)
		);
		assert!(!presses.hold(&second).await);
	}

	#[tokio::test]
	async fn new_press_rejects_earlier_async_completion_even_when_released() {
		let presses = TimedPresses::default();
		let (older, _) = presses.down("key").await.unwrap();
		presses.hold(&older).await;
		presses.up("key", Duration::ZERO).await;
		let (newer, _) = presses.down("key").await.unwrap();
		presses.hold(&newer).await;
		presses.up("key", Duration::ZERO).await;

		// Complete the newer callback before the older blocked callback.
		let (release_old, old_ready) = tokio::sync::oneshot::channel::<()>();
		let older_result = tokio::spawn(async move {
			let _ = old_ready.await;
			older.is_current().await.then_some("older")
		});
		let mut saved = newer.is_current().await.then_some("newer");
		release_old.send(()).unwrap();
		if let Some(old) = older_result.await.unwrap() {
			saved = Some(old);
		}
		assert_eq!(saved, Some("newer"));
		presses.cancel("key").await;
		assert!(!newer.is_current().await);
	}

	#[tokio::test]
	async fn cancel_does_not_invalidate_other_keys_or_replacement() {
		let presses = TimedPresses::default();
		let (first, _) = presses.down("a").await.unwrap();
		let (other, _) = presses.down("b").await.unwrap();
		presses.cancel("a").await;
		let (replacement, _) = presses.down("a").await.unwrap();
		presses.cancel_if_current(&first).await;
		assert!(!first.is_current().await);
		assert!(replacement.is_current().await);
		assert!(other.is_current().await);
	}
}
