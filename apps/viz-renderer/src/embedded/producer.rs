//! Bounded ownership of images crossing the process boundary. No completion here is a display event.
use std::time::{Duration, Instant};
use viz_helper::protocol::{FrameId, SHARED_FRAME_SLOTS};

const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Slot {
    Free,
    Rendering(FrameId),
    Desk(FrameId),
}

pub(super) struct Producer {
    generation: u64,
    sequence: u64,
    slots: [Slot; SHARED_FRAME_SLOTS],
    count: usize,
    visible: bool,
    next: Instant,
    flow: Option<super::flow::Flow>,
}

impl Producer {
    pub fn new(shared: bool, now: Instant) -> Self {
        Self {
            generation: 1,
            sequence: 0,
            slots: [Slot::Free; SHARED_FRAME_SLOTS],
            count: if shared { SHARED_FRAME_SLOTS } else { 1 },
            visible: true,
            next: now,
            flow: None,
        }
    }

    pub(super) fn with_flow(mut self, flow: Option<super::flow::Flow>) -> Self {
        self.flow = flow;
        self
    }
    pub(super) fn ready_send_returned(&mut self, frame: FrameId) {
        if let Some(flow) = self.flow.as_mut() {
            flow.frame("ready_send_returned", frame, None);
        }
    }
    /// True once on a hidden-to-visible edge; the caller invalidates its successful image stamp.
    pub fn set_visible(&mut self, visible: bool, now: Instant) -> bool {
        let resumed = visible && !self.visible;
        self.visible = visible;
        if !visible && let Some(flow) = self.flow.as_mut() {
            flow.wait(now, false);
        }
        if resumed {
            self.next = now;
        }
        resumed
    }

    pub fn available(&self, now: Instant) -> bool {
        self.visible && now >= self.next && self.slots[..self.count].contains(&Slot::Free)
    }

    /// A due slot is reserved before expensive frame preparation. Even a static skip is paced.
    pub fn sleep_for(&self, now: Instant) -> Duration {
        let poll = Duration::from_millis(2);
        if self.visible && self.slots[..self.count].contains(&Slot::Free) && self.next > now {
            poll.min(self.next.saturating_duration_since(now))
        } else {
            poll
        }
    }

    pub fn begin(&mut self, now: Instant) -> Option<FrameId> {
        if let Some(flow) = self.flow.as_mut() {
            flow.wait(
                now,
                self.visible && now >= self.next && !self.slots[..self.count].contains(&Slot::Free),
            );
        }
        if !self.available(now) {
            return None;
        }
        let slot = self.slots[..self.count]
            .iter()
            .position(|state| *state == Slot::Free)?;
        // Preserve phase for ordinary wake jitter. After a whole missed slot, re-anchor so a
        // long non-integral stall cannot produce a near-immediate catch-up pair.
        let lateness = now.saturating_duration_since(self.next);
        self.next = if lateness >= FRAME_INTERVAL {
            now + FRAME_INTERVAL
        } else {
            self.next + FRAME_INTERVAL
        };
        let frame = FrameId {
            generation: self.generation,
            slot: slot as u8,
            sequence: self.sequence.checked_add(1)?,
        };
        self.slots[slot] = Slot::Rendering(frame);
        if let Some(flow) = self.flow.as_mut() {
            flow.frame("begin_reserved", frame, None);
        }
        Some(frame)
    }

    pub fn cancel(&mut self, frame: FrameId) {
        if self.slots.get(frame.slot as usize) == Some(&Slot::Rendering(frame)) {
            self.slots[frame.slot as usize] = Slot::Free;
            if let Some(flow) = self.flow.as_mut() {
                flow.frame("reservation_cancelled", frame, None);
            }
        }
    }

    pub fn submitted(&mut self, frame: FrameId) {
        self.sequence = frame.sequence;
        if let Some(flow) = self.flow.as_mut() {
            flow.frame("gpu_submission_returned", frame, None);
        }
    }

    /// A GPU callback can arrive late after a resize; only its exact reservation can become ready.
    pub fn completed(&mut self, frame: FrameId) -> bool {
        if self.slots.get(frame.slot as usize) != Some(&Slot::Rendering(frame)) {
            return false;
        }
        self.slots[frame.slot as usize] = Slot::Desk(frame);
        if let Some(flow) = self.flow.as_mut() {
            flow.frame("gpu_completed_observed", frame, None);
        }
        true
    }

    pub fn release(&mut self, frame: FrameId) -> bool {
        let accepted = self.slots.get(frame.slot as usize) == Some(&Slot::Desk(frame));
        if let Some(flow) = self.flow.as_mut() {
            flow.frame("release_received_observed", frame, Some(accepted));
        }
        if !accepted {
            return false;
        }
        self.slots[frame.slot as usize] = Slot::Free;
        if let Some(flow) = self.flow.as_mut() {
            flow.wait(Instant::now(), false);
        }
        true
    }

    /// Coalesce resize bursts while a producer GPU submission is outstanding. Desk-owned old
    /// surfaces remain alive through its imports and are never written again by this producer.
    pub fn resize(&mut self, now: Instant) -> Result<bool, String> {
        if self
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Rendering(_)))
        {
            return Ok(false);
        }
        if let Some(flow) = self.flow.as_mut() {
            flow.wait(now, false);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Stage image generation exhausted")?;
        self.slots = [Slot::Free; SHARED_FRAME_SLOTS];
        self.next = now;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_late_wakes_preserve_sixty_hertz_phase_without_drift() {
        let origin = Instant::now();
        let mut p = Producer::new(false, origin);
        for index in 0..600_u32 {
            let at = origin + FRAME_INTERVAL * index + Duration::from_micros(500);
            let frame = p.begin(at).expect("one late frame at each scheduled slot");
            p.submitted(frame);
            assert!(p.completed(frame));
            assert!(p.release(frame));
            assert_eq!(p.next, origin + FRAME_INTERVAL * (index + 1));
            assert!(
                p.begin(at).is_none(),
                "no second image at the same late wake"
            );
        }
        assert_eq!(p.sequence, 600);
    }

    #[test]
    fn long_stall_skips_missed_deadlines_and_resize_reveal_reanchor() {
        let origin = Instant::now();
        let mut p = Producer::new(false, origin);
        let at = origin + Duration::from_secs(5) + Duration::from_millis(16);
        let frame = p.begin(at).unwrap();
        assert_eq!(p.next, at + FRAME_INTERVAL);
        assert!(p.begin(at).is_none());
        p.submitted(frame);
        assert!(p.completed(frame));
        assert!(p.release(frame));
        assert!(
            p.begin(at + FRAME_INTERVAL - Duration::from_nanos(1))
                .is_none()
        );
        let resized_at = at + FRAME_INTERVAL;
        let after_stall = p.begin(resized_at).expect("one full period after stall");
        p.cancel(after_stall);
        assert!(p.resize(resized_at).unwrap());
        assert_eq!(p.next, resized_at);
        let resized = p.begin(resized_at).unwrap();
        p.cancel(resized);
        p.set_visible(false, resized_at);
        let revealed = resized_at + Duration::from_secs(2);
        assert!(p.set_visible(true, revealed));
        assert_eq!(p.next, revealed);
        assert!(p.begin(revealed).is_some());
    }
    #[test]
    fn slots_are_never_reused_before_exact_release() {
        let now = Instant::now();
        let mut p = Producer::new(true, now);
        let a = p.begin(now).unwrap();
        p.submitted(a);
        assert!(p.completed(a));
        let b = p.begin(now + Duration::from_millis(17)).unwrap();
        p.submitted(b);
        assert!(p.completed(b));
        let c = p.begin(now + Duration::from_millis(34)).unwrap();
        p.submitted(c);
        assert!(p.completed(c));
        assert_ne!(a.slot, b.slot);
        assert_ne!(a.slot, c.slot);
        assert_ne!(b.slot, c.slot);
        assert_eq!([a.slot, b.slot, c.slot], [0, 1, 2]);
        assert!(!p.available(now + Duration::from_secs(1)));
        assert!(p.begin(now + Duration::from_secs(1)).is_none());
        assert!(!p.release(FrameId {
            sequence: a.sequence + 10,
            ..a
        }));
        assert!(p.release(a));
        assert!(!p.release(a));
        assert_eq!(p.begin(now + Duration::from_secs(1)).unwrap().slot, a.slot);
    }
    #[test]
    fn copy_transport_has_exactly_one_credit_even_with_shared_capacity_three() {
        let now = Instant::now();
        let mut p = Producer::new(false, now);
        assert_eq!(p.count, 1);
        let a = p.begin(now).unwrap();
        assert_eq!(a.slot, 0);
        p.submitted(a);
        assert!(!p.available(now + FRAME_INTERVAL));
        assert!(p.completed(a));
        assert!(!p.available(now + FRAME_INTERVAL));
        assert!(p.begin(now + FRAME_INTERVAL).is_none());
        assert!(!p.release(FrameId { slot: 1, ..a }));
        assert!(!p.release(FrameId { slot: 2, ..a }));
        assert!(p.release(a));
        let b = p.begin(now + FRAME_INTERVAL).unwrap();
        assert_eq!(b.slot, 0);
    }
    #[test]
    fn resize_waits_for_every_rendering_credit_and_old_callbacks_cannot_free_new_images() {
        let now = Instant::now();
        let mut p = Producer::new(true, now);
        let mut old = Vec::new();
        for index in 0..SHARED_FRAME_SLOTS {
            let frame = p.begin(now + FRAME_INTERVAL * index as u32).unwrap();
            p.submitted(frame);
            old.push(frame);
        }
        assert!(!p.resize(now + FRAME_INTERVAL * 3).unwrap());
        for frame in &old[..2] {
            assert!(p.completed(*frame));
            assert!(!p.resize(now + FRAME_INTERVAL * 3).unwrap());
        }
        assert!(p.completed(old[2]));
        let resized_at = now + FRAME_INTERVAL * 3;
        assert!(p.resize(resized_at).unwrap());
        let new = p.begin(resized_at).unwrap();
        p.submitted(new);
        for frame in old {
            assert!(!p.completed(frame));
            assert!(!p.release(frame));
        }
        assert_eq!(p.slots[new.slot as usize], Slot::Rendering(new));
        assert!(p.completed(new));
        assert!(p.release(new));
    }
    #[test]
    fn hidden_and_missed_deadlines_never_burst() {
        let now = Instant::now();
        let mut p = Producer::new(false, now);
        let a = p.begin(now).unwrap();
        p.cancel(a);
        assert!(p.begin(now).is_none());
        p.set_visible(false, now);
        assert!(p.begin(now + Duration::from_secs(1)).is_none());
        assert!(p.set_visible(true, now + Duration::from_secs(1)));
        let a = p.begin(now + Duration::from_secs(1)).unwrap();
        p.cancel(a);
        assert!(p.begin(now + Duration::from_secs(1)).is_none());
    }
    #[test]
    fn resize_waits_for_gpu_and_stale_releases_cannot_free_new_slot() {
        let now = Instant::now();
        let mut p = Producer::new(true, now);
        let old = p.begin(now).unwrap();
        p.submitted(old);
        assert!(!p.resize(now).unwrap());
        assert!(p.completed(old));
        assert!(p.resize(now).unwrap());
        let new = p.begin(now).unwrap();
        assert_ne!(old.generation, new.generation);
        assert!(!p.release(old));
        assert!(!p.completed(old));
        assert!(p.completed(new));
    }
}
