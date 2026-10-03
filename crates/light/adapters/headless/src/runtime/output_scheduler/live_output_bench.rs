//! TL-596: narrow released-benchmark seam over the production Live output transaction.
//!
//! `light-benchmark` drives the same `dynamic_output_frame` boundary as `render_tick`: one
//! authoritative capture, reconciliation of Programmer Dynamics into the Dynamics runtime, the
//! all-family hybrid frame (Position, Color/UV and Focus/Zoom adapters behind the production
//! opt-in), and publication of the committed frame into a `VisualizationFrameHub`.
//!
//! It deliberately leaves out what the benchmark runner owns or what has no semantic cost of its
//! own: the ordered Playback unit of work (captured automatic-cue events and persistence
//! checkpoints), timecode and internal audio, Hold and raw overrides, and network or USB I/O.
//! The runner encodes routes itself, exactly as it does for the legacy path. Nothing here changes
//! production behavior: it only composes `pub(in crate::runtime)` pieces the scheduler already
//! uses, with the production opt-in (`live_family_adapters_opted_in`).
use super::*;
use std::sync::atomic::AtomicBool;

/// One Live output lane outside the server: Engine, Dynamics, publication and family lanes.
pub struct LiveOutputBench {
    engine: Arc<Engine>,
    dynamics: Mutex<light_dynamics::DynamicRuntime>,
    publication: DynamicSnapshotPublication,
    origins: super::super::dynamic_source_origins::SharedDynamicSourceOrigins,
    speed_groups: Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: AtomicU16,
    cache: ProgrammerReconciliationCache,
    family: Arc<LiveFamilyAdapters>,
    visualization: Arc<VisualizationFrameHub>,
    publish: bool,
}

/// One committed frame with the cost of each part of the transaction.
pub struct LiveOutputFrame {
    pub rendered: RenderResult,
    /// The all-family hybrid source produced this frame (not the legacy scalar source).
    pub hybrid: bool,
    /// `Engine::try_prepare_output_frame` (the engine's `PreparedCapture` phase).
    pub capture: Duration,
    /// `dynamic_output_frame`: Dynamics reconciliation and sampling, family preparation and
    /// physical solves, the engine's final render and lane acceptance.
    pub transaction: Duration,
    /// Building the published frame and storing it in the visualization hub.
    pub publication: Duration,
    /// Cumulative Live adapter work after this frame.
    pub work: LiveOutputWork,
}

/// Cumulative counters of the Live family lanes. Counters only; no budget is implied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct LiveOutputWork {
    pub position_compiles: u64,
    pub position_fits: u64,
    pub position_fit_cache_hits: u64,
    pub position_candidate_evaluations: u64,
    pub color_descriptor_compiles: u64,
    pub color_fitting_compiles: u64,
    pub color_fitting_cache_hits: u64,
    pub color_fitting_failures: u64,
    pub color_resolves: u64,
    pub color_fits: u64,
    pub color_refits: u64,
    /// Color head resolves replayed from unchanged fit inputs (TL-553).
    pub color_result_reuses: u64,
    pub color_candidates_ranked: u64,
    pub optics_descriptor_compiles: u64,
    pub optics_fitting_compiles: u64,
    pub optics_fitting_cache_hits: u64,
    pub optics_resolves: u64,
    pub optics_fitted: u64,
    pub optics_held: u64,
    /// Changed Points and dirty instances of the census accepted with this exact frame; `None`
    /// when this frame accepted no census (no registered Point dependencies).
    pub tracking_changed_points: Option<u64>,
    pub tracking_dirty_instances: Option<u64>,
}

impl LiveOutputBench {
    /// `engine` must already hold the patch and Dynamics definitions; `dynamics` must have the
    /// same definitions installed. `publish` enables the visualization publication step.
    pub fn new(
        engine: Arc<Engine>,
        dynamics: light_dynamics::DynamicRuntime,
        rate_hz: u16,
        publish: bool,
    ) -> Result<Self, String> {
        let mut speed_groups = Vec::with_capacity(5);
        for _ in 0..5 {
            speed_groups.push(
                light_control::speed::SpeedGroupController::new(120.0, Default::default())
                    .map_err(|error| format!("speed group: {error:?}"))?,
            );
        }
        let speed_groups: [light_control::speed::SpeedGroupController; 5] = speed_groups
            .try_into()
            .map_err(|_| "five speed groups".to_owned())?;
        Ok(Self {
            publication: DynamicSnapshotPublication::new(engine.snapshot()),
            engine,
            dynamics: Mutex::new(dynamics),
            origins: Default::default(),
            speed_groups: Mutex::new(speed_groups),
            rate: AtomicU16::new(rate_hz),
            cache: ProgrammerReconciliationCache::default(),
            family: Arc::new(LiveFamilyAdapters::new(
                super::super::e2e_semantic_contract::live_family_adapters_opted_in(),
            )),
            visualization: Arc::new(VisualizationFrameHub::default()),
            publish,
        })
    }

    /// Whether frames take the all-family hybrid path (production opt-in and engine contract).
    pub fn family_engaged(&self) -> bool {
        self.family.engaged(&self.engine)
    }

    /// Render one frame through the production Live transaction.
    pub fn render(
        &self,
        options: RenderOptions,
        baseline: &[ContributionBatch],
    ) -> Result<LiveOutputFrame, String> {
        let started = Instant::now();
        let prepared = self
            .engine
            .try_prepare_output_frame(options)
            .ok_or_else(|| EngineError::StalePreparedFrame.to_string())?;
        let capture = started.elapsed();
        let transaction_started = Instant::now();
        let mut hybrid = false;
        let committed = dynamic_output_frame(
            &self.engine,
            &prepared,
            None,
            baseline,
            &self.dynamics,
            &self.publication,
            &self.origins,
            &self.speed_groups,
            &self.rate,
            &self.cache,
            &self.family,
            |source| match source {
                dynamic_projection::OutputRenderSource::Legacy(batches) => {
                    self.engine.render_prepared(&prepared, batches)
                }
                dynamic_projection::OutputRenderSource::Hybrid(rendered) => {
                    hybrid = true;
                    Ok(rendered)
                }
            },
        )
        .map_err(|error| error.to_string())?;
        let transaction = transaction_started.elapsed();
        let publish_started = Instant::now();
        let frame = RenderedSemanticFrame {
            rendered: committed.output,
            options,
            dynamics: Some(Arc::new(FrameDynamicSources {
                sample_boundary: committed.sample_boundary,
                runtime: committed.runtime,
                samples: committed.samples,
                origins: committed.origins,
                programmer_values: Arc::clone(prepared.dynamic_programmer_values()),
                cue_values: prepared.cue_dynamic_values().into(),
                ordinary: committed.ordinary,
            })),
        };
        if self.publish {
            self.visualization
                .publish(&frame, VisualizationScope { show_id: None });
        }
        let publication = publish_started.elapsed();
        let work = self.work(frame.rendered.sampled_at);
        Ok(LiveOutputFrame {
            rendered: frame.rendered,
            hybrid,
            capture,
            transaction,
            publication,
            work,
        })
    }

    fn work(&self, sampled_at: chrono::DateTime<chrono::Utc>) -> LiveOutputWork {
        self.family.with_lanes(|lanes| {
            let position = lanes.position().adapter();
            let counters = position.counters();
            let census = position
                .tracking_census()
                .filter(|(census_at, _, _)| *census_at == sampled_at);
            let color = lanes.color().adapter().lamp().counters();
            let optics = lanes.optics().counters();
            LiveOutputWork {
                position_compiles: counters.compiles,
                position_fits: counters.fits,
                position_fit_cache_hits: counters.fit_cache_hits,
                position_candidate_evaluations: counters.candidate_evaluations,
                color_descriptor_compiles: color.descriptor_compiles,
                color_fitting_compiles: color.fitting_compiles,
                color_fitting_cache_hits: color.fitting_cache_hits,
                color_fitting_failures: color.fitting_failures,
                color_resolves: color.resolves,
                color_fits: color.fits,
                color_refits: color.refits,
                color_result_reuses: color.result_reuses,
                color_candidates_ranked: color.candidates_ranked,
                optics_descriptor_compiles: optics.descriptor_compiles,
                optics_fitting_compiles: optics.fitting_compiles,
                optics_fitting_cache_hits: optics.fitting_cache_hits,
                optics_resolves: optics.resolves,
                optics_fitted: optics.fitted,
                optics_held: optics.held,
                tracking_changed_points: census.map(|(_, changed, _)| changed as u64),
                tracking_dirty_instances: census.map(|(_, _, dirty)| dirty as u64),
            }
        })
    }

    /// TL-639: what readers see of `rendered`, per programming target: the accepted Color
    /// heads, outputs and held targets of that exact frame. The benchmark's `--digest-ticks`
    /// mode hashes it to compare two builds frame by frame. Frame tokens are process-local and
    /// left out.
    pub fn readout_digest(&self, rendered: &RenderResult) -> Vec<(FixtureId, String)> {
        let Some(frame) = self
            .family
            .accepted_color(rendered.generation, rendered.sampled_at)
        else {
            return Vec::new();
        };
        let mut rows = frame
            .heads
            .iter()
            .map(|head| (head.target, format!("head {head:?}")))
            .chain(frame.outputs.iter().map(|output| {
                (
                    output.target,
                    format!("output {:?} {:?}", output.value, output.writes),
                )
            }))
            .chain(frame.held.iter().map(|target| (*target, "held".to_owned())))
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| (left.0.0, &left.1).cmp(&(right.0.0, &right.1)));
        rows
    }

    /// Start `count` readout consumers of the published frames on their own threads, the way
    /// Stage and the Color readout read them: the newest frame, its physical sidecars and the
    /// accepted Color results of that exact frame. `hold` keeps each frame for that long before
    /// the next read, which models a slow consumer. Consumers never call into the adapters.
    pub fn spawn_readout_consumers(&self, count: usize, hold: Duration) -> ReadoutConsumers {
        let stop = Arc::new(AtomicBool::new(false));
        let threads = (0..count)
            .map(|_| {
                let stop = Arc::clone(&stop);
                let hub = Arc::clone(&self.visualization);
                let family = Arc::clone(&self.family);
                std::thread::spawn(move || read_published_frames(&stop, &hub, &family, hold))
            })
            .collect();
        ReadoutConsumers { stop, threads }
    }
}

/// Running readout consumers; `finish` stops them and returns what they read.
pub struct ReadoutConsumers {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<ReadoutConsumerReport>>,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct ReadoutConsumerReport {
    pub consumers: u64,
    pub frames_read: u64,
    pub accepted_color_frames_found: u64,
    pub physical_instances_read: u64,
}

impl ReadoutConsumers {
    pub fn finish(self) -> ReadoutConsumerReport {
        self.stop.store(true, Ordering::Relaxed);
        let mut total = ReadoutConsumerReport::default();
        for thread in self.threads {
            let Ok(report) = thread.join() else { continue };
            total.consumers += 1;
            total.frames_read += report.frames_read;
            total.accepted_color_frames_found += report.accepted_color_frames_found;
            total.physical_instances_read += report.physical_instances_read;
        }
        total
    }
}

fn read_published_frames(
    stop: &AtomicBool,
    hub: &VisualizationFrameHub,
    family: &LiveFamilyAdapters,
    hold: Duration,
) -> ReadoutConsumerReport {
    let mut report = ReadoutConsumerReport::default();
    let mut last = 0;
    while !stop.load(Ordering::Relaxed) {
        let Some(frame) = hub.latest().filter(|frame| frame.sequence != last) else {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        };
        last = frame.sequence;
        report.frames_read += 1;
        report.physical_instances_read += frame.physical.instances.len() as u64;
        if family
            .accepted_color(frame.generation, frame.sampled_at)
            .is_some_and(|accepted| std::hint::black_box(accepted.outputs.len()) < usize::MAX)
        {
            report.accepted_color_frames_found += 1;
        }
        std::hint::black_box(&frame.values);
        if !hold.is_zero() {
            std::thread::sleep(hold);
        }
    }
    report
}

#[cfg(test)]
mod tests;
