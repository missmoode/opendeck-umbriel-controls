use std::{pin::Pin, time::Duration};

use tokio::time::{Instant, Sleep};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderPriority {
	Immediate,
	Deferred,
}

/// Coalesces background render requests while allowing time-sensitive
/// interaction feedback to flush immediately.
pub(crate) struct RenderQueue {
	deferred_delay: Duration,
	immediate_grace: Duration,
	pending: bool,
	immediate_until: Option<Instant>,
	sleep: Pin<Box<Sleep>>,
}

impl RenderQueue {
	pub(crate) fn new(deferred_delay: Duration, immediate_grace: Duration) -> Self {
		Self {
			deferred_delay,
			immediate_grace,
			pending: false,
			immediate_until: None,
			sleep: Box::pin(tokio::time::sleep(Duration::from_secs(24 * 60 * 60))),
		}
	}

	pub(crate) fn request(&mut self, priority: RenderPriority) -> bool {
		let now = Instant::now();
		self.prune_immediate(now);
		if self.immediate_until.is_some() {
			if priority == RenderPriority::Immediate {
				self.immediate_until = Some(now + self.immediate_grace);
			}
			self.clear_pending();
			return true;
		}
		match priority {
			RenderPriority::Immediate => {
				self.immediate_until = Some(now + self.immediate_grace);
				self.clear_pending();
				true
			}
			RenderPriority::Deferred => {
				if !self.pending {
					let deadline = now + self.deferred_delay;
					self.pending = true;
					self.sleep.as_mut().reset(deadline);
				}
				false
			}
		}
	}

	pub(crate) fn is_pending(&self) -> bool {
		self.pending
	}

	pub(crate) fn sleep(&mut self) -> Pin<&mut Sleep> {
		self.sleep.as_mut()
	}

	pub(crate) fn fire(&mut self) {
		self.clear_pending();
		self.prune_immediate(Instant::now());
	}

	fn clear_pending(&mut self) {
		self.pending = false;
		self.sleep
			.as_mut()
			.reset(Instant::now() + Duration::from_secs(24 * 60 * 60));
	}

	fn prune_immediate(&mut self, now: Instant) {
		if self.immediate_until.is_some_and(|until| now > until) {
			self.immediate_until = None;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tokio::time::Duration;

	#[tokio::test(start_paused = true)]
	async fn deferred_requests_coalesce_until_their_deadline() {
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));
		assert!(!queue.request(RenderPriority::Deferred));
		assert!(queue.is_pending());
		assert!(!queue.request(RenderPriority::Deferred));
		assert!(queue.is_pending());
		tokio::time::advance(Duration::from_millis(199)).await;
		assert!(!queue.sleep().is_elapsed());
		tokio::time::advance(Duration::from_millis(1)).await;
		assert!(queue.sleep().is_elapsed());
		queue.fire();
		assert!(!queue.is_pending());
	}

	#[tokio::test(start_paused = true)]
	async fn immediate_request_flushes_and_makes_following_events_immediate_briefly() {
		let mut queue = RenderQueue::new(Duration::from_millis(200), Duration::from_millis(250));
		assert!(queue.request(RenderPriority::Immediate));
		assert!(!queue.is_pending());
		assert!(queue.request(RenderPriority::Deferred));
		tokio::time::advance(Duration::from_millis(251)).await;
		assert!(!queue.request(RenderPriority::Deferred));
		assert!(queue.is_pending());
	}
}
