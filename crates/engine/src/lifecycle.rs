use crate::{Engine, EngineError, EngineSnapshot, value_for_ordered_position};
use light_playback::PlaybackEngine;
use light_programmer::{GroupDefinition, resolve_group};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

impl Engine {
    pub fn replace_snapshot(&self, snapshot: EngineSnapshot) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, true)
    }

    /// Validates every runtime-dependent part of a candidate snapshot without mutating the live
    /// engine. Server persistence uses this preflight so an invalid Chaser or playback assignment
    /// cannot be written first and rejected only during the subsequent live-engine refresh.
    pub fn validate_snapshot_for_runtime(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(), EngineError> {
        snapshot.validate()?;
        self.compile_playback(snapshot).map(|_| ())
    }

    pub fn replace_snapshot_releasing_playback(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<(), EngineError> {
        self.replace_snapshot_with_playback_policy(snapshot, false)
    }
    fn replace_snapshot_with_playback_policy(
        &self,
        snapshot: EngineSnapshot,
        preserve_playback: bool,
    ) -> Result<(), EngineError> {
        snapshot.validate()?;
        let (active_playbacks, dynamics_paused_at) = if preserve_playback {
            let playback = self.playback.read();
            (
                playback.active_for_snapshot(&snapshot.cue_lists, self.clock.now()),
                playback.dynamics_paused_since(),
            )
        } else {
            (Vec::new(), None)
        };
        let (mut playback, groups) = self.compile_playback(&snapshot)?;
        self.programmers.refresh_live_selections(&groups);
        playback.restore_active(active_playbacks);
        playback.restore_dynamics_paused_since(dynamics_paused_at);
        *self.playback.write() = playback;
        self.snapshot.store(Arc::new(snapshot));
        Ok(())
    }

    fn compile_playback(
        &self,
        snapshot: &EngineSnapshot,
    ) -> Result<(PlaybackEngine, HashMap<String, GroupDefinition>), EngineError> {
        let mut playback = PlaybackEngine::with_clock(Arc::clone(&self.clock));
        playback.set_control_timing(
            self.speed_groups_bpm
                .each_ref()
                .map(|bpm| f64::from_bits(bpm.load(Ordering::Relaxed))),
            self.sequence_master_fade_millis.load(Ordering::Relaxed),
        );
        playback.set_speed_groups_paused(
            self.speed_groups_paused
                .each_ref()
                .map(|paused| paused.load(Ordering::Relaxed)),
        );
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        for source in &snapshot.cue_lists {
            let mut cue_list = source.clone();
            for cue in &mut cue_list.cues {
                let mut expanded_addresses = cue
                    .changes
                    .iter()
                    .map(|change| (change.fixture_id, change.attribute.clone()))
                    .collect::<std::collections::HashSet<_>>();
                for change in cue.group_changes.clone() {
                    if let Ok(fixtures) = resolve_group(&change.group_id, &groups) {
                        let count = fixtures.len();
                        for (index, fixture_id) in fixtures.into_iter().enumerate() {
                            if expanded_addresses.insert((fixture_id, change.attribute.clone())) {
                                cue.changes.push(light_playback::CueChange {
                                    fixture_id,
                                    attribute: change.attribute.clone(),
                                    value: change.value.as_ref().map(|value| {
                                        value_for_ordered_position(value, index, count)
                                    }),
                                    automatic_restore: false,
                                    fade_millis: change.fade_millis,
                                    delay_millis: change.delay_millis,
                                });
                            }
                        }
                    }
                }
                for phaser in &mut cue.phasers {
                    for group_id in &phaser.group_ids {
                        if let Ok(fixtures) = resolve_group(group_id, &groups) {
                            for fixture in fixtures {
                                if !phaser.fixture_ids.contains(&fixture) {
                                    phaser.fixture_ids.push(fixture);
                                }
                            }
                        }
                    }
                }
            }
            playback.register(cue_list).map_err(EngineError::Invalid)?;
        }
        for definition in snapshot.playbacks.clone() {
            playback
                .register_definition(definition)
                .map_err(EngineError::Invalid)?;
        }
        Ok((playback, groups))
    }

    pub fn snapshot(&self) -> Arc<EngineSnapshot> {
        self.snapshot.load_full()
    }
    pub fn playback(&self) -> &RwLock<PlaybackEngine> {
        &self.playback
    }
    pub fn set_timecode_frame(&self, frame: Option<u64>) {
        self.timecode_frame
            .store(frame.unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}
