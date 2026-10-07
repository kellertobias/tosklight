use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicRuntimeSourceCheckpoint, SharedDynamicSourceOrigins,
};
use crate::runtime::visualization_frame::{FrameDynamicSources, RenderedSemanticFrame};
use light_core::programming::IntentError;

mod activation;
mod family_adapters;
mod finalization;
/// TL-548 C4 hook: Live handles for the Pending episode worker (own file).
mod pending_episode_sources;
mod preload_commit;
mod restored_owners;
mod runtime;
mod transition;

pub(in crate::runtime) use transition::{OutputTransitionLease, OutputTransitionOverlay};

#[cfg(test)]
mod transition_tests;

pub(in crate::runtime) use activation::PreparedOutputActivation;

#[cfg(test)]
mod finalization_tests;
#[cfg(test)]
mod publication_tests;
#[cfg(test)]
mod restore_tests;

/// Cold validation for both registries. Ordinary edits capture no advancing Dynamic state:
/// they preserve current controllers/clocks at publication. Explicit show activation may attach
/// a separately validated incoming checkpoint, intentionally replacing that state.
#[must_use]
pub(crate) struct PreparedOutputSnapshot {
    engine: PreparedEngineSnapshot,
    definitions: light_dynamics::PreparedDynamicDefinitions,
    restored: Option<output_scheduler::RestoredDynamicCandidate>,
}

impl PreparedOutputSnapshot {
    pub(in crate::runtime) fn snapshot(&self) -> &EngineSnapshot {
        self.engine.snapshot()
    }
}

impl std::fmt::Debug for PreparedOutputSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedOutputSnapshot")
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub(in crate::runtime) struct OutputResource {
    runtime_service: OutputRuntimeService,
    speed_group_service: SpeedGroupService,
    engine: Arc<Engine>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    rate: Arc<AtomicU16>,
    control: OutputControlCapability,
    timecode: Arc<Mutex<TimecodeRouter>>,
    network: Option<Arc<NetworkOutput>>,
    usb: Arc<light_output::UsbOutputFanout>,
    usb_configuration_lock: Arc<tokio::sync::Mutex<()>>,
    sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    manual_clock: Option<Arc<ManualClock>>,
    test_clock_lock: Arc<tokio::sync::Mutex<()>>,
    speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    dynamic_snapshot: Arc<DynamicSnapshotPublication>,
    dynamic_source_origins: SharedDynamicSourceOrigins,
    programmer_reconciliation_cache: Arc<output_scheduler::ProgrammerReconciliationCache>,
    family_adapters: Arc<output_scheduler::LiveFamilyAdapters>,
    visualization_ordinary: Arc<Mutex<Option<CachedVisualizationOrdinary>>>,
    dynamic_auto_offs: Arc<Mutex<Vec<light_playback::PlaybackIdentity>>>,
    visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    sound_capture_active: Arc<Mutex<[bool; 5]>>,
    /// Injected accepted Pending Position source; empty until TL-548 installs one.
    pending_position_readouts: crate::runtime::position_readout::PendingPositionReadoutSlot,
    #[cfg(test)]
    runtime_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    runtime_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    speed_group_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    speed_group_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
}

pub(in crate::runtime) struct OutputSemanticRenderTiming {
    pub(in crate::runtime) dynamic: Duration,
    pub(in crate::runtime) engine: Duration,
}

struct CachedVisualizationOrdinary {
    snapshot: Arc<EngineSnapshot>,
    captured_at: std::time::Instant,
    values: Arc<light_engine::ResolvedValues>,
}

#[derive(Clone)]
pub(in crate::runtime) struct OutputControlCapability {
    control: Arc<Mutex<OutputControl>>,
}

impl OutputControlCapability {
    pub(in crate::runtime) fn new(control: Arc<Mutex<OutputControl>>) -> Self {
        Self { control }
    }

    fn lock(&self) -> parking_lot::MutexGuard<'_, OutputControl> {
        self.control.lock()
    }
}

#[derive(Clone, Copy)]
pub(in crate::runtime) struct OutputRuntimeControlProjection {
    pub(in crate::runtime) revision: u64,
    pub(in crate::runtime) grand_master: f32,
    pub(in crate::runtime) blackout: bool,
    pub(in crate::runtime) grand_master_flash: bool,
}

pub(in crate::runtime) struct TestClockSession {
    _guard: tokio::sync::OwnedMutexGuard<()>,
    driver: TestClockDriver,
}

#[derive(Clone)]
pub(in crate::runtime) struct TestClockDriver {
    clock: Arc<ManualClock>,
}

impl TestClockSession {
    pub(in crate::runtime) fn driver(&self) -> TestClockDriver {
        self.driver.clone()
    }

    pub(in crate::runtime) fn set(&self, time: chrono::DateTime<chrono::Utc>) {
        self.driver.set(time);
    }

    pub(in crate::runtime) fn advance_millis(&self, millis: i64) -> chrono::DateTime<chrono::Utc> {
        self.driver.advance_millis(millis)
    }

    pub(in crate::runtime) fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.driver.now()
    }
}

impl TestClockDriver {
    pub(in crate::runtime) fn set(&self, time: chrono::DateTime<chrono::Utc>) {
        self.clock.set(time);
    }

    pub(in crate::runtime) fn advance_millis(&self, millis: i64) -> chrono::DateTime<chrono::Utc> {
        self.clock.advance_millis(millis)
    }

    pub(in crate::runtime) fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.clock.now()
    }
}

impl OutputResource {
    pub(in crate::runtime) fn supported_programming_contract(&self) -> u16 {
        self.engine().supported_programming_contract()
    }
    pub(in crate::runtime) fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(in crate::runtime) fn group_color(&self, group_id: &str) -> Option<light_core::Xyz> {
        self.engine.group_color(group_id)
    }

    pub(in crate::runtime) fn set_group_color(
        &self,
        group_id: &str,
        color: Option<light_core::Xyz>,
    ) -> Result<bool, light_engine::EngineError> {
        self.engine.set_group_color(group_id, color)
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::runtime) fn new(
        runtime_service: OutputRuntimeService,
        speed_group_service: SpeedGroupService,
        engine: Arc<Engine>,
        health: Arc<std::sync::Mutex<OutputHealth>>,
        rate: Arc<AtomicU16>,
        control: OutputControlCapability,
        timecode: Arc<Mutex<TimecodeRouter>>,
        network: Option<Arc<NetworkOutput>>,
        usb: Arc<light_output::UsbOutputFanout>,
        sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
        manual_clock: Option<Arc<ManualClock>>,
        speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
        dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
        dynamic_snapshot: Arc<DynamicSnapshotPublication>,
        dynamic_source_origins: SharedDynamicSourceOrigins,
        dynamic_auto_offs: Arc<Mutex<Vec<light_playback::PlaybackIdentity>>>,
        visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    ) -> Self {
        Self {
            runtime_service,
            speed_group_service,
            engine,
            health,
            rate,
            control,
            timecode,
            network,
            usb,
            usb_configuration_lock: Arc::default(),
            sequences,
            manual_clock,
            test_clock_lock: Arc::default(),
            speed_groups,
            dynamics,
            dynamic_snapshot,
            dynamic_source_origins,
            programmer_reconciliation_cache: Arc::new(
                output_scheduler::ProgrammerReconciliationCache::default(),
            ),
            family_adapters: Arc::default(),
            visualization_ordinary: Arc::new(Mutex::new(None)),
            dynamic_auto_offs,
            visualization_frames,
            sound_capture_active: Arc::new(Mutex::new([false; 5])),
            pending_position_readouts: Default::default(),
            #[cfg(test)]
            runtime_persistence_attempts: Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            runtime_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            speed_group_persistence_attempts: Arc::new(AtomicU64::new(0)),
            #[cfg(test)]
            speed_group_persistence_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(in crate::runtime) fn health_snapshot(&self) -> OutputHealth {
        let mut health = self.health.lock().expect("output health mutex poisoned");
        // An output that stalled must report the stall, not the last healthy window it filled.
        health.refresh_recent(std::time::Instant::now());
        health.clone()
    }

    pub(in crate::runtime) fn latest_visualization_frame(
        &self,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.latest()
    }

    /// Pending (Preload) Position readouts come only from this slot, never from Live frames.
    pub(in crate::runtime) fn pending_position_readouts(
        &self,
    ) -> &crate::runtime::position_readout::PendingPositionReadoutSlot {
        &self.pending_position_readouts
    }

    pub(in crate::runtime) fn sampled_visualization_frame(
        &self,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.sampled()
    }

    pub(in crate::runtime) async fn wait_for_visualization_sample_after(
        &self,
        sequence: u64,
    ) -> Arc<super::visualization_frame::PublishedVisualizationFrame> {
        self.visualization_frames
            .wait_for_sample_after(sequence)
            .await
    }

    pub(in crate::runtime) fn visualization_frame_hub(
        &self,
    ) -> Arc<super::visualization_frame::VisualizationFrameHub> {
        Arc::clone(&self.visualization_frames)
    }

    pub(in crate::runtime) fn visualization_projection(
        &self,
        key: super::visualization_frame::VisualizationProjectionKey,
        source: &super::visualization_frame::PublishedVisualizationFrame,
        build: impl FnOnce(
            bool,
        )
            -> Result<light_wire::v2::visualization::VisualizationLaneSnapshot, ApiError>,
    ) -> Result<Arc<super::visualization_frame::ProjectedVisualizationFrame>, ApiError> {
        self.visualization_frames.projection(key, source, build)
    }

    pub(in crate::runtime) fn change_visualization_subscribers(
        &self,
        lane: light_wire::v2::visualization::VisualizationLane,
        delta: i8,
    ) {
        self.visualization_frames.change_subscribers(lane, delta);
    }

    pub(in crate::runtime) fn change_visualization_projection_claim(
        &self,
        key: super::visualization_frame::VisualizationProjectionKey,
        delta: i8,
    ) {
        self.visualization_frames
            .change_projection_claim(key, delta);
    }

    pub(in crate::runtime) fn visualization_metrics(
        &self,
    ) -> super::visualization_frame::VisualizationMetrics {
        self.visualization_frames.metrics()
    }

    pub(in crate::runtime) fn record_visualization_snapshot_route(
        &self,
        projection_duration: Duration,
        serialization_duration: Duration,
        payload_bytes: u64,
        source: Option<&super::visualization_frame::PublishedVisualizationFrame>,
    ) {
        self.visualization_frames.record_snapshot_route(
            projection_duration,
            serialization_duration,
            payload_bytes,
            source,
        );
    }

    pub(in crate::runtime) fn record_visualization_stream_serialization(
        &self,
        duration: Duration,
        payload_bytes: u64,
    ) {
        self.visualization_frames
            .record_stream_serialization(duration, payload_bytes);
    }

    pub(in crate::runtime) fn record_visualization_stream_queue_push(
        &self,
        replaced_pending: bool,
    ) {
        self.visualization_frames
            .record_stream_queue_push(replaced_pending);
    }

    pub(in crate::runtime) fn record_visualization_stream_queue_take(&self) {
        self.visualization_frames.record_stream_queue_take();
    }

    pub(in crate::runtime) fn record_visualization_stream_send(
        &self,
        duration: Duration,
        succeeded: bool,
    ) {
        self.visualization_frames
            .record_stream_send(duration, succeeded);
    }

    pub(in crate::runtime) fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.engine.snapshot()
    }

    pub(in crate::runtime) fn start_dynamic(
        &self,
        request: light_dynamics::DynamicStartRequest,
    ) -> Result<Uuid, light_dynamics::DynamicRuntimeError> {
        self.dynamics
            .lock()
            .apply_recorded_control(light_dynamics::TimedDynamicControl {
                at_millis: request.now_millis,
                control: light_dynamics::DynamicControl::Start(Box::new(request)),
            })
            .map(|outcome| outcome.instance_id.expect("Start selects an instance"))
    }

    pub(in crate::runtime) fn dynamic_runtime_snapshot(
        &self,
    ) -> light_dynamics::DynamicRuntimeSnapshot {
        self.dynamics.lock().snapshot()
    }

    /// Capture the complete persisted runtime and its provenance under one runtime lock.
    /// The output lane publishes catalogue changes before releasing that same lock.
    pub(in crate::runtime) fn dynamic_source_checkpoint(
        &self,
    ) -> Result<DynamicRuntimeSourceCheckpoint, IntentError> {
        let dynamics = self.dynamics.lock();
        let runtime = dynamics.snapshot();
        let published = self.dynamic_source_origins.load_full();
        let mut origins = (*published).clone();
        // The full persistence snapshot is already available. Retire only unreachable
        // historical records here, never during a frame; active bindings remain until the
        // source assignment lifecycle explicitly unbinds them.
        origins.prune_runtime(&runtime)?;
        if !origins.shares_storage(&published) {
            self.dynamic_source_origins.store(Arc::new(origins.clone()));
        }
        Ok(DynamicRuntimeSourceCheckpoint {
            runtime,
            origins: Some(origins.snapshot()),
        })
    }

    /// Validate both sides before changing either. A failed runtime restore leaves the
    /// published catalogue untouched; successful publication happens under the runtime lock.
    pub(in crate::runtime) fn restore_dynamic_source_checkpoint(
        &self,
        checkpoint: DynamicRuntimeSourceCheckpoint,
    ) -> Result<(), IntentError> {
        Self::restore_dynamic_source_state(
            &self.engine,
            &self.dynamics,
            &self.dynamic_source_origins,
            &self.dynamic_snapshot,
            checkpoint,
        )
    }

    /// Shared cold restore seam for startup and show/runtime restore. Validate with the
    /// current definitions and native models before reserving IDs. Playback and Dynamics
    /// guards are never held together. The final restore rechecks any intervening definition
    /// change; a rejected retry can only leave a conservative, monotonically higher reserve.
    /// Production callers run before workers start or hold the exclusive show activation
    /// permit through generation installation and this restore. That boundary also prevents a
    /// generation swap from racing the Playback reservation; this helper does not acquire it.
    pub(in crate::runtime) fn restore_dynamic_source_state(
        engine: &Engine,
        dynamics: &Mutex<light_dynamics::DynamicRuntime>,
        published: &SharedDynamicSourceOrigins,
        publication: &DynamicSnapshotPublication,
        checkpoint: DynamicRuntimeSourceCheckpoint,
    ) -> Result<(), IntentError> {
        // Restoring historical native definitions can pin newly verified originals before a
        // later payload fails validation. Prepare destination Preset tables on that same
        // detached candidate; no fallible dependency work may follow publication.
        let snapshot = engine.snapshot();
        let prepared = output_scheduler::prepare_restored_dynamic_candidate(
            &snapshot,
            &dynamics.lock(),
            checkpoint.clone(),
        )
        .map_err(|error| IntentError(error.to_string()))?;
        engine.reserve_playback_source_occurrence_watermark(
            prepared.playback_source_occurrence_watermark,
        );
        drop(prepared);
        // Playback reservation must stay outside the Dynamics guard. Rebuild against current
        // runtime state after reacquiring it; never overwrite intervening controls with the
        // preflight fork. The caller's activation permit keeps the destination stable.
        let snapshot = engine.snapshot();
        let mut dynamics = dynamics.lock();
        let prepared =
            output_scheduler::prepare_restored_dynamic_candidate(&snapshot, &dynamics, checkpoint)
                .map_err(|error| IntentError(error.to_string()))?;
        *dynamics = prepared.runtime;
        published.store(Arc::new(prepared.origins));
        publication.installed(snapshot);
        Ok(())
    }

    pub(in crate::runtime) fn dynamic_controller_instance(
        &self,
        controller_id: Uuid,
    ) -> Option<Uuid> {
        self.dynamics
            .lock()
            .controller(controller_id)
            .map(|(instance_id, _)| instance_id)
    }

    pub(in crate::runtime) fn off_dynamic_controller(
        &self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), light_dynamics::DynamicRuntimeError> {
        self.dynamics
            .lock()
            .apply_recorded_control(light_dynamics::TimedDynamicControl {
                at_millis: now_millis,
                control: light_dynamics::DynamicControl::Off {
                    controller: controller_id,
                    delay: release_delay_millis,
                    duration: release_duration_millis,
                },
            })
            .map(|outcome| {
                (
                    outcome.instance_id.expect("Off resolves its instance"),
                    outcome.instance_removed,
                )
            })
    }

    pub(in crate::runtime) fn update_dynamic_controller(
        &self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), light_dynamics::DynamicRuntimeError> {
        let now_millis =
            u64::try_from(self.engine.application_time().timestamp_millis()).unwrap_or_default();
        self.dynamics
            .lock()
            .apply_recorded_control(light_dynamics::TimedDynamicControl {
                at_millis: now_millis,
                control: light_dynamics::DynamicControl::Update {
                    controller: controller_id,
                    size,
                    speed: speed_multiplier,
                    phase: phase_offset_degrees,
                },
            })
            .map(|_| ())
    }

    pub(in crate::runtime) fn is_dynamic_definition_running(&self, definition_id: Uuid) -> bool {
        self.dynamics.lock().is_definition_running(definition_id)
    }

    pub(in crate::runtime) fn set_dynamic_definitions_pinned(&self, pinned: bool) {
        self.dynamics.lock().set_definitions_pinned(pinned);
    }

    #[cfg(test)]
    pub(in crate::runtime) fn replace_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        let prepared = self.prepare_snapshot(snapshot)?;
        self.install_prepared_snapshot(prepared);
        Ok(())
    }

    pub(in crate::runtime) fn prepare_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<PreparedOutputSnapshot, EngineError> {
        let engine = self.engine.prepare_snapshot(snapshot)?;
        let definitions = self
            .dynamics
            .lock()
            .prepare_definitions(engine.snapshot().dynamics.iter().cloned())
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        Ok(PreparedOutputSnapshot {
            engine,
            definitions,
            restored: None,
        })
    }

    /// Prepare the incoming show's checkpoint before transition, selection or active-show
    /// mutations. The token carries the exact validated destination runtime to publication;
    /// there is no fallible restore after the Engine switches to that show.
    pub(in crate::runtime) fn prepare_snapshot_restore(
        &self,
        mut prepared: PreparedOutputSnapshot,
        checkpoint: DynamicRuntimeSourceCheckpoint,
    ) -> Result<PreparedOutputSnapshot, IntentError> {
        prepared.restored = Some(
            output_scheduler::prepare_restored_dynamic_candidate(
                prepared.snapshot(),
                &self.dynamics.lock(),
                checkpoint,
            )
            .map_err(|error| IntentError(error.to_string()))?,
        );
        Ok(prepared)
    }

    pub(in crate::runtime) fn install_prepared_snapshot(&self, prepared: PreparedOutputSnapshot) {
        let _publication = self.dynamic_snapshot.begin_install();
        let snapshot = prepared.engine.snapshot_arc();
        self.engine.install_prepared_snapshot(prepared.engine);
        let mut dynamics = self.dynamics.lock();
        dynamics.install_prepared_definitions(prepared.definitions);
        self.dynamic_snapshot.installed(snapshot);
    }

    pub(in crate::runtime) fn resolved_values(&self) -> light_engine::ResolvedValues {
        self.engine.resolved_values()
    }

    pub(in crate::runtime) fn cached_visualization_ordinary_values(
        &self,
    ) -> Arc<light_engine::ResolvedValues> {
        let snapshot = self.engine.snapshot();
        let mut cached = self.visualization_ordinary.lock();
        if let Some(cached) = cached.as_ref()
            && Arc::ptr_eq(&cached.snapshot, &snapshot)
            && cached.captured_at.elapsed() < Duration::from_millis(250)
        {
            return Arc::clone(&cached.values);
        }
        let values = Arc::new(self.engine.held_parameter_values());
        *cached = Some(CachedVisualizationOrdinary {
            snapshot,
            captured_at: std::time::Instant::now(),
            values: Arc::clone(&values),
        });
        values
    }

    #[cfg(test)]
    pub(in crate::runtime) fn visualization_dynamic_values(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        projected: bool,
    ) -> light_engine::ResolvedValues {
        self.visualization_dynamic_projection(extra_programmer_values, projected)
            .0
    }

    pub(in crate::runtime) fn visualization_dynamic_projection(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        _projected: bool,
    ) -> (
        light_engine::ResolvedValues,
        light_dynamics::DynamicRuntimeSnapshot,
        Vec<light_dynamics::DynamicRuntimeSample>,
    ) {
        // Visualization is observational. Sampling can retire completed release
        // transitions, so always operate on a clone and leave the authoritative
        // output scheduler responsible for mutating and publishing runtime state.
        let snapshot = self.engine.snapshot();
        let mut visualization_runtime =
            light_dynamics::DynamicRuntime::with_programming_contract_support(
                self.engine.supported_programming_contract(),
            );
        visualization_runtime
            .install_definitions(snapshot.dynamics.iter().cloned())
            .expect("Engine snapshot contains validated Dynamic definitions");
        visualization_runtime
            .restore_snapshot(self.dynamics.lock().snapshot())
            .expect("live Dynamic runtime snapshot remains restorable");
        let visualization_runtime = Mutex::new(visualization_runtime);
        let (sampled, runtime_samples) = output_scheduler::dynamic_projection(
            &self.engine,
            &visualization_runtime,
            &self.speed_groups,
            &self.rate,
            extra_programmer_values,
        );
        let runtime_snapshot = visualization_runtime.lock().output_projection_snapshot();
        (
            self.engine
                .resolved_values_with_contribution_batches(&sampled),
            runtime_snapshot,
            runtime_samples,
        )
    }

    pub(in crate::runtime) fn dynamic_programmer_values(
        &self,
    ) -> Arc<Vec<(Uuid, i16, light_dynamics::DynamicAddressValue)>> {
        self.engine.dynamic_programmer_values()
    }

    pub(in crate::runtime) fn active_cue_dynamic_values(
        &self,
    ) -> Vec<light_playback::ActiveCueDynamicValue> {
        self.engine.active_cue_dynamic_values()
    }

    /// Reconciles typed Programmer, Cue, and Playback Dynamic state into the persisted runtime
    /// without sending an output frame. Preload GO uses this inside the active-show exclusion
    /// boundary so the committed Programmer layer and its runtime identity share one timestamp.
    pub(in crate::runtime) fn reconcile_dynamic_runtime(&self) {
        output_scheduler::reconcile_dynamic_controllers(
            &self.engine,
            &self.dynamics,
            &self.dynamic_snapshot,
        );
    }

    pub(in crate::runtime) fn take_dynamic_auto_offs(
        &self,
    ) -> Vec<light_playback::PlaybackIdentity> {
        std::mem::take(&mut *self.dynamic_auto_offs.lock())
    }

    pub(in crate::runtime) fn restore_dynamic_auto_offs(
        &self,
        identities: impl IntoIterator<Item = light_playback::PlaybackIdentity>,
    ) {
        let mut pending = self.dynamic_auto_offs.lock();
        for identity in identities {
            if !pending.contains(&identity) {
                pending.push(identity);
            }
        }
    }

    pub(in crate::runtime) fn playback_runtime(&self) -> Vec<light_playback::ActivePlayback> {
        self.engine.playback_runtime()
    }

    pub(in crate::runtime) fn playback_runtime_status(
        &self,
    ) -> Vec<light_playback::PlaybackRuntimeStatus> {
        self.engine.playback_runtime_status()
    }

    pub(in crate::runtime) fn playback_runtime_status_at(
        &self,
        identity: light_playback::PlaybackIdentity,
    ) -> Option<light_playback::PlaybackRuntimeStatus> {
        self.engine.playback_runtime_status_at(identity)
    }

    pub(in crate::runtime) fn playback_runtime_status_for_cue_list(
        &self,
        cue_list_id: light_core::CueListId,
    ) -> Option<light_playback::PlaybackRuntimeStatus> {
        self.engine
            .playback_runtime_status_for_cue_list(cue_list_id)
    }

    pub(in crate::runtime) fn set_cue_external_completion_millis(
        &self,
        cue_list_id: light_core::CueListId,
        duration_millis: u64,
    ) -> bool {
        self.engine
            .set_cue_external_completion_millis(cue_list_id, duration_millis)
    }

    pub(in crate::runtime) fn active_dynamic_playback_at(
        &self,
        identity: light_playback::PlaybackIdentity,
    ) -> Option<light_playback::ActiveDynamicPlayback> {
        self.engine.active_dynamic_playback_at(identity)
    }

    pub(in crate::runtime) fn active_dynamic_playbacks_for_persistence(
        &self,
    ) -> Vec<light_playback::ActiveDynamicPlayback> {
        self.engine.active_dynamic_playbacks_for_persistence()
    }

    pub(in crate::runtime) fn playback_dynamics(&self) -> light_engine::PlaybackDynamicsProjection {
        self.engine.playback_dynamics()
    }

    pub(in crate::runtime) fn set_dynamic_runtime_paused(&self, paused: bool) {
        let now_millis =
            u64::try_from(self.engine.application_time().timestamp_millis()).unwrap_or_default();
        self.dynamics
            .lock()
            .apply_recorded_control(light_dynamics::TimedDynamicControl {
                at_millis: now_millis,
                control: light_dynamics::DynamicControl::GlobalPause(paused),
            })
            .expect("Global pause outside sampling has no fallible input");
    }

    pub(in crate::runtime) fn active_playbacks(&self) -> Vec<light_playback::ActivePlayback> {
        self.engine.active_playbacks()
    }

    pub(in crate::runtime) fn move_in_black_runtime(
        &self,
    ) -> Vec<light_engine::MoveInBlackDiagnostic> {
        self.engine.move_in_black_runtime()
    }

    pub(in crate::runtime) fn enabled_auto_off_playbacks(&self) -> Vec<u16> {
        self.engine.enabled_auto_off_playbacks()
    }

    pub(in crate::runtime) fn application_time(&self) -> chrono::DateTime<chrono::Utc> {
        self.engine.application_time()
    }

    pub(in crate::runtime) fn group_master_flash(&self, group_id: &str) -> f32 {
        self.engine.group_master_flash(group_id)
    }

    pub(in crate::runtime) fn group_master(&self, group_id: &str) -> Option<f32> {
        self.engine.group_master(group_id)
    }

    pub(in crate::runtime) fn group_master_for_persistence(&self, group_id: &str) -> Option<f32> {
        self.engine.group_master_for_persistence(group_id)
    }

    pub(in crate::runtime) fn set_highlighted_fixtures(
        &self,
        fixtures: impl IntoIterator<Item = light_core::FixtureId>,
    ) {
        self.engine.set_highlighted_fixtures(fixtures);
    }

    pub(in crate::runtime) fn set_highlight_layers(
        &self,
        layers: impl IntoIterator<Item = light_programmer::HighlightOutputLayer>,
    ) {
        self.engine.set_highlight_layers(layers);
    }

    pub(in crate::runtime) fn clear_highlighted_fixtures(&self) {
        self.engine.clear_highlighted_fixtures();
    }

    pub(in crate::runtime) fn set_highlight_look(
        &self,
        look: light_fixture::HighlightLook,
    ) -> Result<(), light_engine::EngineError> {
        self.engine.set_highlight_look(look)
    }

    pub(in crate::runtime) fn highlight_look_warnings(
        &self,
        look: &light_fixture::HighlightLook,
    ) -> Vec<String> {
        self.engine.highlight_look_warnings(look)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn highlighted_fixtures(&self) -> Vec<light_core::FixtureId> {
        self.engine.highlighted_fixtures()
    }

    pub(in crate::runtime) fn clear_programmer_transitions(&self) {
        self.engine.clear_programmer_transitions();
    }

    pub(in crate::runtime) fn set_control_timing(
        &self,
        speed_groups_bpm: [f64; 5],
        programmer_fade_millis: u64,
        sequence_master_fade_millis: u64,
        release_fade_millis: u64,
    ) {
        self.engine.set_control_timing(
            speed_groups_bpm,
            programmer_fade_millis,
            sequence_master_fade_millis,
            release_fade_millis,
        );
    }

    pub(in crate::runtime) fn prepare_playback_batch(
        &self,
        commands: &[light_engine::PlaybackBatchCommand],
        started_at: chrono::DateTime<chrono::Utc>,
        fallback_millis: u64,
    ) -> Result<light_engine::PreparedPlaybackBatch, String> {
        self.engine
            .prepare_playback_batch(commands, started_at, fallback_millis)
    }

    pub(in crate::runtime) fn install_prepared_playback_batch(
        &self,
        prepared: light_engine::PreparedPlaybackBatch,
    ) -> Result<(), String> {
        self.engine.install_prepared_playback_batch(prepared)
    }

    pub(in crate::runtime) fn install_prepared_snapshot_releasing_playback(
        &self,
        prepared: PreparedOutputSnapshot,
    ) {
        let _publication = self.dynamic_snapshot.begin_install();
        let snapshot = prepared.engine.snapshot_arc();
        if let Some(restored) = &prepared.restored {
            self.engine.reserve_playback_source_occurrence_watermark(
                restored.playback_source_occurrence_watermark,
            );
        }
        self.engine
            .install_prepared_snapshot_releasing_playback(prepared.engine);
        let mut dynamics = self.dynamics.lock();
        if let Some(restored) = prepared.restored {
            *dynamics = restored.runtime;
            self.dynamic_source_origins
                .store(Arc::new(restored.origins));
        } else {
            dynamics.install_prepared_definitions(prepared.definitions);
        }
        self.dynamic_snapshot.installed(snapshot);
    }

    pub(in crate::runtime) fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.prepare_snapshot(snapshot.clone()).map(|_| ())
    }

    #[cfg(test)]
    pub(in crate::runtime) fn render(
        &self,
        options: RenderOptions,
    ) -> Result<light_engine::RenderResult, EngineError> {
        self.engine.render(options)
    }

    pub(in crate::runtime) fn profile_visualization_values(
        &self,
        values: &light_engine::ResolvedValues,
        options: RenderOptions,
    ) -> Result<light_engine::ResolvedValues, EngineError> {
        self.engine.profile_visualization_values(values, options)
    }

    pub(in crate::runtime) fn execute_playback(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<EnginePlaybackOutcome, String> {
        self.engine.execute_playback(command)
    }

    pub(in crate::runtime) fn playback_control_state_at(
        &self,
        identity: light_playback::PlaybackIdentity,
    ) -> light_playback::PlaybackControlState {
        self.engine.playback_control_state_at(identity)
    }

    pub(in crate::runtime) fn execute_pool_playback_with_activation(
        &self,
        number: u16,
        action: PoolPlaybackAction,
        exclusion_zones: &[Vec<u16>],
        activation_origin: Option<light_playback::PlaybackActivationOrigin>,
    ) -> Result<light_engine::PoolPlaybackTransition, String> {
        self.engine.execute_pool_playback_with_activation(
            number,
            action,
            exclusion_zones,
            activation_origin,
        )
    }

    pub(in crate::runtime) fn set_group_master(
        &self,
        group_id: &str,
        value: f32,
    ) -> Result<bool, EngineError> {
        self.engine.set_group_master(group_id, value)
    }

    pub(in crate::runtime) fn set_group_master_transition(
        &self,
        group_id: &str,
        value: f32,
        duration_millis: u64,
    ) -> Result<bool, EngineError> {
        self.engine
            .set_group_master_transition(group_id, value, duration_millis)
    }

    pub(in crate::runtime) fn set_group_master_flash(&self, group_id: String, value: f32) {
        self.engine.set_group_master_flash(group_id, value);
    }

    pub(in crate::runtime) fn set_speed_groups_paused(&self, paused: [bool; 5]) {
        self.engine.set_speed_groups_paused(paused);
    }

    pub(in crate::runtime) fn set_timecode_frame(&self, frame: Option<u64>) {
        self.engine.set_timecode_frame(frame);
    }

    pub(in crate::runtime) fn render_with_playback_events(
        &self,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        options: RenderOptions,
    ) -> Result<RenderedSemanticFrame, EngineError> {
        self.render_with_playback_events_timed(active_show, playback, options)
            .map(|(rendered, _)| rendered)
    }

    pub(in crate::runtime) fn render_with_playback_events_timed(
        &self,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        options: RenderOptions,
    ) -> Result<(RenderedSemanticFrame, OutputSemanticRenderTiming), EngineError> {
        self.render_with_playback_events_timed_with_capture(active_show, playback, options, |_| {})
    }

    /// The callback runs inside the same ordered Playback operation as the authoritative
    /// capture. It may consume captured Cue actions, but must not enter another Playback UOW.
    pub(in crate::runtime) fn render_with_playback_events_timed_with_capture(
        &self,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        options: RenderOptions,
        on_capture: impl FnOnce(&light_engine::PreparedOutputFrame),
    ) -> Result<(RenderedSemanticFrame, OutputSemanticRenderTiming), EngineError> {
        output_scheduler::ordered_output_operation(playback, || {
            let dynamic_started = Instant::now();
            let Some(prepared) = self.engine.try_prepare_output_frame(options) else {
                return light_application::PlaybackOperation::new(Err(
                    EngineError::StalePreparedFrame,
                ));
            };
            let prepared =
                crate::runtime::dynamic_snapshot_publication::RetainedFrameCapture::select(
                    prepared,
                    &self.dynamic_snapshot,
                    dynamic_started,
                );
            let mut events = Vec::new();
            let completed = output_scheduler::dynamic_output_frame(
                &self.engine,
                &prepared,
                prepared.retained(),
                &[],
                &self.dynamics,
                &self.dynamic_snapshot,
                &self.dynamic_source_origins,
                &self.speed_groups,
                &self.rate,
                &self.programmer_reconciliation_cache,
                &self.family_adapters,
                |source| {
                    let dynamic = dynamic_started.elapsed();
                    let engine_started = Instant::now();
                    let operation = source.playback_operation(
                        &self.engine,
                        active_show,
                        playback,
                        &prepared,
                        None,
                    );
                    events.extend(operation.events);
                    operation.output.map(|rendered| {
                        (
                            rendered,
                            OutputSemanticRenderTiming {
                                dynamic,
                                engine: engine_started.elapsed(),
                            },
                        )
                    })
                },
            );
            if completed.is_err() {
                events.extend(output_scheduler::captured_playback_events(
                    &self.engine,
                    active_show,
                    playback,
                    &prepared,
                    None,
                    None,
                ));
            }
            on_capture(&prepared);
            let result = completed.map(|completed| {
                self.dynamic_auto_offs.lock().extend(completed.auto_offs);
                events.extend(completed.events);
                let dynamics = Arc::new(FrameDynamicSources {
                    sample_boundary: completed.sample_boundary,
                    runtime: completed.runtime,
                    samples: completed.samples,
                    origins: completed.origins,
                    programmer_values: Arc::clone(prepared.dynamic_programmer_values()),
                    cue_values: prepared.cue_dynamic_values().into(),
                    ordinary: completed.ordinary,
                });
                let (rendered, timing) = completed.output;
                (
                    RenderedSemanticFrame {
                        rendered,
                        options,
                        dynamics: Some(dynamics),
                    },
                    timing,
                )
            });
            light_application::PlaybackOperation::with_events(result, events)
        })
    }

    #[cfg(test)]
    pub(in crate::runtime) fn dynamic_contributions_for_test(
        &self,
    ) -> Vec<light_engine::ContributionBatch> {
        output_scheduler::dynamic_contributions(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            false,
        )
    }

    pub(in crate::runtime) fn frame_rate_hz(&self) -> u16 {
        self.rate.load(Ordering::Relaxed)
    }

    pub(in crate::runtime) fn set_frame_rate_hz(&self, frame_rate_hz: u16) {
        self.rate.store(frame_rate_hz, Ordering::Relaxed);
    }

    pub(in crate::runtime) fn route_send_errors(&self) -> Vec<light_output::RouteSendError> {
        self.network
            .as_ref()
            .map(|output| output.route_send_errors())
            .unwrap_or_default()
    }

    /// What the network output sent and heard, or `None` without a network output.
    pub(in crate::runtime) fn network_activity(&self) -> Option<light_output::NetworkActivity> {
        self.network
            .as_ref()
            .map(|output| output.network_activity())
    }

    pub(in crate::runtime) fn usb_diagnostics(&self) -> Vec<light_output::UsbEndpointDiagnostic> {
        self.usb.diagnostics()
    }

    pub(in crate::runtime) fn configure_usb_endpoints(
        &self,
        document: &light_output::UsbEndpointDocument,
    ) -> Result<(), String> {
        self.usb.configure(document)
    }

    pub(in crate::runtime) async fn lock_usb_configuration(
        &self,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.usb_configuration_lock).lock_owned().await
    }

    pub(in crate::runtime) fn take_send_errors(&self) -> u64 {
        self.network
            .as_ref()
            .map(|output| output.take_send_errors())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(in crate::runtime) fn has_network_output(&self) -> bool {
        self.network.is_some()
    }

    pub(in crate::runtime) fn inject_network_failure(
        &self,
        destination: SocketAddr,
        enabled: bool,
    ) -> Result<(), ApiError> {
        self.network
            .as_ref()
            .ok_or_else(|| ApiError::unavailable("network output is unavailable"))?
            .inject_failure(destination, enabled);
        Ok(())
    }

    pub(in crate::runtime) fn clear_runtime_replay(&self) {
        self.runtime_service.clear_replay();
    }

    pub(in crate::runtime) fn handle_runtime_action<P: light_application::OutputRuntimePorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::OutputRuntimeCommand>,
        ports: &P,
    ) -> Result<light_application::OutputRuntimeResult, light_application::ActionError> {
        self.runtime_service.handle(action, ports)
    }
}
