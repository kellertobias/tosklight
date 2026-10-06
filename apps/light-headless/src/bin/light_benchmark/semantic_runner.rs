//! TL-596: the per-frame record of a semantic scenario rendered through the production Live
//! transaction (`LiveOutputBench`), and the tracking feed injected at the engine boundary.
//!
//! Timing is split the way the transaction is: capture (`PreparedCapture`), the Dynamics and
//! all-family transaction (sampling, physical solves, final render, lane acceptance) and
//! publication. Encoding is recorded by the ordinary runner. Work counters are per-frame deltas
//! of the Live lanes' cumulative counters. A value the run could not observe is `null`, never 0.
use crate::light_benchmark::statistics::{Distribution, distribution};
use light_engine::{Engine, TrackedInputFrame, TrackedPointInput};
use light_headless_runtime::output_benchmark::{
    LiveOutputBench, LiveOutputFrame, LiveOutputWork, ReadoutConsumerReport,
};
use serde::Serialize;
use std::{cell::Cell, sync::Arc, time::Duration};

/// A semantic scenario: the Live transaction plus its optional tracking feed.
pub struct LiveScenario {
    pub bench: LiveOutputBench,
    pub tracking: Option<TrackingFeed>,
    pub description: LiveWorkloadDescription,
    /// TL-641: the Dynamic `--start-latency` starts itself; `None` when the build started them.
    pub pending_start: Option<PendingStart>,
}

/// One Start Now gesture the probe performs on a running Live lane.
pub struct PendingStart {
    pub session: light_core::SessionId,
    pub definition: light_dynamics::DynamicDefinition,
    pub targets: Vec<light_core::FixtureId>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LiveWorkloadDescription {
    pub kind: &'static str,
    pub typed_lane_attributes: Vec<String>,
    pub semantic_base_targets: usize,
    pub started_dynamics: usize,
    pub animated_targets: usize,
    /// TL-564 inputs when the workload came from a manifest; absent for the capacity variants.
    pub manifest_sha256: Option<String>,
    pub workload_id: Option<String>,
    /// The exact dependent set the manifest predicts for the tracking scenario.
    pub expected_dirty_targets: Option<usize>,
    pub expected_moving_points: Option<usize>,
    /// Harness-assigned fixture grid height of a TL-564 workload (the patch carries none).
    pub harness_rig_height_mm: Option<i32>,
    pub omitted_from_live_transaction: &'static [&'static str],
}

/// Deterministic receiver samples, installed with `Engine::set_tracking_frame` exactly as the
/// PSN receiver publishes them. Injection happens outside the timed pipeline at each tick.
pub struct TrackingFeed {
    pub rate_hz: u16,
    pub scenario: String,
    pub stream_sha256: Option<String>,
    frames: Vec<(u64, Arc<[TrackedPointInput]>)>,
    period_micros: u64,
    installed: Cell<Option<u64>>,
    sequence: Cell<u64>,
}

impl TrackingFeed {
    pub fn new(
        rate_hz: u16,
        scenario: String,
        stream_sha256: Option<String>,
        frames: Vec<(u64, Arc<[TrackedPointInput]>)>,
    ) -> Result<Self, String> {
        let last = frames.last().ok_or("tracking stream is empty")?.0;
        Ok(Self {
            rate_hz,
            scenario,
            stream_sha256,
            period_micros: last + 1_000_000 / u64::from(rate_hz.max(1)),
            frames,
            installed: Cell::new(None),
            sequence: Cell::new(0),
        })
    }

    /// Install the newest sample due at `elapsed_micros` (the stream loops). Returns the age of
    /// the installed sample at that instant.
    pub fn inject(&self, engine: &Engine, elapsed_micros: u64) -> Duration {
        let looped = elapsed_micros / self.period_micros;
        let within = elapsed_micros % self.period_micros;
        let index = self
            .frames
            .partition_point(|(t_micros, _)| *t_micros <= within)
            .saturating_sub(1);
        let absolute = looped * self.frames.len() as u64 + index as u64;
        if self.installed.get() != Some(absolute) {
            self.installed.set(Some(absolute));
            let sequence = self.sequence.get() + 1;
            self.sequence.set(sequence);
            engine.set_tracking_frame(Arc::new(TrackedInputFrame {
                accepted_sequence: sequence,
                sampled_at_millis: elapsed_micros / 1_000,
                points: Arc::clone(&self.frames[index].1),
                ..Default::default()
            }));
        }
        Duration::from_micros(within.saturating_sub(self.frames[index].0))
    }

    pub fn installed_samples(&self) -> u64 {
        self.sequence.get()
    }
}

/// Measured-window record of a semantic scenario.
#[derive(Default)]
pub struct LiveRecorder {
    capture: Vec<Duration>,
    transaction: Vec<Duration>,
    publication: Vec<Duration>,
    tracking_to_output: Vec<Duration>,
    position_fits: Vec<u64>,
    position_cache_hits: Vec<u64>,
    candidate_evaluations: Vec<u64>,
    color_resolves: Vec<u64>,
    color_fits: Vec<u64>,
    color_result_reuses: Vec<u64>,
    optics_resolves: Vec<u64>,
    dirty_instances: Vec<u64>,
    changed_points: Vec<u64>,
    census_frames: u64,
    frames: u64,
    hybrid_frames: u64,
    generation_changes: u64,
    first: Option<LiveOutputWork>,
    last: Option<LiveOutputWork>,
    generation: Option<u64>,
}

impl LiveRecorder {
    /// `previous` is the cumulative work before this frame (the last warmup or measured frame).
    pub fn record(
        &mut self,
        frame: &LiveOutputFrame,
        previous: LiveOutputWork,
        age: Option<Duration>,
        pipeline: Duration,
    ) {
        let work = frame.work;
        self.frames += 1;
        self.hybrid_frames += u64::from(frame.hybrid);
        self.capture.push(frame.capture);
        self.transaction.push(frame.transaction);
        self.publication.push(frame.publication);
        if let Some(age) = age {
            self.tracking_to_output.push(age + pipeline);
        }
        self.position_fits
            .push(work.position_fits - previous.position_fits);
        self.position_cache_hits
            .push(work.position_fit_cache_hits - previous.position_fit_cache_hits);
        self.candidate_evaluations
            .push(work.position_candidate_evaluations - previous.position_candidate_evaluations);
        self.color_resolves
            .push(work.color_resolves - previous.color_resolves);
        self.color_fits.push(
            (work.color_fits + work.color_refits) - (previous.color_fits + previous.color_refits),
        );
        self.color_result_reuses
            .push(work.color_result_reuses - previous.color_result_reuses);
        self.optics_resolves
            .push(work.optics_resolves - previous.optics_resolves);
        if let (Some(changed), Some(dirty)) =
            (work.tracking_changed_points, work.tracking_dirty_instances)
        {
            self.census_frames += 1;
            self.changed_points.push(changed);
            self.dirty_instances.push(dirty);
        }
        if self
            .generation
            .is_some_and(|generation| generation != frame.rendered.generation)
        {
            self.generation_changes += 1;
        }
        self.generation = Some(frame.rendered.generation);
        self.first.get_or_insert(previous);
        self.last = Some(work);
    }

    pub fn report(
        self,
        live: &LiveScenario,
        consumers: Option<ReadoutConsumerReport>,
        slow_consumer_millis: u64,
    ) -> SemanticScenarioReport {
        let compiles = |work: &LiveOutputWork| {
            work.position_compiles
                + work.color_descriptor_compiles
                + work.color_fitting_compiles
                + work.optics_descriptor_compiles
                + work.optics_fitting_compiles
        };
        let measured_compiles = match (self.first, self.last) {
            (Some(first), Some(last)) => Some(compiles(&last) - compiles(&first)),
            _ => None,
        };
        SemanticScenarioReport {
            workload: live.description.clone(),
            family_engaged: live.bench.family_engaged(),
            frames: self.frames,
            hybrid_frames: self.hybrid_frames,
            phases: SemanticPhases {
                prepared_capture: distribution(&self.capture),
                dynamics_and_family_transaction: distribution(&self.transaction),
                publication: distribution(&self.publication),
            },
            tracking: live.tracking.as_ref().map(|feed| TrackingReport {
                rate_hz: feed.rate_hz,
                scenario: feed.scenario.clone(),
                stream_sha256: feed.stream_sha256.clone(),
                installed_samples: feed.installed_samples(),
                tracking_to_output: distribution(&self.tracking_to_output),
                definition: "age of the newest injected receiver sample at frame capture plus the frame's pipeline time; injection is at the engine boundary, not a PSN socket",
            }),
            work: SemanticWork {
                position_fits: counts(&self.position_fits),
                position_fit_cache_hits: counts(&self.position_cache_hits),
                position_candidate_evaluations: counts(&self.candidate_evaluations),
                color_resolves: counts(&self.color_resolves),
                color_fits_and_refits: counts(&self.color_fits),
                color_result_reuses: counts(&self.color_result_reuses),
                optics_resolves: counts(&self.optics_resolves),
                census_frames: self.census_frames,
                tracking_changed_points: counts(&self.changed_points),
                tracking_dirty_instances: counts(&self.dirty_instances),
                descriptor_and_fitter_compiles_in_window: measured_compiles,
                generation_changes_in_window: self.generation_changes,
                cumulative_at_end: self.last,
            },
            readout_consumers: consumers.map(|report| ConsumerSummary {
                report,
                slow_consumer_millis,
            }),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CountDistribution {
    pub frames: usize,
    pub total: u64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub maximum: u64,
}

fn counts(values: &[u64]) -> Option<CountDistribution> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = |percentile: usize| {
        sorted[(percentile * sorted.len())
            .div_ceil(100)
            .saturating_sub(1)
            .min(sorted.len() - 1)]
    };
    Some(CountDistribution {
        frames: sorted.len(),
        total: sorted.iter().sum(),
        p50: rank(50),
        p95: rank(95),
        p99: rank(99),
        maximum: *sorted.last().expect("non-empty"),
    })
}

#[derive(Debug, Serialize)]
pub struct SemanticScenarioReport {
    pub workload: LiveWorkloadDescription,
    pub family_engaged: bool,
    pub frames: u64,
    pub hybrid_frames: u64,
    pub phases: SemanticPhases,
    pub tracking: Option<TrackingReport>,
    pub work: SemanticWork,
    pub readout_consumers: Option<ConsumerSummary>,
}

#[derive(Debug, Serialize)]
pub struct SemanticPhases {
    pub prepared_capture: Option<Distribution>,
    pub dynamics_and_family_transaction: Option<Distribution>,
    pub publication: Option<Distribution>,
}

#[derive(Debug, Serialize)]
pub struct TrackingReport {
    pub rate_hz: u16,
    pub scenario: String,
    pub stream_sha256: Option<String>,
    pub installed_samples: u64,
    pub tracking_to_output: Option<Distribution>,
    pub definition: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SemanticWork {
    pub position_fits: Option<CountDistribution>,
    pub position_fit_cache_hits: Option<CountDistribution>,
    pub position_candidate_evaluations: Option<CountDistribution>,
    pub color_resolves: Option<CountDistribution>,
    pub color_fits_and_refits: Option<CountDistribution>,
    /// Color head resolves replayed from unchanged fit inputs (TL-553).
    pub color_result_reuses: Option<CountDistribution>,
    pub optics_resolves: Option<CountDistribution>,
    pub census_frames: u64,
    pub tracking_changed_points: Option<CountDistribution>,
    pub tracking_dirty_instances: Option<CountDistribution>,
    pub descriptor_and_fitter_compiles_in_window: Option<u64>,
    pub generation_changes_in_window: u64,
    pub cumulative_at_end: Option<LiveOutputWork>,
}

#[derive(Debug, Serialize)]
pub struct ConsumerSummary {
    pub report: ReadoutConsumerReport,
    pub slow_consumer_millis: u64,
}
