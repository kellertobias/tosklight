use super::*;

mod runtime;

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
    sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
    manual_clock: Option<Arc<ManualClock>>,
    test_clock_lock: Arc<tokio::sync::Mutex<()>>,
    speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
    dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    programmer_reconciliation_cache: Arc<output_scheduler::ProgrammerReconciliationCache>,
    visualization_dynamics: Arc<Mutex<Option<CachedVisualizationDynamics>>>,
    visualization_ordinary: Arc<Mutex<Option<CachedVisualizationOrdinary>>>,
    dynamic_auto_offs: Arc<Mutex<Vec<light_playback::PlaybackIdentity>>>,
    visualization_frames: Arc<super::visualization_frame::VisualizationFrameHub>,
    sound_capture_owners: Arc<Mutex<[Option<SoundCaptureOwner>; 5]>>,
    #[cfg(test)]
    runtime_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    runtime_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    speed_group_persistence_attempts: Arc<AtomicU64>,
    #[cfg(test)]
    speed_group_persistence_failure: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub(in crate::runtime) struct CachedVisualizationDynamics {
    pub(in crate::runtime) runtime: light_dynamics::DynamicRuntimeSnapshot,
    pub(in crate::runtime) samples: Vec<light_dynamics::DynamicRuntimeSample>,
}

pub(in crate::runtime) struct OutputSemanticRenderTiming {
    pub(in crate::runtime) dynamic: Duration,
    pub(in crate::runtime) engine: Duration,
}

struct CachedVisualizationOrdinary {
    snapshot: Arc<EngineSnapshot>,
    captured_at: std::time::Instant,
    values:
        Arc<HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>>,
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
        sequences: Arc<tokio::sync::Mutex<HashMap<(light_output::Protocol, u16), u8>>>,
        manual_clock: Option<Arc<ManualClock>>,
        speed_groups: Arc<Mutex<[SpeedGroupController; 5]>>,
        dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
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
            sequences,
            manual_clock,
            test_clock_lock: Arc::default(),
            speed_groups,
            dynamics,
            programmer_reconciliation_cache: Arc::new(
                output_scheduler::ProgrammerReconciliationCache::default(),
            ),
            visualization_dynamics: Arc::new(Mutex::new(None)),
            visualization_ordinary: Arc::new(Mutex::new(None)),
            dynamic_auto_offs,
            visualization_frames,
            sound_capture_owners: Arc::new(Mutex::new([None; 5])),
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
        self.health
            .lock()
            .expect("output health mutex poisoned")
            .clone()
    }

    pub(in crate::runtime) fn latest_visualization_frame(
        &self,
    ) -> Option<Arc<super::visualization_frame::PublishedVisualizationFrame>> {
        self.visualization_frames.latest()
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
        self.dynamics.lock().start(request)
    }

    pub(in crate::runtime) fn dynamic_runtime_snapshot(
        &self,
    ) -> light_dynamics::DynamicRuntimeSnapshot {
        self.dynamics.lock().snapshot()
    }

    pub(in crate::runtime) fn restore_dynamic_runtime_snapshot(
        &self,
        snapshot: light_dynamics::DynamicRuntimeSnapshot,
    ) -> Result<(), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().restore_snapshot(snapshot)
    }

    pub(in crate::runtime) fn off_dynamic_controller(
        &self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().off_controller_by_id(
            controller_id,
            now_millis,
            release_delay_millis,
            release_duration_millis,
        )
    }

    pub(in crate::runtime) fn update_dynamic_controller(
        &self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), light_dynamics::DynamicRuntimeError> {
        self.dynamics.lock().update_controller(
            controller_id,
            size,
            speed_multiplier,
            phase_offset_degrees,
        )
    }

    pub(in crate::runtime) fn is_dynamic_definition_running(&self, definition_id: Uuid) -> bool {
        self.dynamics.lock().is_definition_running(definition_id)
    }

    pub(in crate::runtime) fn set_dynamic_definitions_pinned(&self, pinned: bool) {
        self.dynamics.lock().set_definitions_pinned(pinned);
    }

    pub(in crate::runtime) fn replace_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        let definitions = snapshot.dynamics.iter().cloned().collect::<Vec<_>>();
        self.engine.replace_snapshot(snapshot)?;
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("Engine validation and Dynamic validation stay equivalent");
        Ok(())
    }

    pub(in crate::runtime) fn prepare_snapshot(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<PreparedEngineSnapshot, EngineError> {
        self.engine.prepare_snapshot(snapshot)
    }

    pub(in crate::runtime) fn install_prepared_snapshot(&self, prepared: PreparedEngineSnapshot) {
        let definitions = prepared
            .snapshot()
            .dynamics
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.engine.install_prepared_snapshot(prepared);
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("prepared Engine snapshot contains valid Dynamic definitions");
    }

    pub(in crate::runtime) fn resolved_values(
        &self,
    ) -> HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>
    {
        self.engine.resolved_values()
    }

    pub(in crate::runtime) fn cached_visualization_ordinary_values(
        &self,
    ) -> Arc<HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>>
    {
        let snapshot = self.engine.snapshot();
        let mut cached = self.visualization_ordinary.lock();
        if let Some(cached) = cached.as_ref()
            && Arc::ptr_eq(&cached.snapshot, &snapshot)
            && cached.captured_at.elapsed() < Duration::from_millis(250)
        {
            return Arc::clone(&cached.values);
        }
        let values = Arc::new(self.engine.resolved_values());
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
    ) -> HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>
    {
        self.visualization_dynamic_projection(extra_programmer_values, projected)
            .0
    }

    pub(in crate::runtime) fn visualization_dynamic_projection(
        &self,
        extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
        _projected: bool,
    ) -> (
        HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>,
        light_dynamics::DynamicRuntimeSnapshot,
        Vec<light_dynamics::DynamicRuntimeSample>,
    ) {
        // Visualization is observational. Sampling can retire completed release
        // transitions, so always operate on a clone and leave the authoritative
        // output scheduler responsible for mutating and publishing runtime state.
        let snapshot = self.engine.snapshot();
        let mut visualization_runtime = light_dynamics::DynamicRuntime::default();
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

    pub(in crate::runtime) fn cached_visualization_dynamics(
        &self,
    ) -> Option<CachedVisualizationDynamics> {
        self.visualization_dynamics.lock().clone()
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
        let _ = output_scheduler::dynamic_contributions(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            false,
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

    pub(in crate::runtime) fn active_dynamic_playbacks(
        &self,
    ) -> Vec<light_playback::ActiveDynamicPlayback> {
        self.engine.active_dynamic_playbacks()
    }

    pub(in crate::runtime) fn playback_dynamics(&self) -> light_engine::PlaybackDynamicsProjection {
        self.engine.playback_dynamics()
    }

    pub(in crate::runtime) fn set_dynamic_runtime_paused(&self, paused: bool) {
        let now_millis =
            u64::try_from(self.engine.application_time().timestamp_millis()).unwrap_or_default();
        self.dynamics.lock().set_global_paused(paused, now_millis);
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
    ) {
        self.engine.set_control_timing(
            speed_groups_bpm,
            programmer_fade_millis,
            sequence_master_fade_millis,
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
        prepared: PreparedEngineSnapshot,
    ) {
        let definitions = prepared
            .snapshot()
            .dynamics
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.engine
            .install_prepared_snapshot_releasing_playback(prepared);
        self.dynamics
            .lock()
            .install_definitions(definitions)
            .expect("prepared Engine snapshot contains valid Dynamic definitions");
    }

    pub(in crate::runtime) fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.engine.validate_snapshot_for_runtime(snapshot)
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
        values: &HashMap<
            (light_core::FixtureId, light_core::AttributeKey),
            light_core::AttributeValue,
        >,
        options: RenderOptions,
    ) -> Result<
        HashMap<(light_core::FixtureId, light_core::AttributeKey), light_core::AttributeValue>,
        EngineError,
    > {
        self.engine.profile_visualization_values(values, options)
    }

    pub(in crate::runtime) fn execute_playback(
        &self,
        command: EnginePlaybackCommand,
    ) -> Result<EnginePlaybackOutcome, String> {
        self.engine.execute_playback(command)
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
    ) -> Result<light_engine::RenderResult, EngineError> {
        self.render_with_playback_events_timed(active_show, playback, options)
            .map(|(rendered, _)| rendered)
    }

    pub(in crate::runtime) fn render_with_playback_events_timed(
        &self,
        active_show: &ActiveShowProjection,
        playback: &PlaybackRenderCapability,
        options: RenderOptions,
    ) -> Result<(light_engine::RenderResult, OutputSemanticRenderTiming), EngineError> {
        let dynamic_started = Instant::now();
        let (sampled, runtime, samples) = output_scheduler::dynamic_contributions_cached(
            &self.engine,
            &self.dynamics,
            &self.speed_groups,
            &self.rate,
            &[],
            &self.programmer_reconciliation_cache,
            true,
        );
        let dynamic = dynamic_started.elapsed();
        *self.visualization_dynamics.lock() =
            Some(CachedVisualizationDynamics { runtime, samples });
        let engine_started = Instant::now();
        let rendered = output_scheduler::render_with_playback_events(
            &self.engine,
            active_show,
            playback,
            options,
            &sampled,
        )?;
        Ok((
            rendered,
            OutputSemanticRenderTiming {
                dynamic,
                engine: engine_started.elapsed(),
            },
        ))
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
