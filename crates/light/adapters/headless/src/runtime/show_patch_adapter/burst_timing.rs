//! Private numeric-only telemetry for the isolated manual-clock sync bench.
//! Production adapters never enable it; no transport response or persisted data changes.

use parking_lot::Mutex;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

#[derive(Default)]
struct Phase {
    samples: u64,
    total_micros: u128,
    maximum_micros: u128,
}

#[derive(Default)]
struct Aggregate {
    transactions: u64,
    phases: BTreeMap<&'static str, Phase>,
}

impl Aggregate {
    fn record(&mut self, name: &'static str, elapsed: Duration) -> Option<serde_json::Value> {
        let phase = self.phases.entry(name).or_default();
        phase.samples += 1;
        phase.total_micros += elapsed.as_micros();
        phase.maximum_micros = phase.maximum_micros.max(elapsed.as_micros());
        if name != "total" {
            return None;
        }
        self.transactions += 1;
        if !self.transactions.is_multiple_of(100) {
            return None;
        }
        Some(serde_json::json!({
            "transactions": self.transactions,
            "phases": self.phases.iter().map(|(name, phase)| (*name, serde_json::json!({
                "samples": phase.samples,
                "total_micros": phase.total_micros,
                "maximum_micros": phase.maximum_micros,
            }))).collect::<BTreeMap<_, _>>()
        }))
    }
}

pub(super) fn record(phase: &'static str, elapsed: Duration) {
    record_request_phase(phase, elapsed);
    static METRICS: OnceLock<Mutex<Aggregate>> = OnceLock::new();
    let summary = METRICS
        .get_or_init(Mutex::default)
        .lock()
        .record(phase, elapsed);
    if let Some(summary) = summary {
        tracing::info!(numeric_phases = %summary, "isolated sync transaction timings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_keep_stage_samples_and_total_distinct() {
        let mut metrics = Aggregate::default();
        for _ in 0..99 {
            assert!(metrics.record("commit", Duration::from_micros(7)).is_none());
            assert!(metrics.record("total", Duration::from_micros(11)).is_none());
        }
        assert!(
            metrics
                .record("commit", Duration::from_micros(17))
                .is_none()
        );
        let summary = metrics.record("total", Duration::from_micros(23)).unwrap();
        assert_eq!(summary["transactions"], 100);
        assert_eq!(summary["phases"]["commit"]["samples"], 100);
        assert_eq!(summary["phases"]["commit"]["total_micros"], 710);
        assert_eq!(summary["phases"]["commit"]["maximum_micros"], 17);
        assert_eq!(summary["phases"]["total"]["total_micros"], 1112);
        assert_eq!(summary["phases"]["total"]["maximum_micros"], 23);
    }
}

// Request diagnostics are opt-in AND restricted to the existing isolated clock bench.
// All counters describe the initiating thread. Wall minus CPU is not an I/O diagnosis.
use std::{
    cell::RefCell,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};
const REQUEST_LIMIT: u64 = 2048;
static REQUEST_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    diagnostic_enabled(
        std::env::var("LIGHT_ARCHITECT_SYNC_REQUEST_DIAGNOSTICS")
            .ok()
            .as_deref(),
    )
});
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);
static REQUEST_ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
thread_local! { static REQUEST: RefCell<Option<RequestTrace>> = const { RefCell::new(None) }; }

fn diagnostic_enabled(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
}
fn admit(counter: &AtomicU64) -> Option<u64> {
    let mut count = counter.load(Ordering::Relaxed);
    while count < REQUEST_LIMIT {
        match counter.compare_exchange_weak(count, count + 1, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return Some(count + 1),
            Err(current) => count = current,
        }
    }
    None
}
fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}
fn epoch_micros() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_micros()
        .try_into()
        .ok()
}

/// Seven sequential parts of the original commit; this is not a SQLite-only timer.
#[derive(Clone, Copy)]
pub(in crate::runtime) enum CommitPart {
    ContractValidation,
    StoreApply,
    FeedPublish,
    DocumentApply,
    AttributesInstall,
    PsnInstall,
    ColorModelInstall,
}
const PARTS: [CommitPart; 7] = [
    CommitPart::ContractValidation,
    CommitPart::StoreApply,
    CommitPart::FeedPublish,
    CommitPart::DocumentApply,
    CommitPart::AttributesInstall,
    CommitPart::PsnInstall,
    CommitPart::ColorModelInstall,
];
impl CommitPart {
    fn label(self) -> &'static str {
        match self {
            Self::ContractValidation => "contract_validation",
            Self::StoreApply => "store_apply",
            Self::FeedPublish => "feed_publish",
            Self::DocumentApply => "document_apply",
            Self::AttributesInstall => "attributes_install",
            Self::PsnInstall => "psn_install",
            Self::ColorModelInstall => "color_model_install",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ThreadCounters {
    cpu_ns: u64,
    runnable_queue_ns: u64,
    slices: u64,
}
#[cfg(any(target_os = "linux", test))]
fn parse_schedstat(text: &str) -> Option<ThreadCounters> {
    let mut fields = text.split_whitespace();
    Some(ThreadCounters {
        cpu_ns: fields.next()?.parse().ok()?,
        runnable_queue_ns: fields.next()?.parse().ok()?,
        slices: fields.next()?.parse().ok()?,
    })
}
#[derive(Clone, Copy)]
struct CounterSample {
    counters: Option<ThreadCounters>,
    read_ns: u64,
}
impl CounterSample {
    fn read() -> Self {
        let started = Instant::now();
        #[cfg(target_os = "linux")]
        let counters = std::fs::read_to_string("/proc/thread-self/schedstat")
            .ok()
            .and_then(|text| parse_schedstat(&text));
        #[cfg(not(target_os = "linux"))]
        let counters = None;
        Self {
            counters,
            read_ns: nanos(started.elapsed()),
        }
    }
}
#[derive(Default, Clone)]
struct Observation {
    calls: u64,
    wall_ns: u64,
    cpu_ns: Option<u64>,
    runnable_queue_ns: Option<u64>,
    slices: Option<u64>,
    counter_read_ns: u64,
    first_start_us: Option<u64>,
    last_end_us: Option<u64>,
}
impl Observation {
    fn add(
        &mut self,
        wall_ns: u64,
        before: CounterSample,
        after: CounterSample,
        start_us: u64,
        end_us: u64,
    ) {
        let delta = before.counters.zip(after.counters).and_then(|(a, b)| {
            Some(ThreadCounters {
                cpu_ns: b.cpu_ns.checked_sub(a.cpu_ns)?,
                runnable_queue_ns: b.runnable_queue_ns.checked_sub(a.runnable_queue_ns)?,
                slices: b.slices.checked_sub(a.slices)?,
            })
        });
        let first = self.calls == 0;
        self.cpu_ns = accumulate(self.cpu_ns, delta.map(|d| d.cpu_ns), first);
        self.runnable_queue_ns = accumulate(
            self.runnable_queue_ns,
            delta.map(|d| d.runnable_queue_ns),
            first,
        );
        self.slices = accumulate(self.slices, delta.map(|d| d.slices), first);
        self.calls = self.calls.saturating_add(1);
        self.wall_ns = self.wall_ns.saturating_add(wall_ns);
        self.counter_read_ns = self
            .counter_read_ns
            .saturating_add(before.read_ns)
            .saturating_add(after.read_ns);
        self.first_start_us.get_or_insert(start_us);
        self.last_end_us = Some(end_us);
    }
    fn json(&self) -> serde_json::Value {
        serde_json::json!({"calls":self.calls,"wall_ns":self.wall_ns,"thread_cpu_ns":self.cpu_ns,"runnable_queue_ns":self.runnable_queue_ns,"timeslices":self.slices,"counter_read_wall_ns":self.counter_read_ns,"first_start_mono_us":self.first_start_us,"last_end_mono_us":self.last_end_us})
    }
}
fn accumulate(previous: Option<u64>, next: Option<u64>, first: bool) -> Option<u64> {
    if first {
        next
    } else {
        previous.zip(next).map(|(a, b)| a.saturating_add(b))
    }
}
struct RequestTrace {
    ordinal: u64,
    started: Instant,
    epoch_start_us: Option<u64>,
    before: CounterSample,
    success: Option<bool>,
    parts: [Observation; 7],
    outer: BTreeMap<&'static str, Phase>,
}
impl RequestTrace {
    fn row(self) -> serde_json::Value {
        let ended = Instant::now();
        let wall = ended.duration_since(self.started);
        let epoch_end = epoch_micros();
        let after = CounterSample::read();
        let mut whole = Observation::default();
        whole.add(
            nanos(wall),
            self.before,
            after,
            mono(self.started),
            mono(ended),
        );
        serde_json::json!({"schema":1,"ordinal":self.ordinal,"success":self.success,"epoch_start_us":self.epoch_start_us,"epoch_end_us":epoch_end,"request":whole.json(),"parts":PARTS.iter().map(|part| (part.label(), self.parts[*part as usize].json())).collect::<BTreeMap<_,_>>(),"outer_inclusive":self.outer.iter().map(|(name, phase)| (*name, serde_json::json!({"calls":phase.samples,"wall_us":phase.total_micros,"max_wall_us":phase.maximum_micros}))).collect::<BTreeMap<_,_>>()})
    }
}
fn mono(at: Instant) -> u64 {
    at.saturating_duration_since(*REQUEST_ORIGIN)
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(in crate::runtime) struct RequestScope {
    ordinal: u64,
}
impl RequestScope {
    pub(in crate::runtime) fn begin(isolated_clock: bool) -> Option<Self> {
        if !isolated_clock || !*REQUEST_ENABLED {
            return None;
        }
        Self::open(admit(&REQUEST_COUNT)?)
    }
    fn open(ordinal: u64) -> Option<Self> {
        if REQUEST.with(|request| request.borrow().is_some()) {
            return None;
        }
        LazyLock::force(&REQUEST_ORIGIN);
        let before = CounterSample::read();
        let started = Instant::now();
        let epoch_start_us = epoch_micros();
        REQUEST.with(|request| {
            *request.borrow_mut() = Some(RequestTrace {
                ordinal,
                started,
                epoch_start_us,
                before,
                success: None,
                parts: std::array::from_fn(|_| Observation::default()),
                outer: BTreeMap::new(),
            })
        });
        Some(Self { ordinal })
    }
    pub(in crate::runtime) fn complete(&mut self, success: bool) {
        REQUEST.with(|request| {
            if let Some(request) = request.borrow_mut().as_mut()
                && request.ordinal == self.ordinal
            {
                request.success = Some(success);
            }
        });
    }
    fn take_row(&self) -> Option<serde_json::Value> {
        let trace = REQUEST.with(|request| {
            let mut current = request.borrow_mut();
            if current.as_ref().is_some_and(|r| r.ordinal == self.ordinal) {
                current.take()
            } else {
                None
            }
        });
        trace.map(RequestTrace::row)
    }
}
impl Drop for RequestScope {
    fn drop(&mut self) {
        if let Some(row) = self.take_row() {
            tracing::info!(numeric_request = %row, "isolated correlated sync request");
        }
    }
}
struct SpanStart {
    at: Instant,
    before: CounterSample,
    ordinal: u64,
}
pub(in crate::runtime) struct CommitSpan {
    part: CommitPart,
    start: Option<SpanStart>,
}
impl CommitSpan {
    pub(in crate::runtime) fn begin(part: CommitPart) -> Self {
        let ordinal = REQUEST.with(|request| request.borrow().as_ref().map(|r| r.ordinal));
        let start = ordinal.map(|ordinal| {
            let before = CounterSample::read();
            SpanStart {
                at: Instant::now(),
                before,
                ordinal,
            }
        });
        Self { part, start }
    }
}
impl Drop for CommitSpan {
    fn drop(&mut self) {
        let Some(start) = &self.start else {
            return;
        };
        let ended = Instant::now();
        let elapsed = nanos(ended.duration_since(start.at));
        let after = CounterSample::read();
        REQUEST.with(|request| {
            if let Some(request) = request.borrow_mut().as_mut()
                && request.ordinal == start.ordinal
            {
                request.parts[self.part as usize].add(
                    elapsed,
                    start.before,
                    after,
                    mono(start.at),
                    mono(ended),
                );
            }
        });
    }
}
fn record_request_phase(name: &'static str, elapsed: Duration) {
    REQUEST.with(|request| {
        if let Some(request) = request.borrow_mut().as_mut() {
            let phase = request.outer.entry(name).or_default();
            phase.samples = phase.samples.saturating_add(1);
            phase.total_micros = phase.total_micros.saturating_add(elapsed.as_micros());
            phase.maximum_micros = phase.maximum_micros.max(elapsed.as_micros());
        }
    });
}

#[cfg(test)]
mod request_tests {
    use super::*;
    fn sample(cpu: u64, queue: u64, slices: u64, read_ns: u64) -> CounterSample {
        CounterSample {
            counters: Some(ThreadCounters {
                cpu_ns: cpu,
                runnable_queue_ns: queue,
                slices,
            }),
            read_ns,
        }
    }
    #[test]
    fn opt_in_is_exact_and_disabled_has_no_span_clock() {
        for value in [None, Some(""), Some("0"), Some("TRUE"), Some("true ")] {
            assert!(!diagnostic_enabled(value));
        }
        assert!(diagnostic_enabled(Some("1")));
        assert!(diagnostic_enabled(Some("true")));
        assert!(RequestScope::begin(false).is_none());
        assert!(CommitSpan::begin(CommitPart::StoreApply).start.is_none());
        assert!(REQUEST.with(|r| r.borrow().is_none()));
    }
    #[test]
    fn admission_is_bounded_even_for_exhausted_counter() {
        let counter = AtomicU64::new(REQUEST_LIMIT - 1);
        assert_eq!(admit(&counter), Some(REQUEST_LIMIT));
        assert_eq!(admit(&counter), None);
        assert_eq!(admit(&AtomicU64::new(u64::MAX)), None);
    }
    #[test]
    fn schedstat_requires_three_unsigned_counters() {
        let parsed = parse_schedstat("120 30 4\n").unwrap();
        assert_eq!(
            (parsed.cpu_ns, parsed.runnable_queue_ns, parsed.slices),
            (120, 30, 4)
        );
        for bad in ["", "12 3", "-1 2 3", "1 nope 3", "18446744073709551616 2 3"] {
            assert!(parse_schedstat(bad).is_none());
        }
    }
    #[test]
    fn observation_preserves_cpu_queue_and_observer_cost_separately() {
        let mut part = Observation::default();
        part.add(100, sample(10, 20, 1, 7), sample(70, 50, 3, 11), 4, 5);
        assert_eq!(part.wall_ns, 100);
        assert_eq!(part.cpu_ns, Some(60));
        assert_eq!(part.runnable_queue_ns, Some(30));
        assert_eq!(part.slices, Some(2));
        assert_eq!(part.counter_read_ns, 18);
        part.add(200, sample(70, 50, 3, 13), sample(90, 55, 4, 17), 6, 8);
        assert_eq!(part.calls, 2);
        assert_eq!(part.wall_ns, 300);
        assert_eq!(part.cpu_ns, Some(80));
        assert_eq!(part.first_start_us, Some(4));
        assert_eq!(part.last_end_us, Some(8));
    }
    #[test]
    fn unavailable_and_reset_cpu_counters_are_never_reported_as_zero() {
        let mut part = Observation::default();
        part.add(100, sample(70, 50, 3, 1), sample(10, 60, 4, 2), 0, 1);
        assert!(part.cpu_ns.is_none());
        assert!(part.runnable_queue_ns.is_none());
        part.add(
            100,
            CounterSample {
                counters: None,
                read_ns: 1,
            },
            sample(20, 70, 5, 2),
            1,
            2,
        );
        assert!(part.cpu_ns.is_none());
        assert!(part.json()["thread_cpu_ns"].is_null());
    }
    #[test]
    fn sequential_parts_are_enclosed_by_request_wall_and_keep_outer_inclusive() {
        let mut scope = RequestScope::open(76201).unwrap();
        for part in PARTS {
            let _span = CommitSpan::begin(part);
            std::hint::black_box(41 + 1);
        }
        record_request_phase("commit", Duration::from_micros(5));
        record_request_phase("finalize_runtime", Duration::from_micros(8));
        record_request_phase("total", Duration::from_micros(15));
        scope.complete(true);
        let row = scope.take_row().unwrap();
        let mut sum = 0;
        for part in PARTS {
            assert_eq!(row["parts"][part.label()]["calls"], 1);
            sum += row["parts"][part.label()]["wall_ns"].as_u64().unwrap();
        }
        assert!(sum <= row["request"]["wall_ns"].as_u64().unwrap());
        assert_eq!(row["outer_inclusive"]["total"]["wall_us"], 15);
        assert_eq!(row["outer_inclusive"]["commit"]["wall_us"], 5);
        assert_eq!(row["success"], true);
    }
    #[test]
    fn unwind_records_only_entered_part_and_restores_scope() {
        let mut scope = RequestScope::open(76202).unwrap();
        let outcome = std::panic::catch_unwind(|| {
            let _span = CommitSpan::begin(CommitPart::StoreApply);
            panic!("controlled diagnostic unwind");
        });
        assert!(outcome.is_err());
        scope.complete(false);
        let row = scope.take_row().unwrap();
        assert_eq!(row["parts"]["store_apply"]["calls"], 1);
        assert_eq!(row["parts"]["feed_publish"]["calls"], 0);
        assert_eq!(row["success"], false);
        assert!(REQUEST.with(|r| r.borrow().is_none()));
    }
    #[test]
    fn retired_span_cannot_mutate_new_request() {
        let first = RequestScope::open(76203).unwrap();
        let old = CommitSpan::begin(CommitPart::StoreApply);
        let _ = first.take_row().unwrap();
        let second = RequestScope::open(76204).unwrap();
        drop(old);
        let row = second.take_row().unwrap();
        assert_eq!(row["parts"]["store_apply"]["calls"], 0);
    }
}
