use super::*;

pub(super) fn draw_and_observe(
    state: &mut PaneState,
    epoch: Instant,
    source: &mut HelperSource,
    heartbeat_clock: &mut BenchmarkHeartbeatClock,
) {
    match state.draw(epoch, source) {
        Ok(()) => {}
        // A frame that failed is not fatal on its own: a surface can be lost while a display
        // is reconfigured and be there again next frame. The desk is told, and the loop keeps
        // going rather than taking the pane away over one bad frame.
        Err(detail) => {
            if let Some(benchmark) = state.benchmark.as_mut() {
                benchmark.failed();
            }
            source.report(&detail);
        }
    }
    // Observation follows the existing draw attempt, including an unchanged static skip.
    // It never requests a redraw or changes the provider, picture, or source frame.
    if let Some(sequence) = heartbeat_clock.next(epoch.elapsed()) {
        if let (Some(benchmark), Some(epoch_micros)) =
            (state.benchmark.as_ref(), benchmark_epoch_micros())
        {
            source.send_benchmark_heartbeat(viz_helper::protocol::FromHelper::BenchmarkHeartbeat {
                epoch_micros,
                sequence,
                last_presented_sequence: state.presented_frames,
                source_frame: state.values.frame,
                follow_preload: state.following_preload,
                width: state.size.0,
                height: state.size.1,
                idle_unchanged: benchmark.idle_unchanged,
                draw_healthy: benchmark.draw_healthy,
            });
        }
    }
}

/// Diagnostic state only. A redraw gate can remember an attempted picture even if rendering
/// fails, so that gate alone cannot establish a successful static picture.
#[derive(Default)]
pub(super) struct BenchmarkDrawWitness {
    rig_available: bool,
    connected: bool,
    snapshot_seen: bool,
    values_seen: bool,
    pub(super) last_success: Option<crate::redraw::RedrawState>,
    pub(super) follow_preload: bool,
    pub(super) idle_unchanged: bool,
    pub(super) draw_healthy: bool,
}

impl BenchmarkDrawWitness {
    pub(super) fn new(rig_available: bool) -> Self {
        Self {
            rig_available,
            ..Default::default()
        }
    }

    pub(super) fn ready(&self) -> bool {
        self.rig_available && self.connected && self.snapshot_seen && self.values_seen
    }

    pub(super) fn observe(&mut self, event: &ProviderEvent) {
        match event {
            ProviderEvent::Snapshot { .. } | ProviderEvent::SceneDelta(_) => {
                self.snapshot_seen = true;
                self.values_seen = false;
                self.failed();
            }
            ProviderEvent::Values(_) => self.values_seen = true,
            ProviderEvent::Connection(connection) => {
                self.connected = matches!(connection, viz_scene::ConnectionState::Connected { .. });
                if !self.connected {
                    self.failed();
                }
            }
            _ => {}
        }
    }

    fn failed(&mut self) {
        self.last_success = None;
        self.idle_unchanged = false;
        self.draw_healthy = false;
    }

    pub(super) fn skipped(&mut self, same_successful_picture: bool, presented_frames: u64) {
        self.idle_unchanged = self.ready()
            && same_successful_picture
            && self.last_success.is_some()
            && presented_frames > 0;
        self.draw_healthy = self.idle_unchanged;
    }
}

pub(super) struct BenchmarkHeartbeatClock {
    enabled: bool,
    last: Option<Duration>,
    sequence: u64,
}

impl BenchmarkHeartbeatClock {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last: None,
            sequence: 0,
        }
    }

    fn next(&mut self, now: Duration) -> Option<u64> {
        if !self.enabled
            || self
                .last
                .is_some_and(|last| now.saturating_sub(last) < Duration::from_millis(500))
        {
            return None;
        }
        self.sequence = self.sequence.checked_add(1)?;
        self.last = Some(now);
        Some(self.sequence)
    }
}

fn benchmark_epoch_micros() -> Option<u64> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_micros(),
    )
    .ok()
}

#[cfg(test)]
mod passive_benchmark_tests {
    use super::*;

    fn picture() -> crate::redraw::RedrawState {
        crate::redraw::RedrawState::new(
            1,
            0,
            &SceneValues::default(),
            &ViewConfiguration::default(),
            (800, 600),
            &[],
        )
    }

    #[test]
    fn ordinary_loop_emits_nothing_and_static_loop_is_bounded_without_new_frames() {
        let mut ordinary = BenchmarkHeartbeatClock::new(false);
        let mut clock = BenchmarkHeartbeatClock::new(true);
        let mut witness = BenchmarkDrawWitness {
            rig_available: true,
            connected: true,
            snapshot_seen: true,
            values_seen: true,
            last_success: Some(picture()),
            ..Default::default()
        };
        let last_presented_sequence = 1;
        for (millis, expected) in [
            (0, Some(1)),
            (2, None),
            (499, None),
            (500, Some(2)),
            (999, None),
            (1000, Some(3)),
        ] {
            assert_eq!(ordinary.next(Duration::from_millis(millis)), None);
            witness.skipped(true, last_presented_sequence);
            assert!(witness.idle_unchanged && witness.draw_healthy);
            assert_eq!(clock.next(Duration::from_millis(millis)), expected);
            assert_eq!(last_presented_sequence, 1);
        }
    }

    #[test]
    fn no_rig_or_missing_provider_readiness_never_establishes_idle() {
        let mut witness = BenchmarkDrawWitness {
            last_success: Some(picture()),
            ..Default::default()
        };
        witness.skipped(true, 1);
        assert!(!witness.draw_healthy && !witness.idle_unchanged);
        witness.rig_available = true;
        witness.snapshot_seen = true;
        witness.values_seen = true;
        witness.connected = true;
        witness.skipped(true, 1);
        assert!(witness.idle_unchanged);
        witness.observe(&ProviderEvent::Connection(viz_scene::ConnectionState::Idle));
        witness.skipped(true, 1);
        assert!(!witness.draw_healthy && !witness.idle_unchanged);
    }

    #[test]
    fn failed_draw_or_changed_picture_cannot_become_idle_on_a_later_gate_skip() {
        let mut witness = BenchmarkDrawWitness {
            rig_available: true,
            connected: true,
            snapshot_seen: true,
            values_seen: true,
            last_success: Some(picture()),
            ..Default::default()
        };
        witness.skipped(false, 1);
        assert!(!witness.idle_unchanged && !witness.draw_healthy);
        witness.failed();
        witness.skipped(true, 1);
        assert!(!witness.idle_unchanged && !witness.draw_healthy);
        witness.last_success = Some(picture());
        witness.skipped(true, 0);
        assert!(!witness.idle_unchanged);
    }
}
