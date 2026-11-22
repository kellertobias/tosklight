//! Ownership-aware schedule arbitration. All publication goes through the canonical reducer.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use media_domain::playlist::{AutomationConfiguration, AutomationOutput, LayerPreset, Playlist};
use media_domain::schedule::{Occurrence, select_winner};
use media_domain::{
    CatalogSnapshot, CommandKind, CommandSource, MasterControls, OutputId, OutputState, Timestamp,
};
use media_http::control::submit_commands_guarded;
use media_http::wire::AutomationStatus;

use super::journal::Journal;
use super::runner::{PlaylistRunner, RunnerPhase};

struct Activation {
    key: String,
    occurrence: Option<Occurrence>,
    playlist_name: String,
    playlist_id: String,
    runner: PlaylistRunner,
    original: OutputState,
    generation: u64,
}

#[derive(Default)]
struct Slot {
    active: Option<Activation>,
    suspended: bool,
    failed: HashSet<String>,
    error: Option<String>,
}

#[derive(Clone, Copy)]
struct Evaluation<'a> {
    state: &'a crate::SharedState,
    catalog: &'a CatalogSnapshot,
    occurrences: &'a [Occurrence],
    received: &'a HashMap<OutputId, Timestamp>,
    apply_now: &'a HashSet<OutputId>,
    wall: i64,
    now: Timestamp,
    previous: Option<i64>,
    due_since: Option<i64>,
}

type Target = (String, Option<Occurrence>, Playlist);

pub(super) struct Engine {
    configuration: AutomationConfiguration,
    slots: HashMap<OutputId, Slot>,
    journal: Journal,
    revision: u64,
    previous_wall: Option<i64>,
    previous_mono: Option<Timestamp>,
    high_water: Option<i64>,
}

impl Engine {
    pub fn new(journal_path: Option<PathBuf>) -> Self {
        let journal = Journal::new(journal_path);
        let high_water = journal.floor();
        Self {
            configuration: Default::default(),
            slots: HashMap::new(),
            journal,
            revision: 1,
            previous_wall: None,
            previous_mono: None,
            high_water,
        }
    }

    pub fn configure(&mut self, configuration: &AutomationConfiguration) -> bool {
        if &self.configuration == configuration {
            return false;
        }
        self.configuration = configuration.clone();
        self.revision = (self.revision + 1).min(9_007_199_254_740_991);
        for slot in self.slots.values_mut() {
            slot.failed.clear();
            slot.error = None;
        }
        true
    }

    pub fn tick(
        &mut self,
        state: &crate::SharedState,
        catalog: &CatalogSnapshot,
        occurrences: &[Occurrence],
        received: &HashMap<OutputId, Timestamp>,
        apply_now: &HashSet<OutputId>,
        wall: i64,
        now: Timestamp,
    ) -> Vec<AutomationStatus> {
        let previous = self.previous_wall;
        let due_since = previous
            .zip(self.previous_mono)
            .and_then(|(previous, mono)| {
                let wall_delta = wall.saturating_sub(previous);
                let mono_delta = now.since(mono).as_millis() as i64;
                (wall_delta >= 0
                    && wall_delta <= 1000
                    && (wall_delta - mono_delta).abs() < 1000
                    && self.high_water.is_none_or(|high| wall >= high))
                .then_some(previous.max(self.journal.floor().unwrap_or(i64::MIN)))
            });
        self.previous_wall = Some(wall);
        self.previous_mono = Some(now);
        self.high_water = Some(self.high_water.map_or(wall, |high| high.max(wall)));
        let snapshot = state.load_full();
        let ids: HashSet<_> = snapshot.outputs.iter().map(|output| output.id).collect();
        self.slots.retain(|id, _| ids.contains(id));
        let evaluation = Evaluation {
            state,
            catalog,
            occurrences,
            received,
            apply_now,
            wall,
            now,
            previous,
            due_since,
        };
        snapshot
            .outputs
            .iter()
            .map(|output| self.tick_output(output, evaluation))
            .collect()
    }

    fn tick_output(
        &mut self,
        output: &OutputState,
        evaluation: Evaluation<'_>,
    ) -> AutomationStatus {
        let Evaluation {
            state,
            occurrences,
            received,
            apply_now,
            wall,
            now,
            previous,
            ..
        } = evaluation;
        let setting = self
            .configuration
            .outputs
            .iter()
            .find(|setting| setting.output_id == output.id)
            .cloned()
            .unwrap_or(AutomationOutput {
                output_id: output.id,
                enabled: false,
                fallback_playlist_id: None,
                dmx_silence_seconds: 10,
            });
        let mut slot = self.slots.remove(&output.id).unwrap_or_default();
        if apply_now.contains(&output.id) {
            slot.failed.clear();
            slot.error = None;
        }
        let packet = output
            .ownership
            .dmx
            .map(|activity| activity.last_seen)
            .into_iter()
            .chain(received.get(&output.id).copied())
            .max();
        let age =
            packet.map(|packet| now.since(packet).as_millis().min(u128::from(u64::MAX)) as u64);
        let timeout = setting.dmx_silence_seconds.saturating_mul(1000);
        let dmx_active = age.is_some_and(|age| age < timeout);
        let reason = if !setting.enabled {
            Some("disabled")
        } else if output.ownership.web_takeover {
            Some("manual")
        } else if dmx_active {
            Some("dmx")
        } else {
            None
        };
        // Record a missed span even when a long stall or a forward clock adjustment makes
        // that span ineligible for playback. A later backward adjustment must not replay it.
        let crossed: Vec<_> = occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.output_id == output.id
                    && occurrence.end_millis.is_none()
                    && previous.is_some_and(|previous| {
                        previous < occurrence.start_millis && occurrence.start_millis <= wall
                    })
            })
            .cloned()
            .collect();

        if let Some(reason) = reason {
            if let Some(active) = slot.active.take() {
                self.record_end(&active, "interrupted");
                self.release(state, &active, now);
            }
            self.journal.consume(
                crossed.into_iter().map(|occurrence| {
                    (occurrence.key, "suppressed".into(), occurrence.start_millis)
                }),
            );
            slot.suspended = true;
            let mut status = self.base_status(output.id, reason, &slot, age);
            status.resume_in_millis =
                (reason == "dmx").then(|| timeout.saturating_sub(age.unwrap_or_default()));
            self.slots.insert(output.id, slot);
            return status;
        }

        let target = self.select_target(
            output.id,
            &setting,
            &mut slot,
            occurrences,
            wall,
            evaluation.due_since,
            crossed,
        );
        self.drive(output, &setting, &mut slot, target, evaluation);
        let status = self.project_status(output.id, &slot, age, now);
        self.slots.insert(output.id, slot);
        status
    }

    fn select_target(
        &mut self,
        output_id: OutputId,
        setting: &AutomationOutput,
        slot: &mut Slot,
        occurrences: &[Occurrence],
        wall: i64,
        due_since: Option<i64>,
        crossed: Vec<Occurrence>,
    ) -> Option<Target> {
        let current_once = slot
            .active
            .as_ref()
            .and_then(|active| active.occurrence.as_ref())
            .filter(|occurrence| occurrence.end_millis.is_none())
            .map(|occurrence| occurrence.key.as_str());
        let eligible_since = if slot.suspended || self.journal.error.is_some() {
            None
        } else {
            due_since
        };
        let scheduled = select_winner(
            occurrences,
            &self.configuration.playlists,
            output_id,
            wall,
            eligible_since,
            &self.journal.consumed,
            current_once,
        )
        .cloned();
        slot.suspended = false;
        let selected_key = scheduled.as_ref().map(|occurrence| occurrence.key.as_str());
        let persisted = self.journal.consume(crossed.into_iter().map(|occurrence| {
            let outcome = if selected_key == Some(occurrence.key.as_str()) {
                "started"
            } else {
                "suppressed"
            };
            (occurrence.key, outcome.into(), occurrence.start_millis)
        }));
        let scheduled = scheduled.filter(|occurrence| occurrence.end_millis.is_some() || persisted);
        let target = scheduled
            .as_ref()
            .and_then(|occurrence| {
                self.configuration
                    .playlists
                    .iter()
                    .find(|playlist| playlist.id == occurrence.playlist_id)
            })
            .map(|playlist| {
                (
                    scheduled.as_ref().unwrap().key.clone(),
                    Some(scheduled.as_ref().unwrap().clone()),
                    playlist.clone(),
                )
            })
            .filter(|(key, _, _)| !slot.failed.contains(key));
        target.or_else(|| {
            setting.fallback_playlist_id.and_then(|id| {
                let playlist = self.configuration.playlists.iter().find(|playlist| {
                    playlist.id == id && playlist.enabled && !playlist.entries.is_empty()
                })?;
                let key = format!("fallback:{id}");
                (!slot.failed.contains(&key)).then(|| (key, None, playlist.clone()))
            })
        })
    }

    fn drive(
        &mut self,
        output: &OutputState,
        setting: &AutomationOutput,
        slot: &mut Slot,
        target: Option<Target>,
        evaluation: Evaluation<'_>,
    ) {
        let Evaluation {
            state,
            catalog,
            apply_now,
            now,
            ..
        } = evaluation;
        let same = target
            .as_ref()
            .zip(slot.active.as_ref())
            .is_some_and(|((key, _, _), active)| key == &active.key)
            && !apply_now.contains(&output.id);
        if same {
            let active = slot.active.as_mut().expect("checked above");
            let update = active.runner.tick(output, catalog, now);
            match submit_commands_guarded(
                state,
                &update.commands,
                CommandSource::Scheduler,
                now,
                Some((output.id, active.generation)),
            ) {
                Ok(()) => {
                    if matches!(
                        update.status.phase,
                        RunnerPhase::Completed | RunnerPhase::Failed
                    ) {
                        let active = slot.active.take().expect("checked above");
                        self.record_end(
                            &active,
                            if update.status.phase == RunnerPhase::Completed {
                                "completed"
                            } else {
                                "failed"
                            },
                        );
                        if update.status.phase == RunnerPhase::Failed {
                            slot.failed.insert(active.key.clone());
                            slot.error = update.status.error;
                        }
                        self.release(state, &active, now);
                    }
                }
                Err(_) => {
                    let active = slot.active.take().expect("checked above");
                    self.record_end(&active, "interrupted");
                    self.release(state, &active, now);
                    slot.suspended = true;
                }
            }
        } else if let Some((key, occurrence, playlist)) = target {
            let failed_once = occurrence
                .as_ref()
                .filter(|occurrence| occurrence.end_millis.is_none())
                .map(|occurrence| {
                    (
                        occurrence.key.clone(),
                        "failed".to_owned(),
                        occurrence.start_millis,
                    )
                });
            let previous = slot.active.take();
            if let Some(previous) = &previous {
                self.record_end(previous, "interrupted");
            }
            match self.activate(
                state,
                output,
                catalog,
                now,
                &setting,
                key.clone(),
                occurrence,
                &playlist,
                previous.as_ref(),
            ) {
                Ok(active) => slot.active = Some(active),
                Err(error) => {
                    if let Some(outcome) = failed_once {
                        self.journal.consume([outcome]);
                    }
                    if let Some(previous) = &previous {
                        self.release(state, previous, now);
                    }
                    slot.failed.insert(key);
                    slot.error = Some(error);
                }
            }
        } else if let Some(active) = slot.active.take() {
            self.record_end(&active, "interrupted");
            self.release(state, &active, now);
        }
    }

    fn project_status(
        &self,
        output_id: OutputId,
        slot: &Slot,
        age: Option<u64>,
        now: Timestamp,
    ) -> AutomationStatus {
        let owner = if slot
            .active
            .as_ref()
            .is_some_and(|active| active.occurrence.is_some())
        {
            "schedule"
        } else if slot.active.is_some() {
            "fallback"
        } else {
            "idle"
        };
        let mut status = self.base_status(output_id, owner, slot, age);
        if let Some(active) = &slot.active {
            let running = active.runner.status(now);
            status.playlist_id = Some(active.playlist_id.clone());
            status.event_id = active
                .occurrence
                .as_ref()
                .map(|occurrence| occurrence.event_id.to_string());
            status.playlist_name = Some(active.playlist_name.clone());
            status.entry_index = running.entry_index;
            status.play = running.play;
            status.phase = match running.phase {
                RunnerPhase::Loading => "loading",
                RunnerPhase::Playing => "playing",
                RunnerPhase::Fading => "fading",
                RunnerPhase::Completed => "completed",
                RunnerPhase::Failed => "failed",
            }
            .into();
            status.error = running.error.or(status.error);
        }
        status
    }

    fn base_status(
        &self,
        output: OutputId,
        owner: &str,
        slot: &Slot,
        age: Option<u64>,
    ) -> AutomationStatus {
        let mut status = base_status(output, owner, slot, &self.journal, age);
        let ids = self
            .configuration
            .events
            .iter()
            .filter(|event| event.output_id == output)
            .map(|event| event.id.to_string())
            .collect();
        if let Some((event_id, outcome)) = self.journal.latest_for_events(&ids) {
            status.last_occurrence_outcome = Some(outcome.to_owned());
            status.last_occurrence_name = self
                .configuration
                .events
                .iter()
                .find(|event| event.id.to_string() == event_id)
                .map(|event| event.name.clone());
        }
        status
    }

    fn activate(
        &mut self,
        state: &crate::SharedState,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        setting: &AutomationOutput,
        key: String,
        occurrence: Option<Occurrence>,
        playlist: &Playlist,
        previous: Option<&Activation>,
    ) -> Result<Activation, String> {
        let looping = occurrence
            .as_ref()
            .is_none_or(|occurrence| occurrence.end_millis.is_some());
        let (runner, update) = PlaylistRunner::activate(playlist, output, catalog, now, looping)?;
        let mut original = output.clone();
        let mut commands = Vec::new();
        if let Some(previous) = previous.filter(|previous| owns(output, previous)) {
            commands.extend(restore(previous));
            for layer in previous.runner.owned_layers() {
                if let (Some(target), Some(saved)) = (
                    original.layers.get_mut(layer),
                    previous.original.layers.get(layer),
                ) {
                    *target = saved.clone();
                }
            }
            original.master = previous.original.master;
        }
        commands.push(CommandKind::SetSchedulerOwnership {
            output: output.id,
            active: true,
            dmx_silence_millis: setting.dmx_silence_seconds.saturating_mul(1000),
        });
        commands.extend(update.commands);
        submit_commands_guarded(
            state,
            &commands,
            CommandSource::Scheduler,
            now,
            Some((output.id, output.ownership.generation)),
        )
        .map_err(|_| "Playback ownership changed before the playlist could start".to_owned())?;
        let generation = if output.ownership.scheduler_active {
            output.ownership.generation
        } else {
            output.ownership.generation.wrapping_add(1)
        };
        Ok(Activation {
            key,
            occurrence,
            playlist_name: playlist.name.clone(),
            playlist_id: playlist.id.to_string(),
            runner,
            original,
            generation,
        })
    }

    fn record_end(&mut self, active: &Activation, outcome: &str) {
        if let Some(occurrence) = active
            .occurrence
            .as_ref()
            .filter(|occurrence| occurrence.end_millis.is_none())
        {
            self.journal.consume([(
                occurrence.key.clone(),
                outcome.into(),
                occurrence.start_millis,
            )]);
        }
    }

    fn release(&self, state: &crate::SharedState, active: &Activation, now: Timestamp) {
        let snapshot = state.load_full();
        let Some(output) = snapshot
            .output(active.original.id)
            .filter(|output| owns(output, active))
        else {
            return;
        };
        let mut commands = restore(active);
        // A personality change can remove previously owned layer indices. Restore only the
        // surviving layers and still release ownership in one accepted batch.
        commands.retain(|command| match command {
            CommandKind::SetLayerControls { layer, .. } => *layer < output.layers.len(),
            _ => true,
        });
        commands.push(CommandKind::SetSchedulerOwnership {
            output: output.id,
            active: false,
            dmx_silence_millis: output.ownership.scheduler_dmx_silence_millis,
        });
        let _ = submit_commands_guarded(
            state,
            &commands,
            CommandSource::Scheduler,
            now,
            Some((output.id, active.generation)),
        );
    }

    pub fn release_all(&mut self, state: &crate::SharedState, now: Timestamp) {
        for (_, mut slot) in std::mem::take(&mut self.slots) {
            if let Some(active) = slot.active.take() {
                self.record_end(&active, "interrupted");
                self.release(state, &active, now);
            }
        }
    }
}

fn owns(output: &OutputState, active: &Activation) -> bool {
    output.ownership.scheduler_active && output.ownership.generation == active.generation
}

fn restore(active: &Activation) -> Vec<CommandKind> {
    let mut commands: Vec<_> = active
        .runner
        .owned_layers()
        .into_iter()
        .filter_map(|layer| {
            let saved = active.original.layers.get(layer)?;
            Some(CommandKind::SetLayerControls {
                output: active.original.id,
                layer,
                controls: Box::new(LayerPreset::from(saved).controls(saved.address)),
            })
        })
        .collect();
    let master = active.original.master;
    commands.push(CommandKind::SetMasterControls {
        output: active.original.id,
        controls: Box::new(MasterControls {
            dimmer: Some(master.dimmer),
            volume: Some(master.volume),
            tint: Some(master.tint),
            flip_mirror: Some(master.flip_mirror),
            mask: Some(master.mask),
            mask_position_x: Some(master.mask_position_x),
            mask_position_y: Some(master.mask_position_y),
            scale_x: Some(master.scale_x),
            scale_y: Some(master.scale_y),
            scaling_mode: Some(master.scaling_mode),
            position_x: Some(master.position_x),
            position_y: Some(master.position_y),
            rotation: Some(master.rotation),
            shaper: Some(master.shaper),
            opacity_cycle: Some(master.opacity_cycle),
        }),
    });
    commands
}

fn base_status(
    output: OutputId,
    owner: &str,
    slot: &Slot,
    journal: &Journal,
    dmx_age_millis: Option<u64>,
) -> AutomationStatus {
    AutomationStatus {
        output_id: output.to_string(),
        owner: if owner == "idle" && (slot.error.is_some() || journal.error.is_some()) {
            "error"
        } else {
            owner
        }
        .into(),
        phase: if slot.error.is_some() {
            "failed"
        } else {
            owner
        }
        .into(),
        error: slot.error.clone().or_else(|| journal.error.clone()),
        dmx_age_millis,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "orchestration_tests.rs"]
mod tests;
