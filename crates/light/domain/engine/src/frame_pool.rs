//! Where a generation's frame buffers wait between frames.
//!
//! A buffer is shaped for one generation, so the pool is too: when the patch changes the old
//! buffers are worth nothing and a new pool is compiled with the new shape. The pool is bounded on
//! purpose. A consumer that holds a frame and never releases it must cost the desk a dropped frame
//! rather than a stalled one, and must never be able to make the desk allocate without limit.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::FrameState;

/// How many buffers one generation may have in flight at once.
///
/// Two would be enough for a render that publishes and immediately reclaims. Four leaves room for
/// a consumer to be one or two frames behind without costing anyone a frame.
const MAX_BUFFERS: usize = 4;

pub(crate) struct FramePool {
    generation: u64,
    slots: usize,
    idle: Mutex<Vec<FrameState>>,
    /// Buffers this pool has handed out and not yet had back, capped at [`MAX_BUFFERS`].
    outstanding: Mutex<usize>,
    /// How many buffers this generation has had to build. A desk holding its rate builds a few
    /// and then reuses them; a rising count means frames are being allocated rather than reused.
    built: AtomicUsize,
}

impl FramePool {
    /// A pool for a generation's shape. Compiled with the patch, never in the frame loop.
    pub(crate) fn for_generation(generation: u64, slots: usize) -> Self {
        Self {
            generation,
            slots,
            idle: Mutex::new(Vec::with_capacity(MAX_BUFFERS)),
            outstanding: Mutex::new(0),
            built: AtomicUsize::new(0),
        }
    }

    /// How many buffers this generation has built rather than reused.
    #[cfg(test)]
    pub(crate) fn built(&self) -> usize {
        self.built.load(Ordering::Relaxed)
    }

    /// A buffer to fill, or nothing when every one of this generation's buffers is still held.
    ///
    /// Returning nothing is how a stalled consumer costs a frame instead of the desk.
    pub(crate) fn take(&self) -> Option<FrameState> {
        {
            let mut outstanding = self.outstanding.lock();
            if *outstanding >= MAX_BUFFERS {
                return None;
            }
            *outstanding += 1;
        }
        let reused = self.idle.lock().pop();
        Some(match reused {
            Some(mut state) => {
                state.begin();
                state
            }
            None => {
                self.built.fetch_add(1, Ordering::Relaxed);
                let mut state = FrameState::for_generation(self.generation, self.slots);
                state.begin();
                state
            }
        })
    }

    /// Hand a buffer back for the next frame to fill. A buffer shaped for another generation is
    /// dropped rather than kept, since its numbering addresses a patch that no longer stands.
    pub(crate) fn give_back(&self, state: FrameState) {
        {
            let mut outstanding = self.outstanding.lock();
            *outstanding = outstanding.saturating_sub(1);
        }
        if state.generation() != self.generation || state.capacity() != self.slots {
            return;
        }
        let mut idle = self.idle.lock();
        if idle.len() < MAX_BUFFERS {
            idle.push(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_returned_buffer_waits_for_the_next_frame_rather_than_being_dropped() {
        let pool = FramePool::for_generation(1, 8);
        let first = pool.take().unwrap();
        assert_eq!(pool.idle.lock().len(), 0, "a buffer in use is not idle");
        pool.give_back(first);
        assert_eq!(pool.idle.lock().len(), 1, "a returned buffer is kept");
        let second = pool.take().unwrap();
        assert_eq!(second.capacity(), 8);
        assert_eq!(pool.idle.lock().len(), 0, "the next frame takes it back");
    }

    #[test]
    fn a_pool_never_hands_out_more_than_its_bound() {
        let pool = FramePool::for_generation(1, 8);
        let held = (0..MAX_BUFFERS)
            .map(|_| pool.take().expect("within the bound"))
            .collect::<Vec<_>>();
        assert!(pool.take().is_none(), "a stalled consumer drops a frame");
        for state in held {
            pool.give_back(state);
        }
        assert!(pool.take().is_some());
    }

    #[test]
    fn a_pool_stops_building_buffers_once_it_has_one_to_reuse() {
        let pool = FramePool::for_generation(1, 8);
        for _ in 0..20 {
            let state = pool
                .take()
                .expect("a returned buffer is always available again");
            pool.give_back(state);
        }
        assert_eq!(
            pool.built(),
            1,
            "a desk holding its rate builds one buffer and refills it"
        );
    }

    #[test]
    fn a_buffer_from_another_generation_is_not_kept() {
        let pool = FramePool::for_generation(2, 8);
        let stale = FrameState::for_generation(1, 8);
        pool.give_back(stale);
        assert_eq!(pool.idle.lock().len(), 0);
    }

    #[test]
    fn each_fill_starts_empty() {
        let pool = FramePool::for_generation(1, 8);
        let mut state = pool.take().unwrap();
        state.force(
            crate::Slot::from_index(3),
            light_core::AttributeValue::Normalized(1.0),
        );
        assert_eq!(state.occupied_len(), 1);
        pool.give_back(state);
        let state = pool.take().unwrap();
        assert_eq!(state.occupied_len(), 0);
    }
}
