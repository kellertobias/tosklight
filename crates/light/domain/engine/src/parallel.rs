//! Ordered fork/join over a frame's independent per-target work (TL-639 round 5).
//!
//! A frame splits work whose items read only shared, immutable inputs and write only their own
//! result into contiguous chunks. Threads take chunks in any order; the results come back in
//! chunk order, so whatever merges them sees exactly the sequence the single-threaded loop
//! produced. The worker count is a capability of the desk, not of the show: it never reaches an
//! output value, only how fast one is computed. One worker runs everything on the caller.
//!
//! The threads persist in an [`OutputPool`] the engine owns; a frame's chunks borrow its inputs
//! only for the call (`rayon`'s scoped install), and a panic in any chunk resumes on the caller.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The default never exceeds this many threads: beyond it the frame's serial share dominates
/// and extra efficiency cores only add spawn cost.
pub const MAX_DEFAULT_OUTPUT_WORKERS: usize = 8;

/// Process-wide default for engines created after it is set; 0 means "not set".
static DEFAULT_OUTPUT_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// The output worker count a new engine starts with: the configured default, or the available
/// parallelism capped at [`MAX_DEFAULT_OUTPUT_WORKERS`].
pub fn default_output_workers() -> usize {
    match DEFAULT_OUTPUT_WORKERS.load(Ordering::Relaxed) {
        0 => std::thread::available_parallelism()
            .map_or(1, |count| count.get())
            .clamp(1, MAX_DEFAULT_OUTPUT_WORKERS),
        configured => configured,
    }
}

/// Set the worker count engines created from now on start with (at least one). The desk and the
/// benchmark set it once at startup from their configuration.
pub fn set_default_output_workers(workers: usize) {
    DEFAULT_OUTPUT_WORKERS.store(workers.max(1), Ordering::Relaxed);
}

/// Parse an operator/benchmark worker setting: a positive count, or `max` for the available
/// parallelism without the default cap.
pub fn parse_output_workers(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("max") {
        return Some(
            std::thread::available_parallelism()
                .map_or(1, |count| count.get())
                .max(1),
        );
    }
    value.parse::<usize>().ok().filter(|count| *count > 0)
}

/// How many chunks to cut `items` into for `workers` threads: several per thread so that a slow
/// (efficiency) core does not hold the frame, but never chunks smaller than `min_chunk` items.
pub fn chunk_count(items: usize, workers: usize, min_chunk: usize) -> usize {
    if workers <= 1 || items == 0 {
        return 1;
    }
    (workers * 4).min(items.div_ceil(min_chunk.max(1))).max(1)
}

/// The half-open item range of chunk `chunk` of `chunks` over `items` items.
pub fn chunk_range(items: usize, chunks: usize, chunk: usize) -> std::ops::Range<usize> {
    let start = items * chunk / chunks;
    let end = items * (chunk + 1) / chunks;
    start..end
}

/// A desk's output worker pool: persistent threads a frame's chunks run on, so a frame neither
/// spawns threads nor warms new ones. Owned by the engine; its threads end with it.
pub struct OutputPool {
    pool: rayon::ThreadPool,
}

impl OutputPool {
    /// A pool of `workers` threads, or `None` for one worker (everything stays on the caller).
    pub fn new(workers: usize) -> Option<Self> {
        if workers <= 1 {
            return None;
        }
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .thread_name(|index| format!("light-output-{index}"))
            .build()
            .ok()
            .map(|pool| Self { pool })
    }

    pub fn workers(&self) -> usize {
        self.pool.current_num_threads()
    }

    /// Free `garbage` on a pool thread while the frame goes on: what a frame drops (last frame's
    /// compiled samples) is a long tail of frees nothing waits for.
    pub fn drop_later<T: Send + 'static>(&self, garbage: T) {
        self.pool.spawn(move || drop(garbage));
    }
}

/// A frame's per-instance Dynamic work (TL-639 round 7): every index once, on the pool.
impl light_dynamics::InstanceWorkers for OutputPool {
    fn run_indexed(&self, count: usize, run: &(dyn Fn(usize) + Sync)) {
        use rayon::prelude::*;
        if self.workers() <= 1 || count <= 1 {
            (0..count).for_each(run);
            return;
        }
        self.pool.install(|| {
            (0..count)
                .into_par_iter()
                .with_max_len(1)
                .for_each(run)
        });
    }
}

/// Below this many items a sort stays on the caller.
const MIN_PARALLEL_SORT: usize = if cfg!(test) { 2 } else { 4096 };

/// `items.sort_unstable_by_key(key)` on `pool`'s threads (TL-639 round 6). The keys must be
/// distinct, so the result is the single-threaded sort's for any worker count.
pub fn sort_unstable_by_key<T: Send, K: Ord>(
    pool: Option<&OutputPool>,
    items: &mut [T],
    key: impl Fn(&T) -> K + Sync,
) {
    use rayon::prelude::*;
    match pool.filter(|pool| pool.workers() > 1 && items.len() >= MIN_PARALLEL_SORT) {
        Some(pool) => pool
            .pool
            .install(|| items.par_sort_unstable_by_key(|item| key(item))),
        None => items.sort_unstable_by_key(key),
    }
}

/// Run `chunks` chunks on `pool` (on the caller without one) and return the results in chunk
/// order. Each pool thread uses one `workers` entry (its scratch), which must have at least
/// [`OutputPool::workers`] entries; which thread ran which chunk is never observable in the
/// result.
pub fn run_ordered<W: Send, R: Send>(
    pool: Option<&OutputPool>,
    workers: &mut [W],
    chunks: usize,
    run: impl Fn(&mut W, usize) -> R + Sync,
) -> Vec<R> {
    use rayon::prelude::*;
    let Some(pool) = pool.filter(|pool| pool.workers() > 1 && chunks > 1) else {
        let Some(worker) = workers.first_mut() else {
            return Vec::new();
        };
        return (0..chunks).map(|chunk| run(worker, chunk)).collect();
    };
    assert!(
        workers.len() >= pool.workers(),
        "one scratch entry per output worker"
    );
    let slots = workers
        .iter_mut()
        .map(parking_lot::Mutex::new)
        .collect::<Vec<_>>();
    pool.pool.install(|| {
        (0..chunks)
            .into_par_iter()
            .with_max_len(1)
            .map(|chunk| {
                let index = rayon::current_thread_index().expect("a pool thread");
                let mut worker = slots[index].lock();
                run(&mut worker, chunk)
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_chunk_order_for_any_worker_count() {
        for workers in [1, 2, 3, 8] {
            let pool = OutputPool::new(workers);
            let mut scratch = vec![0usize; workers];
            let results = run_ordered(pool.as_ref(), &mut scratch, 37, |seen, chunk| {
                *seen += 1;
                chunk * 10
            });
            assert_eq!(results, (0..37).map(|chunk| chunk * 10).collect::<Vec<_>>());
            assert_eq!(scratch.iter().sum::<usize>(), 37);
        }
    }

    #[test]
    fn chunk_ranges_cover_every_item_once_in_order() {
        for (items, chunks) in [(0, 1), (1, 1), (10, 3), (4148, 32), (7, 7)] {
            let covered = (0..chunks)
                .flat_map(|chunk| chunk_range(items, chunks, chunk))
                .collect::<Vec<_>>();
            assert_eq!(covered, (0..items).collect::<Vec<_>>());
        }
        assert_eq!(chunk_count(100, 1, 8), 1);
        assert_eq!(chunk_count(100, 4, 8), 13);
        assert_eq!(chunk_count(4000, 8, 64), 32);
    }

    #[test]
    fn a_parallel_sort_of_distinct_keys_is_the_single_threaded_sort() {
        let items = (0..10_000u64)
            .map(|item| item.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .collect::<Vec<_>>();
        let mut expected = items.clone();
        expected.sort_unstable();
        for workers in [1, 2, 8] {
            let pool = OutputPool::new(workers);
            let mut sorted = items.clone();
            sort_unstable_by_key(pool.as_ref(), &mut sorted, |item| *item);
            assert_eq!(sorted, expected);
        }
    }

    #[test]
    fn worker_settings_parse() {
        assert_eq!(parse_output_workers("3"), Some(3));
        assert_eq!(parse_output_workers("0"), None);
        assert!(parse_output_workers("max").is_some_and(|count| count >= 1));
        assert_eq!(parse_output_workers("x"), None);
    }
}
