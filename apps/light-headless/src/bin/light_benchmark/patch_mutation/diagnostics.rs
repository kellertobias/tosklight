//! Opt-in, bounded request attribution for the released Patch benchmark only.
//! No requests, gates, timer boundary, shared production hooks or SQLite settings are changed.
use super::{ActionContext, ActionError, PatchPerformancePhase, ShowId, phase_name};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

type Phases = BTreeMap<PatchPerformancePhase, Vec<Duration>>;

#[derive(Debug, Serialize)]
pub struct Report {
    pub qualification: &'static str,
    pub host: Host,
    pub warmups: Vec<Request>,
    pub measured: Vec<Request>,
}

#[derive(Debug, Serialize)]
pub struct Host {
    pub operating_system: &'static str,
    pub available_parallelism: Option<usize>,
    pub cpuinfo: Readout,
    pub meminfo: Readout,
    pub pressure_cpu: Readout,
    pub pressure_io: Readout,
    pub loadavg: Readout,
    pub sqlite_configuration_source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Request {
    pub request_id: String,
    pub fixture_count: usize,
    pub invocation_index: usize,
    pub expected_revision: u64,
    pub returned_revision: Option<u64>,
    pub error: Option<String>,
    pub total_microseconds: f64,
    pub request_bytes: usize,
    pub response_bytes: Option<usize>,
    pub outer_segments_microseconds: BTreeMap<&'static str, f64>,
    pub phases_microseconds: BTreeMap<&'static str, Vec<f64>>,
    pub phase_accounting: Accounting,
    pub active_show_boundaries: Vec<ActiveShowBoundary>,
    pub before: Resources,
    pub after: Resources,
}

#[derive(Debug, Serialize)]
pub struct Accounting {
    pub compile_is_nested_in: &'static str,
    pub service_nonnested_covered_microseconds: f64,
    pub service_unattributed_microseconds: Option<f64>,
    pub qualification: &'static str,
    pub active_show_boundary_nesting: &'static str,
    pub service_unattributed_after_transaction_begin_microseconds: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Resources {
    pub utc: String,
    pub read_wall_microseconds: f64,
    pub process_stat: Readout,
    pub thread_stat: Readout,
    pub thread_schedstat: Readout,
    pub process_io: Readout,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Readout {
    Available { value: String },
    Unavailable { reason: String },
}

pub(super) struct Invocation<'a> {
    pub(super) index: usize,
    pub(super) collector: &'a mut Option<Collector>,
}

pub(super) struct Collector {
    warmup_limit: usize,
    sample_limit: usize,
    report: Report,
    boundaries: BoundaryLog,
}

impl Collector {
    pub(super) fn from_environment(warmups: usize, samples: usize) -> Option<Self> {
        enabled(
            std::env::var("LIGHT_PATCH_REQUEST_DIAGNOSTICS")
                .ok()
                .as_deref(),
        )
        .then(|| Self::new(warmups, samples))
    }

    fn new(warmups: usize, samples: usize) -> Self {
        Self {
            warmup_limit: warmups,
            sample_limit: samples,
            boundaries: Arc::new(Mutex::new(Boundaries {
                rows: Vec::with_capacity(warmups.saturating_add(samples).saturating_mul(2)),
                limit: warmups.saturating_add(samples).saturating_mul(2),
            })),
            report: Report {
                qualification: "Diagnostic observer overhead remains; Linux resource reads bracket but are outside the decode-to-response timer. Inclusive wall phases are not parked wait or CPU time.",
                host: Host::read(),
                warmups: Vec::with_capacity(warmups),
                measured: Vec::with_capacity(samples),
            },
        }
    }

    pub(super) fn boundary_log(&self) -> BoundaryLog {
        Arc::clone(&self.boundaries)
    }

    pub(super) fn begin(
        &self,
        request_id: &str,
        fixture_count: usize,
        index: usize,
        revision: u64,
        phases: &Mutex<Phases>,
    ) -> Option<Observation> {
        let has_capacity = if index < self.warmup_limit {
            self.report.warmups.len() < self.warmup_limit
        } else {
            self.report.measured.len() < self.sample_limit
        };
        if !admitted(index, self.warmup_limit, self.sample_limit) || !has_capacity {
            return None;
        }
        // Release the diagnostic phase mutex before the external resource reads.
        let cursors = {
            let phases = phases.lock();
            cursors(&phases)
        };
        Some(Observation {
            request_id: request_id.to_owned(),
            fixture_count,
            index,
            revision,
            cursors,
            boundary_cursor: self.boundaries.lock().rows.len(),
            segments: BTreeMap::new(),
            last: Duration::ZERO,
            before: Resources::read(),
        })
    }

    pub(super) fn record(
        &mut self,
        mut observation: Observation,
        total: Duration,
        request_bytes: usize,
        result: &Result<(u64, usize), String>,
        phases: &Mutex<Phases>,
    ) {
        // First read after the original elapsed stop, before copying phase vectors/formatting.
        let after = Resources::read();
        observation.close(
            total,
            if result.is_ok() {
                "response_json_encode"
            } else {
                "failed_segment"
            },
        );
        let slices = slices(&phases.lock(), &observation.cursors);
        let boundaries = self.boundaries.lock().rows[observation.boundary_cursor..].to_vec();
        let mut accounting = accounting(&observation.segments, &slices);
        // The first call is already enclosed by SnapshotLoad; only subtract the second call.
        if boundaries.len() == 2
            && boundaries[0].correlation_id == boundaries[1].correlation_id
            && boundaries[0].show_id == boundaries[1].show_id
            && boundaries[0].expected_revision == boundaries[1].expected_revision
            && boundaries.iter().all(|boundary| {
                boundary.request_id.as_deref() == Some(observation.request_id.as_str())
            })
        {
            accounting.service_unattributed_after_transaction_begin_microseconds = accounting
                .service_unattributed_microseconds
                .and_then(|residual| {
                    (residual >= boundaries[1].total_microseconds)
                        .then_some(residual - boundaries[1].total_microseconds)
                });
        }
        let row = Request {
            request_id: observation.request_id,
            fixture_count: observation.fixture_count,
            invocation_index: observation.index,
            expected_revision: observation.revision,
            returned_revision: result.as_ref().ok().map(|(revision, _)| *revision),
            error: result.as_ref().err().cloned(),
            total_microseconds: micros(total),
            request_bytes,
            response_bytes: result.as_ref().ok().map(|(_, bytes)| *bytes),
            outer_segments_microseconds: observation.segments,
            phases_microseconds: slices,
            phase_accounting: accounting,
            active_show_boundaries: boundaries,
            before: observation.before,
            after,
        };
        if observation.index < self.warmup_limit {
            if self.report.warmups.len() < self.warmup_limit {
                self.report.warmups.push(row);
            }
        } else if self.report.measured.len() < self.sample_limit {
            self.report.measured.push(row);
        }
    }

    // A failed scenario returns no normal report. Retain completed/failed rows on stderr only
    // after its original timer stops, without replacing or retrying the original refusal.
    pub(super) fn emit_failure(&self) {
        match serde_json::to_string(&self.report) {
            Ok(payload) => eprintln!("patch_request_diagnostics_partial={payload}"),
            Err(error) => eprintln!("patch_request_diagnostics_unavailable={error}"),
        }
    }

    pub(super) fn finish(self) -> Report {
        self.report
    }
}

/// Only allocated for an opted-in, finite scenario. Calls remain in invocation order.
pub(super) type BoundaryLog = Arc<Mutex<Boundaries>>;
pub(super) struct Boundaries {
    rows: Vec<ActiveShowBoundary>,
    limit: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActiveShowBoundary {
    pub request_id: Option<String>,
    pub correlation_id: uuid::Uuid,
    pub show_id: uuid::Uuid,
    pub expected_revision: Option<u64>,
    pub total_microseconds: f64,
    pub segments_microseconds: BTreeMap<&'static str, f64>,
    pub failed_segment: Option<&'static str>,
    pub error: Option<String>,
    pub store_open: Option<StoreOpen>,
}

pub(super) struct BoundaryTiming {
    started: Instant,
    last: Duration,
    segments: BTreeMap<&'static str, f64>,
    next: &'static str,
    store_open: Option<StoreOpen>,
}
impl BoundaryTiming {
    fn close(&mut self, elapsed: Duration, name: &'static str) {
        self.segments
            .insert(name, micros(elapsed.saturating_sub(self.last)));
        self.last = elapsed;
    }
}

/// Leaves the operation, error and both actual store/document calls unchanged.
/// Disabled diagnostics perform no diagnostic clocks, allocations or log locks.
pub(super) fn observe_boundary<T>(
    log: Option<&BoundaryLog>,
    context: &ActionContext,
    show_id: ShowId,
    operation: impl FnOnce(&mut Option<BoundaryTiming>) -> Result<T, ActionError>,
) -> Result<T, ActionError> {
    let Some(log) = log else {
        return operation(&mut None);
    };
    let admitted = {
        let log = log.lock();
        log.rows.len() < log.limit
    };
    if !admitted {
        return operation(&mut None);
    }
    let mut timing = Some(BoundaryTiming {
        started: Instant::now(),
        last: Duration::ZERO,
        segments: BTreeMap::new(),
        next: "validation",
        store_open: None,
    });
    let result = operation(&mut timing);
    let mut timing = timing.expect("boundary timing remains present");
    let total = timing.started.elapsed();
    let failed_segment = result.as_ref().err().map(|_| timing.next);
    timing.close(
        total,
        if result.is_err() {
            "failed_segment"
        } else {
            "completion"
        },
    );
    let row = ActiveShowBoundary {
        request_id: context.request_id.clone(),
        correlation_id: context.correlation_id,
        show_id: show_id.0,
        expected_revision: context.expected_revision,
        total_microseconds: micros(total),
        segments_microseconds: timing.segments,
        failed_segment,
        error: result.as_ref().err().map(|error| error.message.clone()),
        store_open: timing.store_open,
    };
    let mut log = log.lock();
    if log.rows.len() < log.limit {
        log.rows.push(row);
    }
    result
}

pub(super) fn mark_boundary(timing: &mut Option<BoundaryTiming>, name: &'static str) {
    if let Some(timing) = timing {
        timing.close(timing.started.elapsed(), name);
        timing.next = match name {
            "validation" => "open_store",
            "open_store" => "portable_document",
            "portable_document" => "unit_setup",
            _ => "completion",
        };
    }
}

/// Nested within the existing open_store segment; never add to parent accounting.
#[derive(Clone, Debug, Serialize)]
pub struct StoreOpen {
    pub total_microseconds: f64,
    pub segments_microseconds: BTreeMap<&'static str, f64>,
    pub failed_phase: Option<&'static str>,
}

pub(super) fn observe_store_open<T, E>(
    timing: &mut Option<BoundaryTiming>,
    operation: impl FnOnce(Option<&mut dyn FnMut(light_show::ShowStoreOpenPhase)>) -> Result<T, E>,
) -> Result<T, E> {
    let Some(timing) = timing else {
        return operation(None);
    };
    use light_show::ShowStoreOpenPhase as Phase;
    let started = Instant::now();
    let mut last = Duration::ZERO;
    let mut segments = BTreeMap::new();
    let mut next = "playback_contract";
    let result = operation(Some(&mut |phase| {
        let (name, following) = match phase {
            Phase::PlaybackContract => ("playback_contract", "connection_open"),
            Phase::ConnectionOpen => ("connection_open", "connection_configure"),
            Phase::ConnectionConfigure => ("connection_configure", "schema_version"),
            Phase::SchemaVersion => ("schema_version", "migration_branch"),
            Phase::Migration => ("migration_branch", "integrity"),
            Phase::Integrity => ("integrity", "schema_validation"),
            Phase::SchemaValidation => ("schema_validation", "identity"),
            Phase::Identity => ("identity", "completion"),
        };
        let elapsed = started.elapsed();
        segments.insert(name, micros(elapsed.saturating_sub(last)));
        last = elapsed;
        next = following;
    }));
    let total = started.elapsed();
    let failed_phase = result.as_ref().err().map(|_| next);
    segments.insert(
        if result.is_err() {
            "failed_segment"
        } else {
            "completion"
        },
        micros(total.saturating_sub(last)),
    );
    timing.store_open = Some(StoreOpen {
        total_microseconds: micros(total),
        segments_microseconds: segments,
        failed_phase,
    });
    result
}

pub(super) struct Observation {
    request_id: String,
    fixture_count: usize,
    index: usize,
    revision: u64,
    cursors: BTreeMap<PatchPerformancePhase, usize>,
    boundary_cursor: usize,
    segments: BTreeMap<&'static str, f64>,
    last: Duration,
    before: Resources,
}

impl Observation {
    fn close(&mut self, elapsed: Duration, name: &'static str) {
        self.segments
            .insert(name, micros(elapsed.saturating_sub(self.last)));
        self.last = elapsed;
    }
}

pub(super) fn mark(observation: &mut Option<Observation>, name: &'static str, started: Instant) {
    if let Some(observation) = observation {
        observation.close(started.elapsed(), name);
    }
}

fn enabled(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
}
fn admitted(index: usize, warmups: usize, samples: usize) -> bool {
    index < warmups.saturating_add(samples)
}
fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn cursors(phases: &Phases) -> BTreeMap<PatchPerformancePhase, usize> {
    phases
        .iter()
        .map(|(phase, values)| (*phase, values.len()))
        .collect()
}

fn slices(
    phases: &Phases,
    before: &BTreeMap<PatchPerformancePhase, usize>,
) -> BTreeMap<&'static str, Vec<f64>> {
    phases
        .iter()
        .filter_map(|(phase, values)| {
            let offset = before.get(phase).copied().unwrap_or(0);
            let values = values.get(offset..)?;
            (!values.is_empty()).then(|| {
                (
                    phase_name(*phase),
                    values.iter().copied().map(micros).collect(),
                )
            })
        })
        .collect()
}

fn accounting(
    segments: &BTreeMap<&'static str, f64>,
    phases: &BTreeMap<&'static str, Vec<f64>>,
) -> Accounting {
    // Compile is inside CandidatePreparation. ConflictDetection may legitimately occur twice.
    let covered = phases
        .iter()
        .filter(|(name, _)| **name != "compile")
        .flat_map(|(_, values)| values)
        .sum::<f64>();
    let residual = segments
        .get("service_handle_inclusive")
        .and_then(|total| (*total >= covered).then_some(*total - covered));
    Accounting {
        active_show_boundary_nesting: "Both calls are inside service_handle_inclusive. In this benchmark the first begin_active_show is inside snapshot_load; the second is transaction begin outside snapshot_load. Boundary segments telescope to their inclusive total and must not be added to their parent. Adjusted residual subtracts only the second total when exactly two same-request calls exist; not CPU or parked wait.",
        service_unattributed_after_transaction_begin_microseconds: None,
        compile_is_nested_in: "candidate_preparation_including_compile",
        service_nonnested_covered_microseconds: covered,
        service_unattributed_microseconds: residual,
        qualification: "Compile excluded once from coverage; all other hook invocations retained. Residual is uninstrumented wall/observer overhead, not mutex wait. None means incomplete/invalid coverage.",
    }
}

fn readout(path: &str) -> Readout {
    if !cfg!(target_os = "linux") {
        return Readout::Unavailable {
            reason: "Linux /proc unavailable".to_owned(),
        };
    }
    match fs::read_to_string(path) {
        Ok(value) => Readout::Available { value },
        Err(error) => Readout::Unavailable {
            reason: error.to_string(),
        },
    }
}

impl Resources {
    fn read() -> Self {
        let started = Instant::now();
        let utc = chrono::Utc::now().to_rfc3339();
        let process_stat = readout("/proc/self/stat");
        let thread_stat = readout("/proc/thread-self/stat");
        let thread_schedstat = readout("/proc/thread-self/schedstat");
        let process_io = readout("/proc/self/io");
        Self {
            utc,
            read_wall_microseconds: micros(started.elapsed()),
            process_stat,
            thread_stat,
            thread_schedstat,
            process_io,
        }
    }
}

impl Host {
    fn read() -> Self {
        Self {
            operating_system: std::env::consts::OS,
            available_parallelism: std::thread::available_parallelism().ok().map(usize::from),
            cpuinfo: readout("/proc/cpuinfo"),
            meminfo: readout("/proc/meminfo"),
            pressure_cpu: readout("/proc/pressure/cpu"),
            pressure_io: readout("/proc/pressure/io"),
            loadavg: readout("/proc/loadavg"),
            sqlite_configuration_source: "shared/show/connection.rs: busy_timeout5000, WAL, synchronousNORMAL, foreign_keysON; configured-source evidence only, not an extra live PRAGMA query",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_malformed_opt_in_do_not_collect() {
        for value in [
            None,
            Some(""),
            Some("false"),
            Some("yes"),
            Some(" true"),
            Some("0"),
        ] {
            assert!(!enabled(value));
        }
        assert!(enabled(Some("1")));
        assert!(enabled(Some("true")));
    }

    #[test]
    fn admission_preserves_eight_warmups_and_forty_samples() {
        assert_eq!((0..100).filter(|i| admitted(*i, 8, 40)).count(), 48);
        assert!(admitted(7, 8, 40));
        assert!(admitted(8, 8, 40));
        assert!(admitted(47, 8, 40));
        assert!(!admitted(48, 8, 40));
    }

    #[test]
    fn duplicate_conflict_events_stay_with_their_request() {
        let mut phases = Phases::new();
        phases.insert(
            PatchPerformancePhase::ConflictDetection,
            vec![Duration::from_micros(2)],
        );
        let before = cursors(&phases);
        phases
            .get_mut(&PatchPerformancePhase::ConflictDetection)
            .unwrap()
            .extend([Duration::from_micros(3), Duration::from_micros(5)]);
        assert_eq!(
            slices(&phases, &before)["conflict_detection"],
            vec![3.0, 5.0]
        );
        let next = cursors(&phases);
        phases
            .get_mut(&PatchPerformancePhase::ConflictDetection)
            .unwrap()
            .push(Duration::from_micros(7));
        assert_eq!(slices(&phases, &next)["conflict_detection"], vec![7.0]);
    }

    #[test]
    fn compile_is_not_double_counted_in_service_coverage() {
        let segments = BTreeMap::from([("service_handle_inclusive", 100.0)]);
        let phases = BTreeMap::from([
            ("candidate_preparation_including_compile", vec![60.0]),
            ("compile", vec![50.0]),
            ("conflict_detection", vec![3.0, 5.0]),
            ("persistence", vec![12.0]),
        ]);
        let result = accounting(&segments, &phases);
        assert_eq!(result.service_nonnested_covered_microseconds, 80.0);
        assert_eq!(result.service_unattributed_microseconds, Some(20.0));
    }

    #[test]
    fn coverage_larger_than_service_is_not_called_zero_wait() {
        let segments = BTreeMap::from([("service_handle_inclusive", 4.0)]);
        let phases = BTreeMap::from([("persistence", vec![5.0])]);
        assert_eq!(
            accounting(&segments, &phases).service_unattributed_microseconds,
            None
        );
    }

    #[test]
    fn contiguous_segments_telescope_and_failure_keeps_partial_observation() {
        let mut collector = Collector::new(8, 40);
        let phases = Mutex::new(Phases::new());
        let mut observation = collector.begin("request", 100, 8, 10, &phases).unwrap();
        observation.close(Duration::from_micros(3), "request_decode");
        observation.close(Duration::from_micros(8), "application_mapping");
        collector.record(
            observation,
            Duration::from_micros(11),
            22,
            &Err("original service refusal".to_owned()),
            &phases,
        );
        let retained = serde_json::to_value(&collector.report).unwrap();
        assert_eq!(retained["measured"][0]["error"], "original service refusal");
        assert_eq!(retained["measured"][0]["request_id"], "request");
        let report = collector.finish();
        assert!(report.warmups.is_empty());
        assert_eq!(report.measured.len(), 1);
        let row = &report.measured[0];
        assert_eq!(
            row.outer_segments_microseconds.values().sum::<f64>(),
            row.total_microseconds
        );
        assert_eq!(row.error.as_deref(), Some("original service refusal"));
        assert_eq!(row.returned_revision, None);
        assert_eq!(row.expected_revision, 10);
        assert_eq!(row.outer_segments_microseconds["failed_segment"], 3.0);
    }

    #[test]
    fn warmups_are_separate_and_zero_capacity_never_starts() {
        let mut collector = Collector::new(1, 1);
        let phases = Mutex::new(Phases::new());
        for index in 0..2 {
            let observation = collector.begin("bounded", 1, index, 0, &phases).unwrap();
            collector.record(
                observation,
                Duration::from_micros(1),
                2,
                &Ok((1, 3)),
                &phases,
            );
        }
        assert!(collector.begin("overflow", 1, 2, 0, &phases).is_none());
        let report = collector.finish();
        assert_eq!(report.warmups.len(), 1);
        assert_eq!(report.measured.len(), 1);
        assert!(
            Collector::new(0, 0)
                .begin("none", 1, 0, 0, &phases)
                .is_none()
        );
    }
    fn boundary_context(request: &str) -> ActionContext {
        ActionContext::operator(
            uuid::Uuid::from_u128(1),
            uuid::Uuid::from_u128(3),
            super::super::ActionSource::Http,
        )
        .with_request_id(request)
        .with_expected_revision(7)
    }

    #[test]
    fn repeated_boundaries_are_correlated_and_do_not_leak_into_next_request() {
        let mut collector = Collector::new(0, 2);
        let log = collector.boundary_log();
        let phases = Mutex::new(Phases::new());
        for (index, name) in ["first", "second"].into_iter().enumerate() {
            let observation = collector.begin(name, 100, index, 7, &phases).unwrap();
            let context = boundary_context(name);
            for _ in 0..2 {
                observe_boundary(
                    Some(&log),
                    &context,
                    ShowId(uuid::Uuid::from_u128(9)),
                    |timing| {
                        mark_boundary(timing, "validation");
                        mark_boundary(timing, "open_store");
                        mark_boundary(timing, "portable_document");
                        mark_boundary(timing, "unit_setup");
                        Ok(())
                    },
                )
                .unwrap();
            }
            collector.record(
                observation,
                Duration::from_millis(1),
                1,
                &Ok((8, 1)),
                &phases,
            );
        }
        assert_eq!(log.lock().rows.len(), 4);
        observe_boundary(
            Some(&log),
            &boundary_context("overflow"),
            ShowId(uuid::Uuid::from_u128(9)),
            |timing| {
                assert!(timing.is_none());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(log.lock().rows.len(), 4);
        for (row, name) in collector.report.measured.iter().zip(["first", "second"]) {
            assert_eq!(row.active_show_boundaries.len(), 2);
            for boundary in &row.active_show_boundaries {
                assert_eq!(boundary.request_id.as_deref(), Some(name));
                assert_eq!(
                    boundary.correlation_id,
                    row.active_show_boundaries[0].correlation_id
                );
                assert!(
                    (boundary.segments_microseconds.values().sum::<f64>()
                        - boundary.total_microseconds)
                        .abs()
                        < 0.000001
                );
            }
        }
    }

    #[test]
    fn boundary_failure_keeps_original_error_context_and_telescoping_segments() {
        let collector = Collector::new(0, 1);
        let log = collector.boundary_log();
        let context = boundary_context("failed");
        let original = ActionError::new(
            super::super::ActionErrorKind::Unavailable,
            "original open refusal",
        )
        .at_revision(19);
        let result: Result<(), ActionError> = observe_boundary(
            Some(&log),
            &context,
            ShowId(uuid::Uuid::from_u128(9)),
            |timing| {
                mark_boundary(timing, "validation");
                Err(original.clone())
            },
        );
        assert_eq!(result.unwrap_err(), original);
        let rows = &log.lock().rows;
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.request_id.as_deref(), Some("failed"));
        assert_eq!(row.correlation_id, context.correlation_id);
        assert_eq!(row.expected_revision, Some(7));
        assert_eq!(row.failed_segment, Some("open_store"));
        assert_eq!(row.error.as_deref(), Some("original open refusal"));
        assert!(
            (row.segments_microseconds.values().sum::<f64>() - row.total_microseconds).abs()
                < 0.000001
        );
        assert!(!row.segments_microseconds.contains_key("portable_document"));
    }

    #[test]
    fn disabled_or_full_boundary_log_passes_through_without_a_timer() {
        let context = boundary_context("disabled");
        let original =
            ActionError::new(super::super::ActionErrorKind::Invalid, "unchanged refusal");
        for log in [None, Some(Collector::new(0, 0).boundary_log())] {
            let result: Result<(), ActionError> = observe_boundary(
                log.as_ref(),
                &context,
                ShowId(uuid::Uuid::from_u128(9)),
                |timing| {
                    assert!(timing.is_none());
                    mark_boundary(timing, "open_store");
                    Err(original.clone())
                },
            );
            assert_eq!(result.unwrap_err(), original);
            if let Some(log) = log {
                assert!(log.lock().rows.is_empty());
            }
        }
    }

    #[test]
    fn adjusted_service_residual_subtracts_only_transaction_begin_not_snapshot_begin() {
        let mut collector = Collector::new(0, 1);
        let log = collector.boundary_log();
        let phases = Mutex::new(Phases::new());
        let mut observation = collector.begin("paired", 100, 0, 7, &phases).unwrap();
        observation
            .segments
            .insert("service_handle_inclusive", 100.0);
        phases.lock().insert(
            PatchPerformancePhase::SnapshotLoad,
            vec![Duration::from_micros(30)],
        );
        let context = boundary_context("paired");
        for total in [20.0, 40.0] {
            log.lock().rows.push(ActiveShowBoundary {
                request_id: context.request_id.clone(),
                correlation_id: context.correlation_id,
                show_id: uuid::Uuid::from_u128(9),
                expected_revision: Some(7),
                total_microseconds: total,
                segments_microseconds: BTreeMap::new(),
                failed_segment: None,
                error: None,
                store_open: None,
            });
        }
        collector.record(
            observation,
            Duration::from_micros(100),
            1,
            &Ok((8, 1)),
            &phases,
        );
        let accounting = &collector.report.measured[0].phase_accounting;
        assert_eq!(accounting.service_unattributed_microseconds, Some(70.0));
        assert_eq!(
            accounting.service_unattributed_after_transaction_begin_microseconds,
            Some(30.0)
        );
    }

    #[test]
    fn actual_store_open_stages_telescope_and_failed_guard_has_no_later_stages() {
        let scratch = super::super::Scratch::new(0).unwrap();
        let (created, _) = super::super::ShowStore::create(&scratch.show_path, "observed").unwrap();
        drop(created);
        let collector = Collector::new(0, 2);
        let log = collector.boundary_log();
        observe_boundary(
            Some(&log),
            &boundary_context("valid"),
            ShowId(uuid::Uuid::from_u128(9)),
            |timing| {
                super::super::open_benchmark_store(&scratch.show_path, timing)
                    .map(|_| ())
                    .map_err(super::super::store_error)
            },
        )
        .unwrap();
        let missing = scratch.root.join("missing.sqlite");
        let expected = super::super::ShowStore::open(&missing)
            .err()
            .unwrap()
            .to_string();
        let actual = observe_boundary(
            Some(&log),
            &boundary_context("missing"),
            ShowId(uuid::Uuid::from_u128(9)),
            |timing| {
                super::super::open_benchmark_store(&missing, timing)
                    .map(|_| ())
                    .map_err(super::super::store_error)
            },
        )
        .unwrap_err();
        assert_eq!(actual.message, expected);
        assert!(!missing.exists());
        let rows = &log.lock().rows;
        let success = rows[0].store_open.as_ref().unwrap();
        assert_eq!(success.failed_phase, None);
        assert_eq!(success.segments_microseconds.len(), 9);
        for name in [
            "playback_contract",
            "connection_open",
            "connection_configure",
            "schema_version",
            "migration_branch",
            "integrity",
            "schema_validation",
            "identity",
            "completion",
        ] {
            assert!(success.segments_microseconds.contains_key(name));
        }
        let failure = rows[1].store_open.as_ref().unwrap();
        assert_eq!(failure.failed_phase, Some("playback_contract"));
        assert_eq!(failure.segments_microseconds.len(), 1);
        for value in [success, failure] {
            assert!(
                (value.segments_microseconds.values().sum::<f64>() - value.total_microseconds)
                    .abs()
                    < 0.000001
            );
        }
        let plain = super::super::open_benchmark_store(&scratch.show_path, &mut None).unwrap();
        assert_eq!(plain.name().unwrap(), "observed");
    }

    #[test]
    fn existing_scenario_serialization_omits_disabled_diagnostics() {
        use super::super::{ByteDistribution, PatchInvariants, PatchScenarioReport, distribution};
        let bytes = || ByteDistribution {
            minimum: 1,
            maximum: 1,
            mean: 1.0,
        };
        let report = PatchScenarioReport {
            fixture_count: 100,
            gate_p95_microseconds: 500_000.0,
            total_server: distribution(&[Duration::from_micros(3)]).unwrap(),
            request_bytes: bytes(),
            response_bytes: bytes(),
            phases: BTreeMap::new(),
            invariants: PatchInvariants {
                samples: 40,
                active_show_document_boundaries: 80,
                show_transactions: 40,
                backup_decisions: 40,
                persistence_commits: 40,
                runtime_preparations: 40,
                runtime_installs: 40,
                reconciliations: 40,
                events: 40,
                actual_backup_copies: 40,
                one_atomic_boundary_per_sample: true,
                unrelated_reads: 0,
            },
            raw_total_microseconds: vec![3.0],
            gate_met: true,
            request_diagnostics: None,
        };
        let value = serde_json::to_value(report).unwrap();
        assert!(value.get("request_diagnostics").is_none());
        assert_eq!(value["gate_p95_microseconds"], 500_000.0);
        assert_eq!(value["raw_total_microseconds"], serde_json::json!([3.0]));
        assert_eq!(value["invariants"]["active_show_document_boundaries"], 80);
    }

    #[test]
    fn absent_resource_counter_is_explicitly_unavailable_not_zero() {
        let value = serde_json::to_value(readout("/proc/not-a-real-tl785-counter")).unwrap();
        assert_eq!(value["status"], "unavailable");
        assert!(value.get("value").is_none());
        assert!(
            value["reason"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
    }
}
