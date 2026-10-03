//! Adapter work counters a frame's parallel workers update together (TL-639 round 5).
//!
//! Counts are commutative, so each thread adds into its own shard and a read sums the shards:
//! counting never waits on another thread, and the total is the single-threaded total.
use std::sync::atomic::{AtomicUsize, Ordering};

/// A counter set that adds field by field.
pub(in crate::runtime) trait CounterSum: Copy + Default {
    fn add(&mut self, other: &Self);
}

/// Implement [`CounterSum`] for a counter struct; every field must be listed, or the
/// destructuring fails to compile.
macro_rules! counter_sum {
    ($ty:ident { $($field:ident),* $(,)? }) => {
        impl super::counters::CounterSum for $ty {
            fn add(&mut self, other: &Self) {
                let $ty { $($field),* } = *other;
                $(self.$field += $field;)*
            }
        }
    };
}
pub(super) use counter_sum;

const SHARDS: usize = 16;

/// One shard per cache line, so two threads never share one.
#[repr(align(128))]
#[derive(Default)]
struct Shard<C>(parking_lot::Mutex<C>);

pub(in crate::runtime) struct ShardedCounters<C> {
    shards: Box<[Shard<C>]>,
}

impl<C: Default> Default for ShardedCounters<C> {
    fn default() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Shard::default()).collect(),
        }
    }
}

/// This thread's shard: threads take shards in turn.
fn shard() -> usize {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    thread_local! {
        static SHARD: usize = NEXT.fetch_add(1, Ordering::Relaxed) % SHARDS;
    }
    SHARD.with(|shard| *shard)
}

impl<C: CounterSum> ShardedCounters<C> {
    pub fn update(&self, update: impl FnOnce(&mut C)) {
        update(&mut self.shards[shard()].0.lock());
    }

    pub fn total(&self) -> C {
        let mut total = C::default();
        for shard in self.shards.iter() {
            total.add(&shard.0.lock());
        }
        total
    }
}
