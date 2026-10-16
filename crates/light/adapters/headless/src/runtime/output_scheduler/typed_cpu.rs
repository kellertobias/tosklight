//! Opt-in synchronous thread/process CPU observations. Async scopes report no CPU.
//! Process CPU includes all server threads, is tick-quantized, and overlaps nested scopes.
//! Runqueue counters never classify mutex, I/O or driver waits; worker CPU is unattributed.
use std::{
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug)]
struct Counters {
    cpu: u64,
    queue: u64,
    slices: u64,
}
#[derive(Clone, Copy)]
struct Reading {
    value: Option<Counters>,
    overhead: u64,
    process: Option<ProcessCounters>,
}
#[derive(Clone, Copy, Debug)]
struct ProcessCounters {
    ticks: u64,
    hz: u64,
}
#[cfg(any(target_os = "linux", test))]
fn parse_process_stat(text: &str) -> Option<u64> {
    // comm may contain spaces and parentheses. Fields after its final ')' start at field 3.
    let (_, tail) = text.rsplit_once(')')?;
    let fields: Vec<_> = tail.split_whitespace().collect();
    let user: u64 = fields.get(11)?.parse().ok()?;
    let system: u64 = fields.get(12)?.parse().ok()?;
    user.checked_add(system) // fields 14/15 only; never cutime/cstime (children).
}
#[cfg(any(target_os = "linux", test))]
fn parse_clock_ticks(bytes: &[u8]) -> Option<u64> {
    let word = std::mem::size_of::<usize>();
    if bytes.len() % (2 * word) != 0 {
        return None;
    }
    let mut hz = None;
    for pair in bytes.chunks_exact(2 * word) {
        let key = usize::from_ne_bytes(pair[..word].try_into().ok()?);
        let value = usize::from_ne_bytes(pair[word..].try_into().ok()?);
        if key == 0 {
            return hz;
        }
        if key == 17 {
            // AT_CLKTCK, supplied by the running kernel.
            let value = u64::try_from(value).ok()?;
            if hz.is_some() || value == 0 || value > 1_000_000_000 {
                return None;
            }
            hz = Some(value);
        }
    }
    None
}
#[cfg(target_os = "linux")]
fn read_process() -> Option<ProcessCounters> {
    static HZ: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    let hz = (*HZ.get_or_init(|| {
        std::fs::read("/proc/self/auxv")
            .ok()
            .and_then(|v| parse_clock_ticks(&v))
    }))?;
    let ticks = parse_process_stat(&std::fs::read_to_string("/proc/self/stat").ok()?)?;
    Some(ProcessCounters { ticks, hz })
}
fn process_delta(
    before: Option<ProcessCounters>,
    after: Option<ProcessCounters>,
) -> Option<(u64, u64, u64)> {
    let (a, b) = before.zip(after)?;
    if a.hz != b.hz || a.hz == 0 || a.hz > 1_000_000_000 {
        return None;
    }
    let ticks = b.ticks.checked_sub(a.ticks)?;
    let ns = u64::try_from(u128::from(ticks) * 1_000_000_000 / u128::from(a.hz)).ok()?;
    Some((ns, a.hz, 1_000_000_000_u64.div_ceil(a.hz)))
}
fn nanos(value: std::time::Duration) -> u64 {
    u64::try_from(value.as_nanos()).unwrap_or(u64::MAX)
}
#[cfg(any(target_os = "linux", test))]
fn parse(text: &str) -> Option<Counters> {
    let mut parts = text.split_whitespace();
    Some(Counters {
        cpu: parts.next()?.parse().ok()?,
        queue: parts.next()?.parse().ok()?,
        slices: parts.next()?.parse().ok()?,
    })
}
#[cfg(test)]
thread_local! { static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn reads_for_test() -> usize {
    READS.with(|r| r.get())
}
fn read() -> Reading {
    #[cfg(test)]
    READS.with(|r| r.set(r.get() + 1));
    let start = Instant::now();
    #[cfg(target_os = "linux")]
    let value = std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|text| parse(&text));
    #[cfg(not(target_os = "linux"))]
    let value = None;
    #[cfg(target_os = "linux")]
    let process = read_process();
    #[cfg(not(target_os = "linux"))]
    let process = None;
    Reading {
        value,
        process,
        overhead: nanos(start.elapsed()),
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Observation {
    calls: u64,
    wall_ns: u64,
    thread_cpu_ns: Option<u64>,
    process_cpu_ns: Option<u64>,
    process_cpu_tick_hz: Option<u64>,
    process_cpu_resolution_ns: Option<u64>,
    runnable_queue_ns: Option<u64>,
    timeslices: Option<u64>,
    counter_read_wall_ns: u64,
}
impl Observation {
    fn from_readings(wall_ns: u64, before: Reading, after: Reading, same_thread: bool) -> Self {
        // Identity is checked by the owning span, never inferred from counter monotonicity.
        let delta = same_thread
            .then_some(())
            .and(before.value)
            .zip(after.value)
            .and_then(|(a, b)| {
                Some(Counters {
                    cpu: b.cpu.checked_sub(a.cpu)?,
                    queue: b.queue.checked_sub(a.queue)?,
                    slices: b.slices.checked_sub(a.slices)?,
                })
            });
        let process = same_thread
            .then(|| process_delta(before.process, after.process))
            .flatten();
        Self {
            calls: 1,
            process_cpu_ns: process.map(|v| v.0),
            process_cpu_tick_hz: process.map(|v| v.1),
            process_cpu_resolution_ns: process.map(|v| v.2),
            wall_ns,
            thread_cpu_ns: delta.map(|x| x.cpu),
            runnable_queue_ns: delta.map(|x| x.queue),
            timeslices: delta.map(|x| x.slices),
            counter_read_wall_ns: before.overhead.saturating_add(after.overhead),
        }
    }
    pub(super) fn add(&mut self, next: Self) {
        let sum = |a: Option<u64>, b: Option<u64>| {
            if self.calls == 0 {
                b
            } else {
                a.zip(b).and_then(|(a, b)| a.checked_add(b))
            }
        };
        let compatible = self.calls == 0
            || (self.process_cpu_tick_hz.is_some()
                && self.process_cpu_tick_hz == next.process_cpu_tick_hz
                && self.process_cpu_resolution_ns == next.process_cpu_resolution_ns);
        self.process_cpu_ns = if compatible {
            sum(self.process_cpu_ns, next.process_cpu_ns)
        } else {
            None
        };
        if self.process_cpu_ns.is_some() {
            self.process_cpu_tick_hz = next.process_cpu_tick_hz;
            self.process_cpu_resolution_ns = next.process_cpu_resolution_ns;
        } else {
            self.process_cpu_tick_hz = None;
            self.process_cpu_resolution_ns = None;
        }
        self.thread_cpu_ns = sum(self.thread_cpu_ns, next.thread_cpu_ns);
        self.runnable_queue_ns = sum(self.runnable_queue_ns, next.runnable_queue_ns);
        self.timeslices = sum(self.timeslices, next.timeslices);
        self.calls = self.calls.saturating_add(next.calls);
        self.wall_ns = self.wall_ns.saturating_add(next.wall_ns);
        self.counter_read_wall_ns = self
            .counter_read_wall_ns
            .saturating_add(next.counter_read_wall_ns);
    }
    pub(super) fn json(&self) -> serde_json::Value {
        serde_json::json!({"calls":self.calls,"wall_ns":self.wall_ns,"thread_cpu_ns":self.thread_cpu_ns,
            "process_cpu_ns":self.process_cpu_ns,"process_cpu_tick_hz":self.process_cpu_tick_hz,
            "process_cpu_resolution_ns":self.process_cpu_resolution_ns,"runnable_queue_ns":self.runnable_queue_ns,"timeslices":self.timeslices,"counter_read_wall_ns":self.counter_read_wall_ns})
    }
}
pub(super) fn named<const N: usize>(
    names: &[&str; N],
    values: &[Observation; N],
) -> serde_json::Value {
    serde_json::Value::Object(
        names
            .iter()
            .zip(values.iter())
            .map(|(name, value)| ((*name).to_owned(), value.json()))
            .collect(),
    )
}
/// A synchronous interval; its caller must not keep this across an await.
/// Identity rejection also protects tests/accidental movement to another executing thread.
pub(super) struct Measurement {
    started: Instant,
    thread: ThreadId,
    before: Reading,
}
impl Measurement {
    pub(super) fn begin() -> Self {
        Self {
            started: Instant::now(),
            thread: thread::current().id(),
            before: read(),
        }
    }
    pub(super) fn finish(self) -> Observation {
        let wall = nanos(self.started.elapsed());
        let same = self.thread == thread::current().id();
        let after = if same {
            read()
        } else {
            Reading {
                value: None,
                process: None,
                overhead: 0,
            }
        };
        Observation::from_readings(wall, self.before, after, same)
    }
}
#[derive(Clone, Copy)]
pub(super) enum Major {
    Dynamic,
    Engine,
    Publish,
    Send,
}
const MAJOR_NAMES: [&str; 4] = ["dynamic", "engine", "publish", "send"];
pub(super) struct Tick {
    sample: u16,
    sampled_at_millis: Option<u64>,
    started: Instant,
    data: Arc<Mutex<[Observation; 4]>>,
}
impl Tick {
    pub(super) fn new(sample: u16) -> Self {
        Self {
            sample,
            sampled_at_millis: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok()),
            started: Instant::now(),
            data: Arc::new(Mutex::new([Observation::default(); 4])),
        }
    }
    pub(super) fn sample(&self) -> u16 {
        self.sample
    }
    pub(super) fn span(&self, stage: Major) -> MajorSpan {
        MajorSpan {
            stage,
            data: self.data.clone(),
            started: Instant::now(),
            cpu: (!matches!(stage, Major::Send)).then(Measurement::begin),
        }
    }
}
/// Optional span setup stays here so scheduler boundaries name their operation directly.
pub(super) fn publish(tick: Option<&Tick>) -> Option<MajorSpan> {
    tick.map(|tick| tick.span(Major::Publish))
}
pub(super) fn send(tick: Option<&Tick>) -> Option<MajorSpan> {
    tick.map(|tick| tick.span(Major::Send))
}
/// The synchronous dynamic-to-engine handoff closes Dynamic before starting Engine's
/// existing wall clock. With diagnostics off it only starts that original wall clock.
pub(super) struct Dynamic<'a> {
    tick: Option<&'a Tick>,
    span: Option<MajorSpan>,
}
impl<'a> Dynamic<'a> {
    pub(super) fn begin(tick: Option<&'a Tick>) -> Self {
        Self {
            tick,
            span: tick.map(|tick| tick.span(Major::Dynamic)),
        }
    }
    pub(super) fn engine(&mut self) -> (Instant, Option<MajorSpan>) {
        drop(self.span.take());
        let started = Instant::now();
        (started, self.tick.map(|tick| tick.span(Major::Engine)))
    }
}
pub(super) struct MajorSpan {
    stage: Major,
    data: Arc<Mutex<[Observation; 4]>>,
    started: Instant,
    cpu: Option<Measurement>,
}
impl Drop for MajorSpan {
    fn drop(&mut self) {
        let observation = self
            .cpu
            .take()
            .map(Measurement::finish)
            .unwrap_or(Observation {
                calls: 1,
                wall_ns: nanos(self.started.elapsed()),
                ..Observation::default()
            });
        self.data.lock().unwrap_or_else(|e| e.into_inner())[self.stage as usize].add(observation);
    }
}
impl Tick {
    fn payload(&self) -> serde_json::Value {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let parts = MAJOR_NAMES
            .iter()
            .zip(data.iter())
            .map(|(name, value)| ((*name).to_owned(), value.json()))
            .collect::<serde_json::Map<_, _>>();
        let total = Observation {
            calls: 1,
            wall_ns: nanos(self.started.elapsed()),
            ..Observation::default()
        };
        serde_json::json!({"schema":1,"role":1,"kind":0,"sample":self.sample,
            "sampled_at_millis":self.sampled_at_millis,"major":{"tick":total.json(),"parts":parts},
            "async_cpu_coverage":0,"worker_cpu_coverage":0,"runqueue_availability":0})
    }
}
impl Drop for Tick {
    fn drop(&mut self) {
        // Display emits JSON; Debug on serde_json::Value emits Object/Number wrappers.
        tracing::info!(typed_cpu_payload=%self.payload(), "bounded typed output major CPU sample");
    }
}
#[cfg(test)]
pub(super) fn assert_numeric_payload(value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::Number(number) => assert!(number.as_u64().is_some()),
        serde_json::Value::Array(values) => values.iter().for_each(assert_numeric_payload),
        serde_json::Value::Object(values) => values.values().for_each(assert_numeric_payload),
        _ => panic!("diagnostic payload values must be nullable unsigned integers"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(cpu: u64, queue: u64, slices: u64) -> Reading {
        Reading {
            value: Some(Counters { cpu, queue, slices }),
            process: None,
            overhead: 3,
        }
    }
    fn proc_read(ticks: u64, hz: u64) -> Reading {
        Reading {
            process: Some(ProcessCounters { ticks, hz }),
            ..r(0, 0, 0)
        }
    }
    #[test]
    fn process_stat_counts_self_user_system_not_children_and_handles_comm() {
        let stat = |user: &str, system: &str| {
            format!("42 (name ) with spaces) S 1 2 3 4 5 6 7 8 9 10 {user} {system} 999 999")
        };
        assert_eq!(parse_process_stat(&stat("10", "20")), Some(30));
        for text in [
            stat("-1", "2"),
            stat("x", "2"),
            stat("18446744073709551615", "1"),
            "42 (short) S 1".into(),
            "bad".into(),
        ] {
            assert_eq!(parse_process_stat(&text), None);
        }
    }
    fn auxv(pairs: &[(usize, usize)]) -> Vec<u8> {
        pairs
            .iter()
            .flat_map(|(key, value)| key.to_ne_bytes().into_iter().chain(value.to_ne_bytes()))
            .collect()
    }
    #[test]
    fn process_clock_uses_actual_auxv_and_rejects_missing_duplicate_invalid_values() {
        assert_eq!(
            parse_clock_ticks(&auxv(&[(6, 4096), (17, 250), (0, 0)])),
            Some(250)
        );
        for pairs in [
            vec![(0, 0)],
            vec![(17, 0), (0, 0)],
            vec![(17, 100)],
            vec![(17, 100), (17, 100), (0, 0)],
            vec![(17, 1_000_000_001), (0, 0)],
        ] {
            assert_eq!(parse_clock_ticks(&auxv(&pairs)), None);
        }
        assert_eq!(parse_clock_ticks(&[1, 2, 3]), None);
    }
    #[test]
    fn process_delta_quantization_zero_identity_regression_overflow_and_hz() {
        let measure = |a, b, same| Observation::from_readings(100, a, b, same);
        let zero = measure(proc_read(100, 100), proc_read(100, 100), true);
        assert_eq!(zero.process_cpu_ns, Some(0));
        assert_eq!(zero.process_cpu_tick_hz, Some(100));
        assert_eq!(zero.process_cpu_resolution_ns, Some(10_000_000));
        assert_eq!(
            measure(proc_read(0, 300), proc_read(1, 300), true).process_cpu_resolution_ns,
            Some(3_333_334)
        );
        assert_eq!(
            measure(proc_read(0, 100), proc_read(3, 100), true).process_cpu_ns,
            Some(30_000_000)
        );
        for (a, b, same) in [
            (proc_read(1, 100), proc_read(0, 100), true),
            (proc_read(0, 100), proc_read(1, 200), true),
            (proc_read(0, 0), proc_read(1, 0), true),
            (proc_read(0, 1), proc_read(u64::MAX, 1), true),
            (proc_read(0, 100), proc_read(1, 100), false),
            (r(0, 0, 0), proc_read(1, 100), true),
        ] {
            let observation = measure(a, b, same);
            assert_eq!(observation.process_cpu_ns, None);
            assert_eq!(observation.process_cpu_tick_hz, None);
            assert_eq!(observation.process_cpu_resolution_ns, None);
        }
    }
    #[test]
    fn process_aggregation_missing_hz_mismatch_and_overflow_are_poisoned() {
        let observation =
            |hz| Observation::from_readings(100, proc_read(0, hz), proc_read(1, hz), true);
        for next in [
            Observation::from_readings(100, r(0, 0, 0), r(0, 0, 0), true),
            observation(200),
        ] {
            let mut total = Observation::default();
            total.add(observation(100));
            total.add(next);
            total.add(observation(100));
            assert_eq!(total.process_cpu_ns, None);
            assert_eq!(total.process_cpu_tick_hz, None);
        }
        let mut total = Observation {
            calls: 1,
            process_cpu_ns: Some(u64::MAX),
            process_cpu_tick_hz: Some(100),
            process_cpu_resolution_ns: Some(10_000_000),
            ..Observation::default()
        };
        total.add(observation(100));
        assert_eq!(total.process_cpu_ns, None);
    }
    #[test]
    fn major_display_payload_roundtrips_strict_numeric_json_with_own_anchor() {
        let mut tick = Tick::new(7);
        tick.sampled_at_millis = Some(1234);
        {
            let _publish = tick.span(Major::Publish);
        }
        let encoded = format!("{}", tick.payload());
        let parsed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_numeric_payload(&parsed);
        assert_eq!(parsed["schema"], 1);
        assert_eq!(parsed["role"], 1);
        assert_eq!(parsed["kind"], 0);
        assert_eq!(parsed["sample"], 7);
        assert_eq!(parsed["sampled_at_millis"], 1234);
        assert_eq!(parsed["major"]["parts"]["publish"]["calls"], 1);
        assert!(parsed["major"]["parts"]["send"]["thread_cpu_ns"].is_null());
        assert_eq!(parsed.as_object().unwrap().len(), 9);
        tick.sampled_at_millis = None;
        assert!(tick.payload()["sampled_at_millis"].is_null());
        struct Writer(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_target(false)
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || Writer(output.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || drop(tick));
        let emitted = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let json = emitted.trim().split_once("typed_cpu_payload=").unwrap().1;
        let actual: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_numeric_payload(&actual);
        assert_eq!(actual["sample"], 7);
        assert!(actual["sampled_at_millis"].is_null());
    }
    #[test]
    fn optional_helpers_preserve_off_no_reads_and_ordered_handoff() {
        let before = reads_for_test();
        assert!(publish(None).is_none());
        assert!(send(None).is_none());
        let mut off = Dynamic::begin(None);
        let (_, engine) = off.engine();
        assert!(engine.is_none());
        assert_eq!(reads_for_test(), before);
        let tick = Tick::new(1);
        let mut dynamic = Dynamic::begin(Some(&tick));
        let (_, engine) = dynamic.engine();
        {
            let data = tick.data.lock().unwrap();
            assert_eq!(data[Major::Dynamic as usize].calls, 1);
            assert_eq!(data[Major::Engine as usize].calls, 0);
        }
        drop(engine);
        assert_eq!(tick.data.lock().unwrap()[Major::Engine as usize].calls, 1);
    }
    #[test]
    fn fake_clock_deschedule_is_wall_not_cpu_or_mutex_time() {
        let x = Observation::from_readings(1000, r(100, 20, 1), r(105, 40, 2), true);
        assert_eq!(x.thread_cpu_ns, Some(5));
        assert_eq!(x.runnable_queue_ns, Some(20));
        assert_eq!(x.wall_ns, 1000);
        assert_eq!(x.counter_read_wall_ns, 6);
    }
    #[test]
    fn fake_clock_identity_change_and_regression_are_unavailable() {
        let moved = Observation::from_readings(1000, r(1, 1, 1), r(9, 9, 9), false);
        assert_eq!(moved.thread_cpu_ns, None);
        assert_eq!(moved.runnable_queue_ns, None);
        let regressed = Observation::from_readings(1000, r(9, 1, 1), r(8, 2, 2), true);
        assert_eq!(regressed.thread_cpu_ns, None);
    }
    #[test]
    fn unavailable_read_poison_is_not_zero_or_partial_sum() {
        let mut x = Observation::default();
        x.add(Observation::from_readings(
            100,
            r(1, 1, 1),
            r(2, 2, 2),
            true,
        ));
        x.add(Observation::from_readings(
            200,
            r(1, 1, 1),
            Reading {
                value: None,
                process: None,
                overhead: 4,
            },
            true,
        ));
        x.add(Observation::from_readings(
            300,
            r(1, 1, 1),
            r(2, 2, 2),
            true,
        ));
        assert_eq!(x.calls, 3);
        assert_eq!(x.wall_ns, 600);
        assert_eq!(x.thread_cpu_ns, None);
        assert_eq!(x.counter_read_wall_ns, 19);
    }
    #[test]
    fn first_read_failure_and_sum_overflow_stay_nullable() {
        let missing = Reading {
            value: None,
            process: None,
            overhead: 7,
        };
        let x = Observation::from_readings(50, missing, r(9, 9, 9), true);
        assert_eq!(x.thread_cpu_ns, None);
        assert_eq!(x.counter_read_wall_ns, 10);
        let mut sum = Observation {
            calls: 1,
            thread_cpu_ns: Some(u64::MAX),
            ..Observation::default()
        };
        sum.add(Observation::from_readings(1, r(0, 0, 0), r(1, 1, 1), true));
        assert_eq!(sum.thread_cpu_ns, None);
    }
    #[test]
    fn parser_rejects_missing_malformed_negative_or_overflow_counters() {
        for text in ["", "1 2", "1 -2 3", "a 2 3", "18446744073709551616 2 3"] {
            assert!(parse(text).is_none());
        }
        assert_eq!(parse("1 0 2\n").unwrap().queue, 0);
    }
    #[test]
    fn moved_measurement_is_unavailable_and_async_send_never_reads_cpu() {
        let measurement = Measurement::begin();
        let x = std::thread::spawn(move || measurement.finish())
            .join()
            .unwrap();
        assert_eq!(x.thread_cpu_ns, None);
        assert_eq!(x.process_cpu_ns, None);
        let tick = Tick::new(1);
        {
            let span = tick.span(Major::Send);
            assert!(span.cpu.is_none());
        }
        let data = tick.data.lock().unwrap();
        assert_eq!(data[Major::Send as usize].calls, 1);
        assert_eq!(data[Major::Send as usize].counter_read_wall_ns, 0);
        assert_eq!(data[Major::Send as usize].process_cpu_ns, None);
    }
    #[test]
    fn synchronous_error_and_panic_still_finish_named_span() {
        let tick = Tick::new(1);
        let result: Result<(), ()> = (|| {
            let _span = tick.span(Major::Publish);
            Err(())
        })();
        assert!(result.is_err());
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _span = tick.span(Major::Engine);
            panic!("observed unwind")
        }));
        let data = tick.data.lock().unwrap();
        assert_eq!(data[Major::Publish as usize].calls, 1);
        assert_eq!(data[Major::Engine as usize].calls, 1);
    }
}
