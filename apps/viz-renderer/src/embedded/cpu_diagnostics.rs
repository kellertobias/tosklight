//! Opt-in private, numeric-only synchronous renderer-thread observations.
//! Background decoder/provider CPU is unattributed. Wall minus CPU has no causal meaning.
//! Queue/slices require sched_schedstats enabled at both endpoints; otherwise they are null.
//! Runtime scheduler-accounting toggles or availability limits still prevent treating queue zero
//! as evidence that the thread did not wait.
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use viz_render::{RenderCpuObserver, RenderCpuStage};

const LIMIT: u32 = 1024;
const STAGES: usize = 8; // seven fixed phases and one inclusive iteration parent
#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum Stage {
    ProviderPollAdopt = 0,
    PhysicsOverlayPrepare = 1,
    RenderSubmit = 2,
    CaptureReadbackSetup = 3,
    CaptureReadbackWait = 4,
    PixelPack = 5,
    PixelSerializeSend = 6,
    Iteration = 7,
}
const ALL_STAGES: [Stage; STAGES] = [
    Stage::ProviderPollAdopt,
    Stage::PhysicsOverlayPrepare,
    Stage::RenderSubmit,
    Stage::CaptureReadbackSetup,
    Stage::CaptureReadbackWait,
    Stage::PixelPack,
    Stage::PixelSerializeSend,
    Stage::Iteration,
];
impl From<RenderCpuStage> for Stage {
    fn from(value: RenderCpuStage) -> Self {
        match value {
            RenderCpuStage::RenderSubmit => Self::RenderSubmit,
            RenderCpuStage::CaptureReadbackSetup => Self::CaptureReadbackSetup,
            RenderCpuStage::CaptureReadbackWait => Self::CaptureReadbackWait,
            RenderCpuStage::PixelPack => Self::PixelPack,
        }
    }
}

pub(crate) struct Sampler {
    epoch: Instant,
    last: Option<Duration>,
    count: u32,
}
impl Sampler {
    pub fn from_env() -> Option<Self> {
        Self::configured(
            std::env::var("LIGHT_VIZ_TYPED_CPU_DIAGNOSTICS")
                .ok()
                .as_deref(),
            Instant::now,
        )
    }
    fn configured(value: Option<&str>, clock: impl FnOnce() -> Instant) -> Option<Self> {
        (value == Some("1")).then(|| Self {
            epoch: clock(),
            last: None,
            count: 0,
        })
    }
    fn select(&mut self, elapsed: Duration) -> bool {
        if self.count >= LIMIT
            || self
                .last
                .is_some_and(|last| elapsed.saturating_sub(last) < Duration::from_secs(1))
        {
            return false;
        }
        self.last = Some(elapsed);
        self.count += 1;
        true
    }
    pub fn sample(&mut self, source_frame: u64, presented: u64, transport: u8) -> Option<Sample> {
        let elapsed = self.epoch.elapsed();
        if !self.select(elapsed) {
            return None;
        }
        let mut sample = Sample {
            epoch: Instant::now(),
            ordinal: self.count,
            relative_ns: ns(elapsed),
            sampled_at_millis: unix_millis(SystemTime::now()),
            source_before: source_frame,
            source_after: source_frame,
            presented_before: presented,
            presented_after: presented,
            transport,
            outcome: 0,
            redrawn: false,
            send_outcome: 0,
            stages: std::array::from_fn(|_| Phase::default()),
        };
        sample.begin_phase(Stage::Iteration);
        Some(sample)
    }
}
// Wall-clock anchor is metadata only. Missing/stepped clocks require external window qualification.
fn unix_millis(value: SystemTime) -> Option<u64> {
    u64::try_from(value.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
}
fn ns(value: Duration) -> u64 {
    value.as_nanos().min(u64::MAX as u128) as u64
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Counters {
    cpu: u64,
    queue: u64,
    slices: u64,
}
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn parse_schedstat(text: &str) -> Option<Counters> {
    let mut fields = text.split_whitespace();
    let result = Counters {
        cpu: fields.next()?.parse().ok()?,
        queue: fields.next()?.parse().ok()?,
        slices: fields.next()?.parse().ok()?,
    };
    fields.next().is_none().then_some(result)
}
fn counters() -> Option<Counters> {
    #[cfg(target_os = "linux")]
    {
        parse_schedstat(&std::fs::read_to_string("/proc/thread-self/schedstat").ok()?)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
fn queue_accounting_enabled() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/sys/kernel/sched_schedstats")
            .ok()
            .is_some_and(|value| value.trim() == "1")
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}
struct Stamp {
    wall: Instant,
    thread: std::thread::ThreadId,
    counters: Option<Counters>,
    read_ns: u64,
    queue_available: bool,
}
impl Stamp {
    fn read() -> Self {
        let wall = Instant::now();
        let thread = std::thread::current().id();
        let counters = counters();
        let queue_available = queue_accounting_enabled();
        Self {
            wall,
            thread,
            counters,
            read_ns: ns(wall.elapsed()),
            queue_available,
        }
    }
}
fn delta(start: Option<Counters>, end: Option<Counters>, same_thread: bool) -> Option<Counters> {
    if !same_thread {
        return None;
    }
    let (a, b) = (start?, end?);
    Some(Counters {
        cpu: b.cpu.checked_sub(a.cpu)?,
        queue: b.queue.checked_sub(a.queue)?,
        slices: b.slices.checked_sub(a.slices)?,
    })
}
#[derive(Default)]
struct Phase {
    start: Option<Stamp>,
    calls: u32,
    wall_ns: u64,
    counters: Option<Counters>,
    read_ns: u64,
    queue_available: bool,
}
impl Phase {
    fn finish(&mut self, end: Stamp) {
        let Some(start) = self.start.take() else {
            return;
        };
        let observed = delta(start.counters, end.counters, start.thread == end.thread);
        let queue_available = start.queue_available && end.queue_available;
        self.queue_available = if self.calls == 0 {
            queue_available
        } else {
            self.queue_available && queue_available
        };
        self.counters = if self.calls == 0 {
            observed
        } else {
            self.counters.zip(observed).and_then(|(a, b)| {
                Some(Counters {
                    cpu: a.cpu.checked_add(b.cpu)?,
                    queue: a.queue.checked_add(b.queue)?,
                    slices: a.slices.checked_add(b.slices)?,
                })
            })
        };
        self.calls = self.calls.saturating_add(1);
        self.wall_ns = self
            .wall_ns
            .saturating_add(ns(end.wall.saturating_duration_since(start.wall)));
        self.read_ns = self
            .read_ns
            .saturating_add(start.read_ns)
            .saturating_add(end.read_ns);
    }
}
pub(crate) struct Sample {
    epoch: Instant,
    ordinal: u32,
    relative_ns: u64,
    sampled_at_millis: Option<u64>,
    source_before: u64,
    source_after: u64,
    presented_before: u64,
    presented_after: u64,
    // transport: 1 copy, 2 shared; outcome: 0 unfinished, 1 draw returned, 2 error, 3 channel ended.
    // redrawn marks an attempted redraw; presented_after > presented_before follows existing presented bookkeeping.
    // send_outcome: 0 not applicable, 1 successful write, 2 encode/write failure.
    transport: u8,
    outcome: u8,
    redrawn: bool,
    send_outcome: u8,
    stages: [Phase; STAGES],
}
impl Sample {
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Sampler::configured(Some("1"), Instant::now)
            .unwrap()
            .sample(7, 3, 1)
            .unwrap()
    }
    #[cfg(test)]
    pub(crate) fn phase_result(&self, stage: usize) -> (u32, u8) {
        (self.stages[stage].calls, self.send_outcome)
    }
    pub fn begin_phase(&mut self, stage: Stage) {
        self.stages[stage as usize].start = Some(Stamp::read());
    }
    pub fn end_phase(&mut self, stage: Stage) {
        self.stages[stage as usize].finish(Stamp::read());
    }
    pub fn redrawn(&mut self) {
        self.redrawn = true;
    }
    pub fn sent(&mut self, success: bool) {
        self.send_outcome = if success { 1 } else { 2 };
    }
    pub fn finish(&mut self, source: u64, presented: u64, outcome: u8) {
        self.source_after = source;
        self.presented_after = presented;
        self.outcome = outcome;
    }
    fn row(&self, stage: usize) -> serde_json::Value {
        let phase = &self.stages[stage];
        serde_json::json!({"schema":1,"role":2,"stage":stage,"ordinal":self.ordinal,"relative_ns":self.relative_ns,"sampled_at_millis":self.sampled_at_millis,
            "source_before":self.source_before,"source_after":self.source_after,"presented_before":self.presented_before,"presented_after":self.presented_after,
            "transport":self.transport,"outcome":self.outcome,"redrawn":u8::from(self.redrawn),"send_outcome":self.send_outcome,
            "calls":phase.calls,"wall_ns":phase.wall_ns,"thread_cpu_ns":phase.counters.map(|c|c.cpu),"queue_ns":phase.counters.filter(|_|phase.queue_available).map(|c|c.queue),"slices":phase.counters.filter(|_|phase.queue_available).map(|c|c.slices),"counter_read_wall_ns":phase.read_ns})
    }
}
impl RenderCpuObserver for Sample {
    fn begin(&mut self, stage: RenderCpuStage) {
        self.begin_phase(stage.into());
    }
    fn end(&mut self, stage: RenderCpuStage) {
        self.end_phase(stage.into());
    }
}
impl Drop for Sample {
    fn drop(&mut self) {
        for stage in ALL_STAGES {
            if self.stages[stage as usize].start.is_some() {
                self.end_phase(stage);
            }
        }
        // Export is outside every measured scope; no payload, identifiers or free-form errors.
        // This measures stage-row export only, excluding emission of the overhead row itself.
        let export = Instant::now();
        for stage in 0..STAGES {
            eprintln!("{}", self.row(stage));
        }
        eprintln!(
            "{}",
            serde_json::json!({"schema":1,"role":2,"ordinal":self.ordinal,"sampled_at_millis":self.sampled_at_millis,"export_wall_ns":ns(export.elapsed()),"window_wall_ns":ns(self.epoch.elapsed())})
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_does_not_read_clock_or_create_sampler() {
        for value in [None, Some("0"), Some("true"), Some("")] {
            assert!(Sampler::configured(value, || panic!("OFF clock read")).is_none());
        }
        let mut disabled: Option<Sampler> = None;
        let selected = disabled
            .as_mut()
            .and_then(|_| -> Option<Sample> { panic!("OFF sample read") });
        assert!(selected.is_none());
    }
    #[test]
    fn numeric_rows_bind_outcome_and_shared_not_applicable() {
        let mut sample = Sample::for_test();
        sample.transport = 2;
        sample.finish(8, 3, 2);
        let row = sample.row(6);
        assert_eq!(row["calls"], 0);
        assert!(row["thread_cpu_ns"].is_null());
        assert_eq!(row["source_before"], 7);
        assert_eq!(row["source_after"], 8);
        assert_eq!(row["presented_after"], 3);
        assert_eq!(row["outcome"], 2);
        assert!(
            row.as_object()
                .unwrap()
                .values()
                .all(|v| v.is_number() || v.is_null())
        );
    }
    #[test]
    fn absolute_anchor_is_checked_nullable_metadata() {
        assert_eq!(
            unix_millis(UNIX_EPOCH + Duration::from_millis(1234)),
            Some(1234)
        );
        assert_eq!(unix_millis(UNIX_EPOCH - Duration::from_millis(1)), None);
        let mut sample = Sample::for_test();
        sample.sampled_at_millis = Some(1234);
        assert_eq!(sample.row(0)["sampled_at_millis"].as_u64(), Some(1234));
        sample.sampled_at_millis = None;
        assert!(sample.row(0)["sampled_at_millis"].is_null());
    }
    #[test]
    fn sampling_has_no_catchup_and_hard_cap() {
        let mut s = Sampler {
            epoch: Instant::now(),
            last: None,
            count: 0,
        };
        assert!(s.select(Duration::ZERO));
        assert!(!s.select(Duration::from_millis(999)));
        assert!(s.select(Duration::from_secs(100)));
        assert!(!s.select(Duration::from_secs(100)));
        for i in 101..1123 {
            assert!(s.select(Duration::from_secs(i)));
        }
        assert!(!s.select(Duration::from_secs(2000)));
    }
    #[test]
    fn counters_are_independent_and_checked() {
        let a = parse_schedstat("100 200 3\n");
        let b = parse_schedstat("110 220 4\n");
        assert_eq!(
            delta(a, b, true),
            Some(Counters {
                cpu: 10,
                queue: 20,
                slices: 1
            })
        );
        assert_eq!(delta(b, a, true), None);
        assert_eq!(delta(a, b, false), None);
        assert_eq!(delta(None, b, true), None);
        assert_eq!(parse_schedstat("1 2"), None);
        assert_eq!(parse_schedstat("1 2 3 4"), None);
    }
    #[test]
    fn fake_wall_does_not_become_cpu_and_invalid_is_sticky() {
        let now = Instant::now();
        let thread = std::thread::current().id();
        let stamp = |offset: u64, cpu: Option<u64>| Stamp {
            wall: now + Duration::from_nanos(offset),
            thread,
            counters: cpu.map(|cpu| Counters {
                cpu,
                queue: cpu * 2,
                slices: cpu,
            }),
            read_ns: 2,
            queue_available: true,
        };
        let mut p = Phase::default();
        p.start = Some(stamp(0, Some(0)));
        p.finish(stamp(1000, Some(10)));
        assert_eq!(p.wall_ns, 1000);
        assert_eq!(p.counters.unwrap().cpu, 10);
        assert_eq!(p.counters.unwrap().queue, 20);
        p.start = Some(stamp(1001, None));
        p.finish(stamp(1002, Some(11)));
        assert!(p.counters.is_none());
        p.start = Some(stamp(1003, Some(11)));
        p.finish(stamp(1004, Some(12)));
        assert!(p.counters.is_none());
    }
}
