//! Opt-in synchronous CPU observations. Covered jobs are not total pool CPU: Rayon sorts and
//! unrelated/deferred jobs can run concurrently. Section-to-job lag includes caller setup, earlier jobs and scratch locking; it is wall time, never CPU wait.
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS")
        .ok()
        .as_deref()
        == Some("all")
});
const CLEANUP_LIMIT: u64 = 65_536;
static CLEANUPS: AtomicU64 = AtomicU64::new(0);
static CLEANUP_CAP: AtomicBool = AtomicBool::new(false);
fn ns(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CpuObservation {
    pub wall_nanos: u64,
    pub thread_cpu_nanos: u64,
    pub cpu_status: u8,
    pub counter_read_nanos: u64,
}
struct Counter {
    value: Option<u64>,
    supported: bool,
    read_nanos: u64,
}
impl Counter {
    fn read() -> Self {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let started = Instant::now();
            // A failed diagnostic must not panic or alter output. This clock belongs to the
            // calling OS thread on both platforms; no Mach port or unsafe code is required.
            let value = rustix::time::clock_gettime_dynamic(rustix::time::DynamicClockId::Known(
                rustix::time::ClockId::ThreadCPUTime,
            ))
            .ok()
            .and_then(|value| checked_cpu_nanos(value.tv_sec, value.tv_nsec));
            Self {
                value,
                supported: true,
                read_nanos: ns(started.elapsed()),
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self {
                value: None,
                supported: false,
                read_nanos: 0,
            }
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn checked_cpu_nanos(seconds: i64, nanos: i64) -> Option<u64> {
    u64::try_from(seconds)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(nanos).ok().filter(|n| *n < 1_000_000_000)?)
}
/// !Send: both CPU observations must stay on one OS thread; never hold this across an await.
pub struct CpuTimer {
    started: Instant,
    before: Counter,
    _same_thread: PhantomData<Rc<()>>,
}
impl CpuTimer {
    pub fn start_if(enabled: bool) -> Option<Self> {
        enabled.then(|| Self {
            started: Instant::now(),
            before: Counter::read(),
            _same_thread: PhantomData,
        })
    }
    pub fn finish(self) -> CpuObservation {
        let after = Counter::read();
        observation(self.before, after, ns(self.started.elapsed()))
    }
}
fn observation(before: Counter, after: Counter, wall_nanos: u64) -> CpuObservation {
    let cpu = before
        .value
        .zip(after.value)
        .and_then(|(a, b)| b.checked_sub(a));
    CpuObservation {
        wall_nanos,
        thread_cpu_nanos: cpu.unwrap_or(0),
        cpu_status: if !before.supported || !after.supported {
            0
        } else if cpu.is_some() {
            1
        } else {
            2
        },
        counter_read_nanos: before.read_nanos.saturating_add(after.read_nanos),
    }
}

/// API identity only, not a semantic operation name. Jobs belong to exactly one kind.
#[derive(Clone, Copy)]
pub(super) enum JobKind {
    Ordered,
    Indexed,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct JobKindSummary {
    pub jobs: u64,
    pub cpu_valid_jobs: u64,
    pub cpu_unavailable_jobs: u64,
    pub cpu_invalid_jobs: u64,
    pub cpu_nanos: u64,
    pub counter_read_nanos: u64,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct WorkerSummary {
    pub calls: u64,
    pub jobs: u64,
    pub cpu_valid_jobs: u64,
    pub cpu_unavailable_jobs: u64,
    pub cpu_invalid_jobs: u64,
    pub cpu_nanos: u64,
    pub counter_read_nanos: u64,
    pub job_wall_nanos: u64,
    pub job_wall_max_nanos: u64,
    pub section_to_job_max_nanos: u64,
    pub uncovered_sort_calls: u64,
    pub pool_timing: PoolTimingSummary,
    /// Ordered, Indexed: sums of the same observations used by the aggregate.
    pub job_kinds: [JobKindSummary; 2],
}
/// All-only wall frontiers of actual synchronous pool installs. Sums overlap across jobs;
/// scheduling, preemption and result collection are included, never inferred CPU wait.
#[derive(Clone, Copy, Debug, Default)]
pub struct PoolTimingSummary {
    pub install_calls: u64,
    pub install_jobs: u64,
    pub install_empty_calls: u64,
    pub install_invalid_calls: u64,
    pub install_wall_nanos: u64,
    pub submit_to_first_entry_nanos: u64,
    pub submit_to_entry_max_nanos: u64,
    pub last_exit_to_return_nanos: u64,
    pub last_exit_to_return_max_nanos: u64,
    pub scratch_lock_calls: u64,
    pub scratch_lock_wall_nanos: u64,
    pub scratch_lock_wall_max_nanos: u64,
}
impl PoolTimingSummary {
    pub fn merge(&mut self, other: Self) {
        macro_rules! sums { ($($field:ident),*) => { $(self.$field = self.$field.saturating_add(other.$field);)* }; }
        sums!(
            install_calls,
            install_jobs,
            install_empty_calls,
            install_invalid_calls,
            install_wall_nanos,
            submit_to_first_entry_nanos,
            last_exit_to_return_nanos,
            scratch_lock_calls,
            scratch_lock_wall_nanos
        );
        self.submit_to_entry_max_nanos = self
            .submit_to_entry_max_nanos
            .max(other.submit_to_entry_max_nanos);
        self.last_exit_to_return_max_nanos = self
            .last_exit_to_return_max_nanos
            .max(other.last_exit_to_return_max_nanos);
        self.scratch_lock_wall_max_nanos = self
            .scratch_lock_wall_max_nanos
            .max(other.scratch_lock_wall_max_nanos);
    }
}
/// One install's common clock origin. The borrow stays inside synchronous pool.install.
/// Job entry precedes scratch locking; exit follows the callback and its local guard drops.
pub(super) struct InstallTiming {
    submitted: Instant,
    entries: AtomicU64,
    exits: AtomicU64,
    first_entry: AtomicU64,
    last_entry: AtomicU64,
    last_exit: AtomicU64,
    lock_calls: AtomicU64,
    lock_wall: AtomicU64,
    lock_max: AtomicU64,
}
impl InstallTiming {
    pub(super) fn begin() -> Self {
        Self {
            submitted: Instant::now(),
            entries: AtomicU64::new(0),
            exits: AtomicU64::new(0),
            first_entry: AtomicU64::new(u64::MAX),
            last_entry: AtomicU64::new(0),
            last_exit: AtomicU64::new(0),
            lock_calls: AtomicU64::new(0),
            lock_wall: AtomicU64::new(0),
            lock_max: AtomicU64::new(0),
        }
    }
    pub(super) fn job<T>(&self, work: impl FnOnce(Instant) -> T) -> T {
        let entered = Instant::now();
        let offset = ns(entered.duration_since(self.submitted));
        self.entries.fetch_add(1, Ordering::Relaxed);
        self.first_entry.fetch_min(offset, Ordering::Relaxed);
        self.last_entry.fetch_max(offset, Ordering::Relaxed);
        let result = work(entered);
        let offset = ns(self.submitted.elapsed());
        self.last_exit.fetch_max(offset, Ordering::Relaxed);
        self.exits.fetch_add(1, Ordering::Relaxed);
        result
    }
    pub(super) fn lock_acquired(&self, started: Instant) {
        let wall = ns(started.elapsed());
        self.lock_calls.fetch_add(1, Ordering::Relaxed);
        self.lock_wall.fetch_add(wall, Ordering::Relaxed);
        self.lock_max.fetch_max(wall, Ordering::Relaxed);
    }
    /// Read the return frontier before locks, aggregation, or destruction on the caller.
    pub(super) fn finish(self, coverage: &Coverage) {
        let returned = ns(self.submitted.elapsed());
        let get = |v: &AtomicU64| v.load(Ordering::Relaxed);
        let jobs = get(&self.entries);
        let first = get(&self.first_entry);
        let last = get(&self.last_entry);
        let exit = get(&self.last_exit);
        let invalid = jobs != get(&self.exits)
            || get(&self.lock_calls) > jobs
            || (jobs > 0 && !(first <= last && last <= exit && exit <= returned));
        let gap = if jobs > 0 {
            returned.saturating_sub(exit)
        } else {
            0
        };
        let row = PoolTimingSummary {
            install_calls: 1,
            install_jobs: jobs,
            install_empty_calls: u64::from(jobs == 0),
            install_invalid_calls: u64::from(invalid),
            install_wall_nanos: returned,
            submit_to_first_entry_nanos: if jobs > 0 { first } else { 0 },
            submit_to_entry_max_nanos: last,
            last_exit_to_return_nanos: gap,
            last_exit_to_return_max_nanos: gap,
            scratch_lock_calls: get(&self.lock_calls),
            scratch_lock_wall_nanos: get(&self.lock_wall),
            scratch_lock_wall_max_nanos: get(&self.lock_max),
        };
        coverage
            .totals
            .pool
            .lock()
            .expect("pool timing mutex poisoned")
            .merge(row);
    }
}
#[derive(Default)]
struct KindTotals {
    jobs: AtomicU64,
    valid: AtomicU64,
    unavailable: AtomicU64,
    invalid: AtomicU64,
    cpu: AtomicU64,
    read: AtomicU64,
}
impl KindTotals {
    fn summary(&self) -> JobKindSummary {
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        JobKindSummary {
            jobs: get(&self.jobs),
            cpu_valid_jobs: get(&self.valid),
            cpu_unavailable_jobs: get(&self.unavailable),
            cpu_invalid_jobs: get(&self.invalid),
            cpu_nanos: get(&self.cpu),
            counter_read_nanos: get(&self.read),
        }
    }
}
#[derive(Default)]
struct Totals {
    pool: Mutex<PoolTimingSummary>,
    jobs: AtomicU64,
    valid: AtomicU64,
    unavailable: AtomicU64,
    invalid: AtomicU64,
    cpu: AtomicU64,
    read: AtomicU64,
    wall: AtomicU64,
    wall_max: AtomicU64,
    lag_max: AtomicU64,
    sorts: AtomicU64,
    kinds: [KindTotals; 2],
}
pub(super) struct Coverage {
    submitted: Instant,
    totals: Totals,
}
thread_local! { static CURRENT: RefCell<Option<Arc<Coverage>>> = const { RefCell::new(None) }; }
struct Restore {
    previous: Option<Arc<Coverage>>,
    _same_thread: PhantomData<Rc<()>>,
}
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|current| *current.borrow_mut() = self.previous.take());
    }
}
/// Diagnostic-only synchronous coverage: nested scope restoration is lexical, including unwind.
pub fn covered<T>(enabled: bool, work: impl FnOnce() -> T) -> (T, Option<WorkerSummary>) {
    if !enabled {
        return (work(), None);
    }
    let shared = Arc::new(Coverage {
        submitted: Instant::now(),
        totals: Totals::default(),
    });
    let previous = CURRENT.with(|current| current.replace(Some(Arc::clone(&shared))));
    let _restore = Restore {
        previous,
        _same_thread: PhantomData,
    };
    let result = work();
    let t = &shared.totals;
    let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
    (
        result,
        Some(WorkerSummary {
            calls: 1,
            job_kinds: t.kinds.each_ref().map(KindTotals::summary),
            jobs: get(&t.jobs),
            cpu_valid_jobs: get(&t.valid),
            cpu_unavailable_jobs: get(&t.unavailable),
            cpu_invalid_jobs: get(&t.invalid),
            cpu_nanos: get(&t.cpu),
            counter_read_nanos: get(&t.read),
            job_wall_nanos: get(&t.wall),
            job_wall_max_nanos: get(&t.wall_max),
            section_to_job_max_nanos: get(&t.lag_max),
            uncovered_sort_calls: get(&t.sorts),
            pool_timing: *t.pool.lock().expect("pool timing mutex poisoned"),
        }),
    )
}
pub(super) fn current() -> Option<Arc<Coverage>> {
    if !*ENABLED && !cfg!(test) {
        return None;
    }
    CURRENT.with(|current| current.borrow().clone())
}
impl Coverage {
    pub(super) fn job<T>(&self, kind: JobKind, work: impl FnOnce() -> T) -> T {
        let lag = ns(self.submitted.elapsed());
        let timer = CpuTimer::start_if(true).expect("enabled coverage");
        let result = work();
        let observed = timer.finish();
        self.record(kind, observed, lag);
        result
    }
    fn record(&self, kind: JobKind, observed: CpuObservation, lag: u64) {
        let t = &self.totals;
        t.jobs.fetch_add(1, Ordering::Relaxed);
        match observed.cpu_status {
            1 => &t.valid,
            0 => &t.unavailable,
            _ => &t.invalid,
        }
        .fetch_add(1, Ordering::Relaxed);
        t.cpu
            .fetch_add(observed.thread_cpu_nanos, Ordering::Relaxed);
        t.read
            .fetch_add(observed.counter_read_nanos, Ordering::Relaxed);
        t.wall.fetch_add(observed.wall_nanos, Ordering::Relaxed);
        t.wall_max.fetch_max(observed.wall_nanos, Ordering::Relaxed);
        t.lag_max.fetch_max(lag, Ordering::Relaxed);
        let k = &t.kinds[kind as usize];
        k.jobs.fetch_add(1, Ordering::Relaxed);
        match observed.cpu_status {
            1 => &k.valid,
            0 => &k.unavailable,
            _ => &k.invalid,
        }
        .fetch_add(1, Ordering::Relaxed);
        k.cpu
            .fetch_add(observed.thread_cpu_nanos, Ordering::Relaxed);
        k.read
            .fetch_add(observed.counter_read_nanos, Ordering::Relaxed);
    }
}
pub(super) fn uncovered_sort() {
    if let Some(current) = current() {
        current.totals.sorts.fetch_add(1, Ordering::Relaxed);
    }
}

pub(super) struct Cleanup {
    queued: Instant,
    parent: tracing::Span,
}
pub(super) fn cleanup() -> Option<Cleanup> {
    if !*ENABLED {
        return None;
    }
    if CLEANUPS
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            if n < CLEANUP_LIMIT { Some(n + 1) } else { None }
        })
        .is_err()
    {
        if !CLEANUP_CAP.swap(true, Ordering::Relaxed) {
            tracing::info!(
                schema_version = 1u8,
                admitted_cleanups = CLEANUP_LIMIT,
                "output pool deferred cleanup cap reached"
            );
        }
        return None;
    }
    Some(Cleanup {
        queued: Instant::now(),
        parent: tracing::Span::current(),
    })
}
impl Cleanup {
    pub(super) fn run(self, work: impl FnOnce()) {
        let lag = ns(self.queued.elapsed());
        let timer = CpuTimer::start_if(true).expect("enabled cleanup");
        work();
        let sample = timer.finish();
        self.parent.in_scope(|| {
            tracing::info!(
                schema_version = 1u8,
                queue_lag_nanos = lag,
                cleanup_wall_nanos = sample.wall_nanos,
                thread_cpu_nanos = sample.thread_cpu_nanos,
                cpu_status = sample.cpu_status,
                counter_read_nanos = sample.counter_read_nanos,
                "output pool deferred cleanup"
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_kind_counters_reuse_exact_observations_and_conserve_aggregate() {
        let (_, summary) = covered(true, || {
            let c = current().unwrap();
            for (kind, status, cpu, read) in [
                (JobKind::Ordered, 1, 7, 2),
                (JobKind::Ordered, 2, 0, 3),
                (JobKind::Indexed, 1, 11, 5),
                (JobKind::Indexed, 0, 0, 0),
            ] {
                c.record(
                    kind,
                    CpuObservation {
                        wall_nanos: 30,
                        thread_cpu_nanos: cpu,
                        cpu_status: status,
                        counter_read_nanos: read,
                    },
                    9,
                );
            }
        });
        let s = summary.unwrap();
        assert_eq!(
            (
                s.jobs,
                s.cpu_valid_jobs,
                s.cpu_unavailable_jobs,
                s.cpu_invalid_jobs,
                s.cpu_nanos,
                s.counter_read_nanos
            ),
            (4, 2, 1, 1, 18, 10)
        );
        let [ordered, indexed] = s.job_kinds;
        assert_eq!(
            (
                ordered.jobs,
                ordered.cpu_valid_jobs,
                ordered.cpu_invalid_jobs,
                ordered.cpu_nanos,
                ordered.counter_read_nanos
            ),
            (2, 1, 1, 7, 5)
        );
        assert_eq!(
            (
                indexed.jobs,
                indexed.cpu_valid_jobs,
                indexed.cpu_unavailable_jobs,
                indexed.cpu_nanos,
                indexed.counter_read_nanos
            ),
            (2, 1, 1, 11, 5)
        );
        assert_eq!(s.job_wall_nanos, 120);
    }
    #[test]
    fn nested_panicking_callback_restores_outer_without_counting_unreturned_job() {
        let (_, summary) = covered(true, || {
            let outer = current().unwrap();
            assert!(
                std::panic::catch_unwind(|| covered(true, || {
                    current()
                        .unwrap()
                        .job(JobKind::Ordered, || panic!("unreturned"))
                }))
                .is_err()
            );
            assert!(Arc::ptr_eq(&outer, &current().unwrap()));
            outer.job(JobKind::Indexed, || 3)
        });
        let s = summary.unwrap();
        assert_eq!(s.jobs, 1);
        assert_eq!(s.job_kinds[0].jobs, 0);
        assert_eq!(s.job_kinds[1].jobs, 1);
        assert!(current().is_none());
    }
    #[test]
    fn invalid_cpu_delta_is_explicit_and_unwind_restores_coverage() {
        let counter = |value| Counter {
            value: Some(value),
            supported: true,
            read_nanos: 3,
        };
        let sample = observation(counter(20), counter(19), 30);
        assert_eq!(sample.cpu_status, 2);
        assert_eq!(sample.thread_cpu_nanos, 0);
        assert_eq!(sample.counter_read_nanos, 6);
        assert!(std::panic::catch_unwind(|| covered(true, || panic!("test unwind"))).is_err());
        assert!(current().is_none());
    }
    #[test]
    fn disabled_creates_no_observation_and_does_not_change_work() {
        assert!(CpuTimer::start_if(false).is_none());
        let (result, summary) = covered(false, || 42);
        assert_eq!(result, 42);
        assert!(summary.is_none());
        assert!(current().is_none());
    }
    #[test]
    fn cpu_observations_are_monotonic_or_explicitly_unavailable() {
        let sample = CpuTimer::start_if(true).unwrap().finish();
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert_eq!(sample.cpu_status, 1);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert_eq!(sample.cpu_status, 0);
            assert_eq!(sample.thread_cpu_nanos, 0);
            assert_eq!(sample.counter_read_nanos, 0);
        }
    }
    #[test]
    fn cpu_timestamp_units_and_failures_are_checked() {
        assert_eq!(checked_cpu_nanos(2, 123), Some(2_000_000_123));
        for (seconds, nanos) in [(-1, 0), (0, -1), (0, 1_000_000_000), (i64::MAX, 0)] {
            assert_eq!(checked_cpu_nanos(seconds, nanos), None);
        }
        let invalid = observation(
            Counter {
                value: None,
                supported: true,
                read_nanos: 4,
            },
            Counter {
                value: Some(123),
                supported: true,
                read_nanos: 5,
            },
            20,
        );
        assert_eq!(
            (
                invalid.cpu_status,
                invalid.thread_cpu_nanos,
                invalid.counter_read_nanos
            ),
            (2, 0, 9)
        );
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn thread_cpu_distinguishes_owned_busy_work_from_parked_wall_time() {
        std::thread::spawn(|| {
            let busy = CpuTimer::start_if(true).unwrap();
            let baseline = Counter::read().value.expect("thread CPU clock available");
            let deadline = Instant::now() + std::time::Duration::from_secs(3);
            loop {
                let mut value = 1u64;
                for index in 0..4096u64 {
                    value = std::hint::black_box(value.wrapping_mul(31).wrapping_add(index));
                }
                std::hint::black_box(value);
                let consumed = Counter::read().value.unwrap().saturating_sub(baseline);
                if consumed >= 30_000_000 || Instant::now() >= deadline {
                    break;
                }
            }
            let busy = busy.finish();
            let parked = CpuTimer::start_if(true).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
            let parked = parked.finish();
            assert_eq!((busy.cpu_status, parked.cpu_status), (1, 1));
            assert!(busy.thread_cpu_nanos >= 20_000_000, "busy: {busy:?}");
            assert!(parked.wall_nanos >= 80_000_000, "parked: {parked:?}");
            assert!(
                parked.thread_cpu_nanos < busy.thread_cpu_nanos / 3 + 1_000_000,
                "thread CPU must exclude sleep: busy={busy:?}, parked={parked:?}"
            );
        })
        .join()
        .expect("owned CPU measurement thread");
    }
    #[test]
    fn ordered_work_is_transparent_and_cpu_coverage_restores_after_nested_scopes() {
        let pool = super::super::OutputPool::new(3).unwrap();
        let mut workers = vec![0usize; 3];
        let (rows, summary) = covered(true, || {
            super::super::run_ordered(Some(&pool), &mut workers, 13, |count, index| {
                *count += 1;
                index * 7
            })
        });
        assert_eq!(rows, (0..13).map(|i| i * 7).collect::<Vec<_>>());
        assert_eq!(workers.iter().sum::<usize>(), 13);
        let summary = summary.unwrap();
        assert_eq!(summary.jobs, 13);
        assert_eq!(summary.job_kinds[0].jobs, 13);
        assert_eq!(summary.job_kinds[1].jobs, 0);
        assert_eq!(summary.pool_timing.install_calls, 1);
        assert_eq!(summary.pool_timing.install_jobs, 13);
        assert_eq!(summary.pool_timing.scratch_lock_calls, 13);
        assert_eq!(summary.pool_timing.install_invalid_calls, 0);
        assert!(
            summary.pool_timing.install_wall_nanos >= summary.pool_timing.submit_to_entry_max_nanos
        );
        assert!(
            summary.pool_timing.scratch_lock_wall_nanos
                >= summary.pool_timing.scratch_lock_wall_max_nanos
        );
        assert_eq!(
            summary.cpu_valid_jobs + summary.cpu_unavailable_jobs + summary.cpu_invalid_jobs,
            13
        );
        assert!(current().is_none());
        let (_, outer) = covered(true, || {
            let (_, inner) = covered(true, || current().unwrap().job(JobKind::Ordered, || 1));
            assert_eq!(inner.unwrap().jobs, 1);
            current().unwrap().job(JobKind::Ordered, || 2)
        });
        assert_eq!(outer.unwrap().jobs, 1);
        assert!(current().is_none());
    }
    #[test]
    fn actual_indexed_installs_and_serial_fallback_keep_exact_callback_counts() {
        use light_dynamics::InstanceWorkers;
        let pool = super::super::OutputPool::new(2).unwrap();
        for count in [0, 1, 11] {
            let seen = (0..count).map(|_| AtomicU64::new(0)).collect::<Vec<_>>();
            let (_, summary) = covered(true, || {
                pool.run_indexed(count, &|index| {
                    seen[index].fetch_add(1, Ordering::Relaxed);
                })
            });
            assert!(seen.iter().all(|n| n.load(Ordering::Relaxed) == 1));
            let summary = summary.unwrap();
            assert_eq!(summary.pool_timing.install_calls, u64::from(count > 1));
            assert_eq!(summary.job_kinds[0].jobs, 0);
            assert_eq!(
                summary.job_kinds[1].jobs,
                if count > 1 { count as u64 } else { 0 }
            );
            assert_eq!(
                summary.pool_timing.install_jobs,
                if count > 1 { count as u64 } else { 0 }
            );
            assert_eq!(summary.pool_timing.install_jobs, summary.jobs);
            assert_eq!(summary.pool_timing.scratch_lock_calls, 0);
            assert_eq!(summary.pool_timing.install_invalid_calls, 0);
        }
    }
    #[test]
    fn frontiers_separate_blocked_scratch_acquisition_and_post_job_return_time() {
        let (_, summary) = covered(true, || {
            let coverage = current().unwrap();
            let timing = InstallTiming::begin();
            let scratch = Mutex::new(());
            let guard = scratch.lock().unwrap();
            let (entered, received) = std::sync::mpsc::channel();
            std::thread::sleep(std::time::Duration::from_millis(2));
            std::thread::scope(|scope| {
                let timing = &timing;
                let scratch = &scratch;
                let worker = scope.spawn(move || {
                    timing.job(|_| {
                        let started = Instant::now();
                        entered.send(()).unwrap();
                        let _guard = scratch.lock().unwrap();
                        timing.lock_acquired(started);
                        17
                    })
                });
                received.recv().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(2));
                drop(guard);
                assert_eq!(worker.join().unwrap(), 17);
            });
            std::thread::sleep(std::time::Duration::from_millis(2));
            timing.finish(&coverage);
        });
        let p = summary.unwrap().pool_timing;
        assert_eq!(
            (p.install_calls, p.install_jobs, p.scratch_lock_calls),
            (1, 1, 1)
        );
        assert_eq!((p.install_invalid_calls, p.install_empty_calls), (0, 0));
        assert!(p.submit_to_first_entry_nanos >= 1_000_000);
        assert!(p.scratch_lock_wall_nanos >= 1_000_000);
        assert!(p.last_exit_to_return_nanos >= 1_000_000);
        assert!(
            p.install_wall_nanos >= p.submit_to_first_entry_nanos + p.last_exit_to_return_nanos
        );
        let (_, empty) = covered(true, || InstallTiming::begin().finish(&current().unwrap()));
        let p = empty.unwrap().pool_timing;
        assert_eq!(
            (p.install_calls, p.install_jobs, p.install_empty_calls),
            (1, 0, 1)
        );
        assert_eq!(p.last_exit_to_return_nanos, 0);
        assert_eq!(p.submit_to_first_entry_nanos, 0);
    }
    #[test]
    fn callback_panic_still_propagates_and_restores_coverage_and_scratch_guards() {
        let pool = super::super::OutputPool::new(2).unwrap();
        let mut scratch = vec![0usize; 2];
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            covered(true, || {
                super::super::run_ordered(Some(&pool), &mut scratch, 9, |value, index| {
                    *value += 1;
                    assert_ne!(index, 3, "original callback panic");
                    index
                })
            })
        }));
        assert!(failed.is_err());
        assert!(current().is_none());
        let (rows, summary) = covered(true, || {
            super::super::run_ordered(Some(&pool), &mut scratch, 9, |_, index| index)
        });
        assert_eq!(rows, (0..9).collect::<Vec<_>>());
        assert_eq!(summary.unwrap().pool_timing.install_invalid_calls, 0);
    }
}
