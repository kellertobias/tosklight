//! Opt-in synchronous CPU observations. Covered jobs are not total pool CPU: Rayon sorts and
//! unrelated/deferred jobs can run concurrently. Section-to-job lag includes caller setup, earlier jobs and scratch locking; it is wall time, never CPU wait.
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, LazyLock,
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
        #[cfg(target_os = "linux")]
        {
            let started = Instant::now();
            let value = rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime);
            let value = u64::try_from(value.tv_sec)
                .ok()
                .and_then(|seconds| seconds.checked_mul(1_000_000_000))
                .zip(
                    u64::try_from(value.tv_nsec)
                        .ok()
                        .filter(|n| *n < 1_000_000_000),
                )
                .and_then(|(seconds, nanos)| seconds.checked_add(nanos));
            Self {
                value,
                supported: true,
                read_nanos: ns(started.elapsed()),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                value: None,
                supported: false,
                read_nanos: 0,
            }
        }
    }
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
}
#[derive(Default)]
struct Totals {
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
    pub(super) fn job<T>(&self, work: impl FnOnce() -> T) -> T {
        let lag = ns(self.submitted.elapsed());
        let timer = CpuTimer::start_if(true).expect("enabled coverage");
        let result = work();
        let observed = timer.finish();
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
        result
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
        #[cfg(target_os = "linux")]
        assert_eq!(sample.cpu_status, 1);
        #[cfg(not(target_os = "linux"))]
        {
            assert_eq!(sample.cpu_status, 0);
            assert_eq!(sample.thread_cpu_nanos, 0);
            assert_eq!(sample.counter_read_nanos, 0);
        }
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
        assert_eq!(
            summary.cpu_valid_jobs + summary.cpu_unavailable_jobs + summary.cpu_invalid_jobs,
            13
        );
        assert!(current().is_none());
        let (_, outer) = covered(true, || {
            let (_, inner) = covered(true, || current().unwrap().job(|| 1));
            assert_eq!(inner.unwrap().jobs, 1);
            current().unwrap().job(|| 2)
        });
        assert_eq!(outer.unwrap().jobs, 1);
        assert!(current().is_none());
    }
}
