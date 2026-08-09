use super::*;

impl OutputResource {
    pub(in crate::runtime) fn runtime_snapshot<P: light_application::OutputRuntimePorts>(
        &self,
        context: &light_application::ActionContext,
        identity: light_application::OutputRuntimeIdentity,
        ports: &P,
    ) -> Result<light_application::OutputRuntimeSnapshot, light_application::ActionError> {
        self.runtime_service.snapshot(context, identity, ports)
    }

    pub(in crate::runtime) fn handle_speed_group_action<P: light_application::SpeedGroupPorts>(
        &self,
        action: light_application::ActionEnvelope<light_application::SpeedGroupCommand>,
        ports: &P,
    ) -> Result<light_application::SpeedGroupResult, light_application::ActionError> {
        self.speed_group_service.handle(action, ports)
    }

    pub(in crate::runtime) fn record_speed_group_external_change<
        P: light_application::SpeedGroupPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        ports: &P,
        changed: &[light_application::SpeedGroupId],
        applied_at_millis: u64,
    ) -> Result<u64, light_application::ActionError> {
        self.speed_group_service
            .record_external_change(context, ports, changed, applied_at_millis)
    }

    pub(in crate::runtime) fn speed_group_service_snapshot<
        P: light_application::SpeedGroupPorts,
    >(
        &self,
        context: &light_application::ActionContext,
        ports: &P,
    ) -> Result<light_application::SpeedGroupSnapshot, light_application::ActionError> {
        self.speed_group_service.snapshot(context, ports)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn rebind_speed_group_events(&mut self, events: &EventResource) {
        self.speed_group_service = SpeedGroupService::new(events.application.clone());
    }

    pub(in crate::runtime) fn has_test_clock(&self) -> bool {
        self.manual_clock.is_some()
    }

    pub(in crate::runtime) async fn acquire_test_clock(
        &self,
    ) -> Result<TestClockSession, ApiError> {
        let clock = self
            .manual_clock
            .as_ref()
            .cloned()
            .ok_or_else(|| ApiError::not_found("test clock"))?;
        Ok(TestClockSession {
            _guard: Arc::clone(&self.test_clock_lock).lock_owned().await,
            driver: TestClockDriver { clock },
        })
    }

    pub(in crate::runtime) fn record_output_health(&self, packets_sent: u64, send_errors: u64) {
        let mut health = self.health.lock().expect("output health mutex poisoned");
        health.frames_sent += 1;
        health.packets_sent += packets_sent;
        health.send_errors += send_errors;
    }

    pub(in crate::runtime) async fn run_output_scheduler<F, Fut>(
        &self,
        cancellation: CancellationToken,
        tick: F,
    ) where
        F: FnMut() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = std::io::Result<u64>> + Send + 'static,
    {
        let rate = Arc::clone(&self.rate);
        let health = Arc::clone(&self.health);
        let runtime = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            runtime.block_on(light_output::run_scheduler_dynamic(
                rate,
                cancellation,
                health,
                tick,
            ));
        })
        .await
        .expect("isolated test output scheduler task joins");
    }

    pub(in crate::runtime) fn control_projection(&self) -> OutputRuntimeControlProjection {
        let control = self.control.lock();
        OutputRuntimeControlProjection {
            revision: control.revision,
            grand_master: control.options.grand_master,
            blackout: control.options.blackout,
            grand_master_flash: control.grand_master_flash,
        }
    }

    pub(in crate::runtime) fn render_options(&self) -> light_engine::RenderOptions {
        self.control.lock().render_options()
    }

    pub(in crate::runtime) fn restore_runtime_control(&self, runtime: &PersistedOutputRuntime) {
        let mut control = self.control.lock();
        control.options.grand_master = runtime.grand_master;
        control.options.blackout = runtime.blackout;
        control.revision = runtime.revision;
    }

    pub(in crate::runtime) fn apply_runtime_control(
        &self,
        grand_master: Option<f32>,
        blackout: Option<bool>,
    ) -> Result<u64, light_application::ActionError> {
        let mut control = self.control.lock();
        let next_revision = control.revision.checked_add(1).ok_or_else(|| {
            light_application::ActionError::new(
                light_application::ActionErrorKind::Unavailable,
                "output revision is exhausted",
            )
        })?;
        if let Some(grand_master) = grand_master {
            control.options.grand_master = grand_master;
        }
        if let Some(blackout) = blackout {
            control.options.blackout = blackout;
        }
        control.revision = next_revision;
        Ok(next_revision)
    }

    pub(in crate::runtime) fn set_transition_hold(&self, hold: bool) {
        self.control.lock().hold = hold;
    }

    pub(in crate::runtime) fn set_transition_blackout(&self, blackout: bool) {
        self.control.lock().options.blackout = blackout;
    }

    pub(in crate::runtime) fn set_transition_grand_master(&self, grand_master: f32) {
        self.control.lock().options.grand_master = grand_master;
    }

    pub(in crate::runtime) fn set_grand_master_flash(&self, pressed: bool) -> bool {
        let mut control = self.control.lock();
        if control.grand_master_flash == pressed {
            false
        } else {
            control.grand_master_flash = pressed;
            true
        }
    }

    pub(in crate::runtime) fn set_dmx_override(
        &self,
        universe: light_core::Universe,
        address: light_core::DmxAddress,
        value: Option<u8>,
    ) {
        let mut control = self.control.lock();
        if let Some(value) = value {
            control.raw_overrides.insert((universe, address), value);
        } else {
            control.raw_overrides.remove(&(universe, address));
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn dmx_override(
        &self,
        universe: light_core::Universe,
        address: light_core::DmxAddress,
    ) -> Option<u8> {
        self.control
            .lock()
            .raw_overrides
            .get(&(universe, address))
            .copied()
    }

    pub(in crate::runtime) fn dmx_snapshot(&self, revision: u64) -> serde_json::Value {
        let control = self.control.lock();
        let mut universes = control
            .last_frames
            .iter()
            .map(|(&universe, frame)| {
                serde_json::json!({"universe":universe,"slots":frame.to_vec()})
            })
            .collect::<Vec<_>>();
        universes.sort_by_key(|universe| universe["universe"].as_u64().unwrap_or_default());
        serde_json::json!({
            "revision": revision,
            "universes": universes,
            "overrides": control.raw_overrides.iter().map(|(&(universe,address),&value)| {
                serde_json::json!({"universe":universe,"address":address,"value":value})
            }).collect::<Vec<_>>()
        })
    }

    pub(in crate::runtime) fn configure_timecode(
        &self,
        sources: Vec<light_control::TimecodeSourceConfig>,
    ) {
        self.timecode.lock().configure(sources);
    }

    pub(in crate::runtime) fn ingest_timecode(
        &self,
        timecode: light_control::SmpteTimecode,
    ) -> Option<light_control::SmpteTimecode> {
        self.timecode.lock().ingest(timecode).cloned()
    }

    pub(in crate::runtime) fn timecode_status(
        &self,
    ) -> (Option<String>, Option<light_control::SmpteTimecode>) {
        let router = self.timecode.lock();
        (
            router.active_source().map(str::to_owned),
            router.current().cloned(),
        )
    }

    pub(in crate::runtime) async fn clear_sequences(&self) {
        self.sequences.lock().await.clear();
    }

    pub(in crate::runtime) fn render_frames_and_publish(
        &self,
        rendered: &light_engine::RenderResult,
        visualization_scope: light_wire::v2::visualization::VisualizationScope,
    ) -> HashMap<light_core::Universe, light_output::DmxFrame> {
        let mut control = self.control.lock();
        if control.hold {
            return control.last_frames.clone();
        }
        let mut frames = rendered.universes.clone();
        for (&(universe, address), &value) in &control.raw_overrides {
            if let Some(frame) = frames.get_mut(&universe) {
                frame[usize::from(address - 1)] = value;
            }
        }
        control.last_frames = frames.clone();
        self.visualization_frames
            .publish(rendered, control.render_options(), visualization_scope);
        frames
    }

    pub(in crate::runtime) async fn send_network_routes(
        &self,
        routes: &[light_output::OutputRoute],
        frames: &HashMap<light_core::Universe, light_output::DmxFrame>,
        patched_slots: &HashMap<light_core::Universe, u16>,
    ) -> Result<u64, std::io::Error> {
        let output = self
            .network
            .as_ref()
            .ok_or_else(|| std::io::Error::other("network output is unavailable"))?;
        let network = output
            .send_routes(
                routes,
                frames,
                patched_slots,
                &mut *self.sequences.lock().await,
            )
            .await;
        let usb = self.usb.enqueue_routes(routes, frames);
        crate::runtime::output_scheduler::combined_delivery_result(network, usb)
    }

    pub(in crate::runtime) async fn terminate_routes(&self, routes: &[light_output::OutputRoute]) {
        if let Some(output) = &self.network {
            let _ = output
                .terminate_routes(routes, &mut *self.sequences.lock().await)
                .await;
        }
        self.usb.terminate_routes(routes);
    }

    pub(in crate::runtime) fn reset_speed_groups(
        &self,
        manual_bpm: [f64; 5],
        sound: [SoundToLightConfig; 5],
    ) {
        *self.speed_groups.lock() = std::array::from_fn(|index| {
            SpeedGroupController::new(manual_bpm[index], sound[index].clone())
                .expect("validated Speed Group configuration")
        });
        *self.sound_capture_owners.lock() = [None; 5];
    }

    pub(in crate::runtime) fn speed_group_snapshots(&self, now: u64) -> [SpeedSnapshot; 5] {
        let controllers = self.speed_groups.lock();
        std::array::from_fn(|index| controllers[index].snapshot(now))
    }

    pub(in crate::runtime) fn speed_group_snapshot(&self, index: usize, now: u64) -> SpeedSnapshot {
        self.speed_groups.lock()[index].snapshot(now)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn speed_group_controller(&self, index: usize) -> SpeedGroupController {
        self.speed_groups.lock()[index].clone()
    }

    pub(in crate::runtime) fn speed_group_sound_config(&self, index: usize) -> SoundToLightConfig {
        self.speed_groups.lock()[index].sound_config().clone()
    }

    pub(in crate::runtime) fn speed_group_manual_bpm(&self, index: usize) -> f64 {
        self.speed_groups.lock()[index].manual_bpm()
    }

    pub(in crate::runtime) fn configure_speed_groups(
        &self,
        previous_bpm: [f64; 5],
        next_bpm: [f64; 5],
        next_sound: [SoundToLightConfig; 5],
        now: u64,
    ) -> Result<[SoundToLightConfig; 5], ApiError> {
        let mut controllers = self.speed_groups.lock();
        let mut applied_sound = next_sound;
        for index in 0..controllers.len() {
            if next_bpm[index] != previous_bpm[index] {
                speed_groups::unlink_speed_group(&mut controllers, index, now);
                controllers[index]
                    .set_manual_bpm(next_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                controllers[index].set_paused_at(false, now);
                applied_sound[index].enabled = false;
                self.sound_capture_owners.lock()[index] = None;
            } else {
                controllers[index]
                    .set_manual_fallback_bpm(next_bpm[index])
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            controllers[index]
                .set_sound_config(applied_sound[index].clone())
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        Ok(applied_sound)
    }

    pub(in crate::runtime) fn tap_speed_group(
        &self,
        index: usize,
        now: u64,
    ) -> light_control::speed::LearnResult {
        let mut controllers = self.speed_groups.lock();
        speed_groups::unlink_speed_group(&mut controllers, index, now);
        let result = controllers[index].tap_learn(now);
        self.sound_capture_owners.lock()[index] = None;
        result
    }

    pub(in crate::runtime) fn set_manual_speed_group(
        &self,
        index: usize,
        bpm: f64,
        now: u64,
        disable_sound: bool,
    ) -> Result<(), ApiError> {
        let mut controllers = self.speed_groups.lock();
        speed_groups::unlink_speed_group(&mut controllers, index, now);
        controllers[index]
            .set_manual_bpm(bpm)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index]
            .set_speed_master_scale(1.0)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        controllers[index].set_paused_at(false, now);
        if disable_sound {
            let mut sound = controllers[index].sound_config().clone();
            sound.enabled = false;
            controllers[index]
                .set_sound_config(sound)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        self.sound_capture_owners.lock()[index] = None;
        Ok(())
    }

    pub(in crate::runtime) fn set_speed_group_sound_config(
        &self,
        index: usize,
        configuration: SoundToLightConfig,
    ) -> Result<(), ApiError> {
        self.speed_groups.lock()[index]
            .set_sound_config(configuration)
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }

    pub(in crate::runtime) fn set_speed_group_manual_fallback(
        &self,
        index: usize,
        bpm: f64,
    ) -> Result<(), ApiError> {
        self.speed_groups.lock()[index]
            .set_manual_fallback_bpm(bpm)
            .map_err(|error| ApiError::bad_request(error.to_string()))
    }

    #[cfg(test)]
    pub(in crate::runtime) fn configure_speed_group_test_state(
        &self,
        index: usize,
        sound: SoundToLightConfig,
        scale: f64,
        paused: bool,
        now: u64,
    ) {
        let mut controllers = self.speed_groups.lock();
        controllers[index].set_sound_config(sound).unwrap();
        controllers[index].set_speed_master_scale(scale).unwrap();
        controllers[index].set_paused_at(paused, now);
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_speed_group_scale_for_test(&self, index: usize, scale: f64) {
        self.speed_groups.lock()[index]
            .set_speed_master_scale(scale)
            .unwrap();
    }

    pub(in crate::runtime) fn observe_speed_group_sound(
        &self,
        index: usize,
        desk_id: Uuid,
        now: u64,
        mut observation: SoundObservation,
    ) -> Result<(), ApiError> {
        if !self.speed_groups.lock()[index].sound_config().enabled {
            return Err(ApiError::conflict(
                "enable Sound to Light before submitting observations",
            ));
        }
        {
            let mut owners = self.sound_capture_owners.lock();
            if owners[index].is_some_and(|owner| {
                owner.desk_id != desk_id && now.saturating_sub(owner.last_seen_millis) <= 3_000
            }) {
                return Err(ApiError::conflict(
                    "this Speed Group is receiving audio from another desk",
                ));
            }
            owners[index] = Some(SoundCaptureOwner {
                desk_id,
                last_seen_millis: now,
            });
        }
        observation.captured_at_millis = now;
        self.speed_groups.lock()[index].observe_sound(observation);
        Ok(())
    }

    pub(in crate::runtime) fn apply_speed_group_action(
        &self,
        index: usize,
        now: u64,
        action: &str,
    ) -> Result<Vec<usize>, ApiError> {
        let mut controllers = self.speed_groups.lock();
        let affected = match action {
            "learn" => {
                speed_groups::unlink_speed_group(&mut controllers, index, now);
                controllers[index].tap_learn(now);
                vec![index]
            }
            "double" => {
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].double();
                }
                affected
            }
            "half" => {
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].half();
                }
                affected
            }
            "pause" => {
                let paused = !controllers[index].snapshot(now).paused;
                let affected = speed_groups::speed_group_action_indices(&controllers, index);
                for &affected_index in &affected {
                    controllers[affected_index].set_paused_at(paused, now);
                }
                affected
            }
            _ => {
                return Err(ApiError::bad_request(
                    "Speed Group action must be learn, double, half, or pause",
                ));
            }
        };
        if action == "learn" {
            self.sound_capture_owners.lock()[index] = None;
        }
        Ok(affected)
    }

    pub(in crate::runtime) fn set_speed_group_level(
        &self,
        index: usize,
        now: u64,
        level: f32,
    ) -> Result<Vec<usize>, ApiError> {
        if !level.is_finite() || !(0.0..=1.0).contains(&level) {
            return Err(ApiError::bad_request(
                "Speed Group level must be finite and within 0-1",
            ));
        }
        let mut controllers = self.speed_groups.lock();
        let affected = speed_groups::speed_group_action_indices(&controllers, index);
        for &affected_index in &affected {
            controllers[affected_index]
                .set_speed_master_scale(f64::from(level))
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            controllers[affected_index].set_paused_at(level == 0.0, now);
        }
        Ok(affected)
    }

    pub(in crate::runtime) fn speed_group_port_state(
        &self,
    ) -> light_application::SpeedGroupPortState {
        let controllers = self.speed_groups.lock();
        let owners = self.sound_capture_owners.lock();
        let groups = controllers
            .iter()
            .enumerate()
            .map(|(index, controller)| {
                let snapshot = controller.snapshot(0);
                light_application::SpeedGroupProjection {
                    group: light_application::SpeedGroupId::new((index + 1) as u8)
                        .expect("fixed Speed Group index"),
                    manual_bpm: snapshot.manual_bpm,
                    paused: snapshot.paused,
                    speed_master_scale: snapshot.speed_master_scale,
                    synchronized_with: snapshot
                        .synchronized_with
                        .and_then(light_application::SpeedGroupId::new),
                    phase_origin_millis: snapshot.phase_origin_millis,
                }
            })
            .collect();
        let manual_control_clean = controllers
            .iter()
            .enumerate()
            .filter(|(index, controller)| {
                controller.manual_entry_is_current(controller.manual_bpm())
                    && owners[*index].is_none()
            })
            .filter_map(|(index, _)| light_application::SpeedGroupId::new((index + 1) as u8))
            .collect();
        light_application::SpeedGroupPortState {
            groups,
            manual_control_clean,
        }
    }

    pub(in crate::runtime) fn apply_resolved_speed_group_action(
        &self,
        action: light_application::SpeedGroupResolvedAction,
    ) -> Result<Vec<usize>, light_application::ActionError> {
        use light_application::{ActionError, ActionErrorKind, SpeedGroupResolvedAction};
        let mut controllers = self.speed_groups.lock();
        let affected = match action {
            SpeedGroupResolvedAction::SetManualBpm {
                group,
                bpm,
                applied_at_millis,
            } => {
                let index = group.index();
                speed_groups::unlink_speed_group(&mut controllers, index, applied_at_millis);
                controllers[index].set_manual_bpm(bpm).map_err(|error| {
                    ActionError::new(ActionErrorKind::Invalid, error.to_string())
                })?;
                controllers[index]
                    .set_speed_master_scale(1.0)
                    .map_err(|error| {
                        ActionError::new(ActionErrorKind::Invalid, error.to_string())
                    })?;
                controllers[index].set_paused_at(false, applied_at_millis);
                vec![index]
            }
            SpeedGroupResolvedAction::Synchronize {
                source,
                target,
                applied_at_millis,
            } => {
                speed_groups::synchronize_speed_groups(
                    &mut controllers,
                    source.index(),
                    target.index(),
                    applied_at_millis,
                )
                .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.message))?;
                vec![source.index(), target.index()]
            }
        };
        self.clear_sound_capture_owners(&affected);
        Ok(affected)
    }

    pub(in crate::runtime) fn apply_speed_group_playback(
        &self,
        index: usize,
        now: u64,
        action: &str,
        input: &PoolPlaybackInput,
        fader: light_playback::PlaybackFaderMode,
        configured_source: SpeedGroupSource,
        linked_fallback: Option<f64>,
    ) -> Result<(bool, Vec<usize>, bool), ApiError> {
        let takes_manual_control = action != "pause";
        let mut controllers = self.speed_groups.lock();
        let owner_present = self.sound_capture_owners.lock()[index].is_some();
        let manual_ownership_changed = takes_manual_control
            && (configured_source != SpeedGroupSource::Manual
                || controllers[index].sound_config().enabled
                || owner_present);
        if let Some(effective_bpm) = linked_fallback {
            controllers[index]
                .set_manual_fallback_bpm(effective_bpm)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        if takes_manual_control {
            let mut sound = controllers[index].sound_config().clone();
            sound.enabled = false;
            controllers[index]
                .set_sound_config(sound)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        let before = playback_speed_groups::controller_snapshots(&controllers, now);
        let (affected, clear_owner) = playback_speed_groups::apply_speed_action(
            &mut controllers,
            index,
            now,
            action,
            input,
            fader,
            owner_present,
        )?;
        let changed = action == "learn"
            || manual_ownership_changed
            || playback_speed_groups::speed_group_changed(&before, &controllers, &affected, now);
        drop(controllers);
        if clear_owner || takes_manual_control {
            self.clear_sound_capture_owner(index);
        }
        Ok((changed, affected, takes_manual_control))
    }

    #[cfg(test)]
    pub(in crate::runtime) fn sound_capture_owner(
        &self,
        index: usize,
    ) -> Option<SoundCaptureOwner> {
        self.sound_capture_owners.lock()[index]
    }

    pub(in crate::runtime) fn clear_sound_capture_owner(&self, index: usize) {
        self.sound_capture_owners.lock()[index] = None;
    }

    #[cfg(test)]
    pub(in crate::runtime) fn set_sound_capture_owner(
        &self,
        index: usize,
        owner: Option<SoundCaptureOwner>,
    ) {
        self.sound_capture_owners.lock()[index] = owner;
    }

    pub(in crate::runtime) fn clear_sound_capture_owners(&self, indices: &[usize]) {
        let mut owners = self.sound_capture_owners.lock();
        for &index in indices {
            owners[index] = None;
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn runtime_persistence_attempts(&self) -> u64 {
        self.runtime_persistence_attempts.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn record_runtime_persistence_attempt(&self) -> Result<(), ApiError> {
        self.runtime_persistence_attempts
            .fetch_add(1, Ordering::SeqCst);
        if self.runtime_persistence_failure.load(Ordering::SeqCst) {
            Err(ApiError::unavailable(
                "injected output runtime persistence failure",
            ))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn force_runtime_persistence_failure(&self, fail: bool) {
        self.runtime_persistence_failure
            .store(fail, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(in crate::runtime) fn speed_group_persistence_attempts(&self) -> u64 {
        self.speed_group_persistence_attempts.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(in crate::runtime) fn record_speed_group_persistence_attempt(
        &self,
    ) -> Result<(), ApiError> {
        self.speed_group_persistence_attempts
            .fetch_add(1, Ordering::SeqCst);
        if self.speed_group_persistence_failure.load(Ordering::SeqCst) {
            Err(ApiError::internal("forced Speed Group persistence failure"))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(in crate::runtime) fn force_speed_group_persistence_failure(&self, fail: bool) {
        self.speed_group_persistence_failure
            .store(fail, Ordering::SeqCst);
    }
}
