//! Containers a frame borrows and hands back when the last reader lets go.
//!
//! A render's output is published rather than consumed on the spot: universe frames go to the
//! network, patched extents go with them, and visualization values may be read by a client that
//! is not connected yet. None of that can be written into storage the next frame will overwrite,
//! which is why they were allocated fresh every tick.
//!
//! They are borrowed from a pool instead. A reader holds one for as long as it likes and it
//! returns itself when dropped, so a desk holding its rate grows them once. The pool is bounded:
//! a reader that never lets go costs the desk a fresh allocation, never a stall.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::ops::Deref;
use std::sync::Arc;

use parking_lot::Mutex;

/// How many of each kind may be in flight before a frame allocates rather than waits.
const MAX_IN_FLIGHT: usize = 8;

/// Something a pool can hand out again once its reader has finished.
pub trait Reusable: Default + Send + Sync + 'static {
    /// Empty it, keeping whatever room it has already grown.
    fn reset(&mut self);
}

impl<K: Eq + Hash + Send + Sync + 'static, V: Send + Sync + 'static, S> Reusable
    for HashMap<K, V, S>
where
    S: BuildHasher + Default + Send + Sync + 'static,
{
    fn reset(&mut self) {
        self.clear();
    }
}

/// Where values of one kind wait between frames.
pub struct ValuePool<T: Reusable> {
    idle: Mutex<Vec<T>>,
    in_flight: Mutex<usize>,
}

impl<T: Reusable> Default for ValuePool<T> {
    fn default() -> Self {
        Self {
            idle: Mutex::new(Vec::new()),
            in_flight: Mutex::new(0),
        }
    }
}

impl<T: Reusable> ValuePool<T> {
    /// Something to fill, reused when the pool has one and freshly made when it does not.
    pub fn take(self: &Arc<Self>) -> Pooled<T> {
        let reusable = {
            let mut in_flight = self.in_flight.lock();
            if *in_flight >= MAX_IN_FLIGHT {
                None
            } else {
                *in_flight += 1;
                Some(self.idle.lock().pop())
            }
        };
        match reusable {
            Some(Some(value)) => Pooled {
                value: Some(value),
                pool: Some(Arc::clone(self)),
            },
            Some(None) => Pooled {
                value: Some(T::default()),
                pool: Some(Arc::clone(self)),
            },
            // Everything is still held. This one is on its own rather than making the desk wait.
            None => Pooled {
                value: Some(T::default()),
                pool: None,
            },
        }
    }

    fn give_back(&self, mut value: T) {
        {
            let mut in_flight = self.in_flight.lock();
            *in_flight = in_flight.saturating_sub(1);
        }
        value.reset();
        let mut idle = self.idle.lock();
        if idle.len() < MAX_IN_FLIGHT {
            idle.push(value);
        }
    }

    /// How many of these the pool has had to build rather than reuse.
    #[cfg(test)]
    pub(crate) fn idle_len(&self) -> usize {
        self.idle.lock().len()
    }
}

/// A value on loan from a pool, returned when it is dropped.
pub struct Pooled<T: Reusable> {
    value: Option<T>,
    pool: Option<Arc<ValuePool<T>>>,
}

impl<T: Reusable> Pooled<T> {
    /// A value with no pool behind it, for a caller that names one before a render has happened.
    pub fn unpooled(value: T) -> Self {
        Self {
            value: Some(value),
            pool: None,
        }
    }
}

impl<T: Reusable> Default for Pooled<T> {
    fn default() -> Self {
        Self::unpooled(T::default())
    }
}

impl<T: Reusable> Deref for Pooled<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_ref().expect("a loan holds its value")
    }
}

/// Filling a loan before it is published. A reader only ever sees it through `Deref`.
impl<T: Reusable> std::ops::DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("a loan holds its value")
    }
}

impl<T: Reusable> Drop for Pooled<T> {
    fn drop(&mut self) {
        if let (Some(pool), Some(value)) = (self.pool.take(), self.value.take()) {
            pool.give_back(value);
        }
    }
}

/// Two loans are equal when what they hold is, which is what a test comparing two frames means.
impl<T: Reusable + PartialEq> PartialEq for Pooled<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Reusable + Eq> Eq for Pooled<T> {}

impl<T: Reusable + std::fmt::Debug> std::fmt::Debug for Pooled<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Frames = HashMap<u16, u8>;

    #[test]
    fn a_returned_value_is_filled_again_rather_than_replaced() {
        let pool = Arc::new(ValuePool::<Frames>::default());
        let mut first = pool.take();
        first.insert(1, 7);
        drop(first);
        assert_eq!(pool.idle_len(), 1);
        let second = pool.take();
        assert!(second.is_empty(), "a loan starts empty however it was left");
        assert_eq!(pool.idle_len(), 0);
    }

    #[test]
    fn a_reader_that_never_lets_go_costs_a_frame_its_own_value_not_the_desk() {
        let pool = Arc::new(ValuePool::<Frames>::default());
        let held = (0..MAX_IN_FLIGHT).map(|_| pool.take()).collect::<Vec<_>>();
        // Past the bound the desk keeps rendering; this one simply has no pool behind it.
        let extra = pool.take();
        assert!(extra.is_empty());
        drop(extra);
        assert_eq!(pool.idle_len(), 0, "an unpooled value is not adopted");
        drop(held);
        assert_eq!(pool.idle_len(), MAX_IN_FLIGHT);
    }

    #[test]
    fn a_steady_render_stops_building_values() {
        let pool = Arc::new(ValuePool::<Frames>::default());
        for _ in 0..32 {
            let mut value = pool.take();
            value.insert(1, 1);
        }
        assert_eq!(pool.idle_len(), 1, "one value, refilled");
    }
}
