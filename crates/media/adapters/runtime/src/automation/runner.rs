//! Playlist transport expressed entirely through the ordinary layer commands.
//!
//! The caller owns arbitration and submits each returned command batch atomically. This runner
//! never writes shared state or reads a clock, and cannot take control from DMX or an operator.

use std::collections::HashSet;
use std::time::Duration;

use media_domain::playlist::{AuxiliaryMode, LayerPreset, Playlist};
use media_domain::{
    CatalogItem, CatalogSnapshot, CommandKind, ItemKind, LayerControls, MasterControls,
    MediaAddress, OnceEndState, OutputId, OutputState, PlayMode, SourceStatus, Timestamp,
};
use serde::Serialize;

const LOAD_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ISSUES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RunnerPhase {
    Loading,
    Playing,
    Fading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunnerStatus {
    pub phase: RunnerPhase,
    pub entry_id: Option<String>,
    /// Zero-based playlist position; the web interface adds one for display.
    pub entry_index: Option<usize>,
    pub play: u32,
    /// The operator's one-based layer number.
    pub layer: Option<usize>,
    pub transition_progress: f32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RunnerUpdate {
    pub commands: Vec<CommandKind>,
    pub status: RunnerStatus,
}

#[derive(Debug, Clone)]
struct Active {
    index: usize,
    layer: usize,
    play: u32,
    started: Timestamp,
    ready: bool,
}

#[derive(Debug, Clone)]
struct Pending {
    index: usize,
    layer: usize,
    address: MediaAddress,
    reset_trigger: u32,
    requested: Timestamp,
}

#[derive(Debug, Clone)]
struct Transition {
    outgoing_layer: usize,
    started: Timestamp,
    duration: Duration,
}

pub(crate) struct PlaylistRunner {
    playlist: Playlist,
    output: OutputId,
    pair: [usize; 2],
    looping: bool,
    active: Option<Active>,
    pending: Option<Pending>,
    transition: Option<Transition>,
    terminal: Option<RunnerPhase>,
    excluded: HashSet<usize>,
    issues: Vec<String>,
    next_start: usize,
    auxiliary_failures: HashSet<usize>,
}

impl PlaylistRunner {
    pub(crate) fn activate(
        playlist: &Playlist,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        looping: bool,
    ) -> Result<(Self, RunnerUpdate), String> {
        if !playlist.enabled || playlist.entries.is_empty() {
            return Err("The playlist is disabled or has no entries".into());
        }
        let [first, second] = playlist.layers;
        if first == 0
            || second == 0
            || first == second
            || first > output.layers.len()
            || second > output.layers.len()
        {
            return Err("Select two distinct playback layers available on this output".into());
        }
        let mut owned = HashSet::from([first, second]);
        for auxiliary in &playlist.auxiliary_layers {
            if auxiliary.layer == 0
                || auxiliary.layer > output.layers.len()
                || !owned.insert(auxiliary.layer)
            {
                return Err(
                    "Auxiliary layers must be available, distinct and outside the playback pair"
                        .into(),
                );
            }
        }
        let mut runner = Self {
            playlist: playlist.clone(),
            output: output.id,
            pair: [first - 1, second - 1],
            looping,
            active: None,
            pending: None,
            transition: None,
            terminal: None,
            excluded: HashSet::new(),
            issues: Vec::new(),
            next_start: 0,
            auxiliary_failures: HashSet::new(),
        };
        let mut commands = Vec::new();
        // Whole-playlist handovers are cuts. No content inherited on the selected pair leaks
        // through while the first clip loads.
        for layer in runner.pair {
            commands.push(runner.clear(layer));
        }
        commands.push(CommandKind::SetMasterControls {
            output: output.id,
            controls: Box::new(MasterControls {
                dimmer: Some(playlist.master_dimmer),
                volume: Some(playlist.master_volume),
                ..Default::default()
            }),
        });
        for auxiliary in &playlist.auxiliary_layers {
            let layer = auxiliary.layer - 1;
            match auxiliary.mode {
                AuxiliaryMode::Unchanged => {}
                AuxiliaryMode::Off => commands.push(runner.clear(layer)),
                AuxiliaryMode::Fixed => {
                    let resolved = auxiliary.asset_id.and_then(|id| {
                        let (_, item) = catalog.item(id)?;
                        item.enabled.then_some(())?;
                        catalog.address_of(id)
                    });
                    if let Some(address) = resolved {
                        commands.push(CommandKind::SetLayerControls {
                            output: output.id,
                            layer,
                            controls: Box::new(auxiliary.controls.controls(address)),
                        });
                        commands.push(CommandKind::ResetLayer {
                            output: output.id,
                            layer,
                        });
                        commands.push(CommandKind::ReportSourceStatus {
                            output: output.id,
                            layer,
                            status: SourceStatus::Loading,
                        });
                    } else {
                        commands.push(runner.clear(layer));
                        runner.issue(format!(
                            "Fixed content on layer {} is missing, parked or disabled",
                            auxiliary.layer
                        ));
                    }
                }
            }
        }
        if let Some(index) = runner.find_playable(0, false, catalog) {
            runner.prepare(index, runner.pair[0], output, catalog, now, &mut commands);
        } else {
            return Err(runner
                .last_issue()
                .unwrap_or("The playlist has no playable entries")
                .to_owned());
        }
        let update = RunnerUpdate {
            commands,
            status: runner.status(now),
        };
        Ok((runner, update))
    }

    pub(crate) fn tick(
        &mut self,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
    ) -> RunnerUpdate {
        let mut commands = Vec::new();
        if output.id != self.output || self.pair.iter().any(|&layer| layer >= output.layers.len()) {
            self.fail(
                "The output or its playback layer count changed",
                &mut commands,
            );
        } else if self.terminal.is_none() {
            self.reconcile_auxiliary(output, catalog, &mut commands);
            self.advance(output, catalog, now, &mut commands);
        }
        RunnerUpdate {
            commands,
            status: self.status(now),
        }
    }

    /// The caller must check scheduler ownership before submitting cleanup or restoring state.
    pub(crate) fn stop(&self) -> Vec<CommandKind> {
        self.owned_layers()
            .into_iter()
            .map(|layer| self.clear(layer))
            .collect()
    }

    /// Zero-based indices of layers this activation may change.
    pub(crate) fn owned_layers(&self) -> Vec<usize> {
        let mut layers = self.pair.to_vec();
        layers.extend(
            self.playlist
                .auxiliary_layers
                .iter()
                .filter_map(|auxiliary| {
                    (auxiliary.mode != AuxiliaryMode::Unchanged).then_some(auxiliary.layer - 1)
                }),
        );
        layers
    }

    fn reconcile_auxiliary(
        &mut self,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        commands: &mut Vec<CommandKind>,
    ) {
        for auxiliary in self.playlist.auxiliary_layers.clone() {
            if auxiliary.mode != AuxiliaryMode::Fixed {
                continue;
            }
            let layer = auxiliary.layer - 1;
            let Some(current) = output.layers.get(layer) else {
                continue;
            };
            let resolved = auxiliary.asset_id.and_then(|id| {
                let (_, item) = catalog.item(id)?;
                item.enabled.then_some(())?;
                catalog.address_of(id)
            });
            if let Some(address) = resolved {
                if address != current.address {
                    self.auxiliary_failures.remove(&layer);
                    commands.push(CommandKind::SetLayerControls {
                        output: self.output,
                        layer,
                        controls: Box::new(auxiliary.controls.controls(address)),
                    });
                    commands.push(CommandKind::ResetLayer {
                        output: self.output,
                        layer,
                    });
                    commands.push(CommandKind::ReportSourceStatus {
                        output: self.output,
                        layer,
                        status: SourceStatus::Loading,
                    });
                } else if current.source_status.is_failed() && self.auxiliary_failures.insert(layer)
                {
                    self.issue(format!(
                        "Fixed content on layer {} failed to load or decode",
                        auxiliary.layer
                    ));
                }
            } else if !current.address.is_blank() {
                commands.push(self.clear(layer));
                self.issue(format!(
                    "Fixed content on layer {} is missing, parked or disabled",
                    auxiliary.layer
                ));
            }
        }
    }

    fn advance(
        &mut self,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        commands: &mut Vec<CommandKind>,
    ) {
        if let Some(pending) = self.pending.clone() {
            if self
                .resolve(pending.index, catalog)
                .is_ok_and(|(address, _)| address != pending.address)
            {
                self.prepare(pending.index, pending.layer, output, catalog, now, commands);
            }
        }
        self.check_pending(output, catalog, now);
        if self.active.is_none() {
            self.start_first(output, catalog, now, commands);
            return;
        }
        if self.transition.is_some() {
            if let Some(active) = &mut self.active {
                active.ready |= matches!(
                    output.layers[active.layer].source_status,
                    SourceStatus::Ready | SourceStatus::Completed
                );
            }
            self.advance_transition(now, commands);
            return;
        }

        let mut active = self.active.take().expect("checked above");
        let item = self.resolve(active.index, catalog).map(|(_, item)| item);
        if item.is_err()
            || output.layers[active.layer].source_status.is_failed()
            || !active.ready
                && !matches!(
                    output.layers[active.layer].source_status,
                    SourceStatus::Ready | SourceStatus::Completed
                )
                && now.since(active.started) >= LOAD_TIMEOUT
        {
            let reason = item
                .err()
                .unwrap_or_else(|| "The active clip failed to load or decode".into());
            self.issue(reason);
            self.excluded.insert(active.index);
            self.next_start = active.index + 1;
            commands.push(self.clear(active.layer));
            if self.pending.is_none() {
                self.prepare_after(active.index, active.layer, output, catalog, now, commands);
            }
            self.start_first(output, catalog, now, commands);
            return;
        }
        let item = item.expect("error handled above");
        active.ready |= matches!(
            output.layers[active.layer].source_status,
            SourceStatus::Ready | SourceStatus::Completed
        );

        // Follow identity if the operator moves/reindexes a running clip. This is a deliberate
        // restart of that play rather than allowing its former slot to select another file.
        if catalog.address_of(item.id) != Some(output.layers[active.layer].address) {
            self.issue("The active asset moved; restarting it at its current address".into());
            self.prepare(active.index, active.layer, output, catalog, now, commands);
            self.start_first(output, catalog, now, commands);
            return;
        }

        let entry = &self.playlist.entries[active.index];
        let duration = match item.kind {
            ItemKind::Image => entry.hold_millis,
            ItemKind::Video => item.duration_millis,
        };
        let completed = active.ready
            && match item.kind {
                ItemKind::Image => duration.is_some_and(|millis| {
                    now.since(active.started) >= Duration::from_millis(millis)
                }),
                ItemKind::Video => {
                    output.layers[active.layer].source_status == SourceStatus::Completed
                }
            };
        if active.play < entry.plays {
            if completed {
                active.play += 1;
                active.started = now;
                active.ready = false;
                self.restart(active.layer, commands);
            }
            self.active = Some(active);
            return;
        }

        // An unfaded video advances only on the transport's terminal report. Duration metadata
        // is used solely to lead a nonzero dissolve, never to truncate an ordinary video pass.
        let predicted_duration = if item.kind == ItemKind::Image
            || self.playlist.entries[active.index].fade_millis > 0
        {
            duration
        } else {
            None
        };
        self.finish_play(
            active,
            completed,
            predicted_duration,
            output,
            catalog,
            now,
            commands,
        );
    }

    fn finish_play(
        &mut self,
        active: Active,
        completed: bool,
        duration: Option<u64>,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        commands: &mut Vec<CommandKind>,
    ) {
        if self.pending.is_none() {
            self.prepare_after(
                active.index,
                self.other(active.layer),
                output,
                catalog,
                now,
                commands,
            );
        }
        let fade = Duration::from_millis(self.playlist.entries[active.index].fade_millis);
        let due = completed
            || active.ready
                && duration.is_some_and(|millis| {
                    now.since(active.started) >= Duration::from_millis(millis).saturating_sub(fade)
                });
        if !due {
            self.active = Some(active);
            return;
        }
        if let Some(pending) = self.pending.as_ref() {
            if self.pending_ready(pending, output) {
                let pending = self.pending.take().expect("checked above");
                let incoming_duration =
                    self.resolve(pending.index, catalog)
                        .ok()
                        .and_then(|(_, item)| match item.kind {
                            ItemKind::Image => self.playlist.entries[pending.index].hold_millis,
                            ItemKind::Video => item.duration_millis,
                        });
                if incoming_duration.is_some_and(|millis| fade > Duration::from_millis(millis)) {
                    self.fail(
                        "The transition is longer than the incoming clip's display duration",
                        commands,
                    );
                    return;
                }
                let incoming = Active {
                    index: pending.index,
                    layer: pending.layer,
                    play: 1,
                    started: now,
                    ready: false,
                };
                self.restart(incoming.layer, commands);
                if fade.is_zero() {
                    commands.push(self.level(active.layer, 0.0, 0.0));
                    commands.push(self.level(incoming.layer, 1.0, 1.0));
                    commands.push(self.clear(active.layer));
                } else {
                    self.transition = Some(Transition {
                        outgoing_layer: active.layer,
                        started: now,
                        duration: fade,
                    });
                    let (outgoing, incoming_level) =
                        crossfade_levels(active.layer, incoming.layer, 0.0);
                    commands.push(self.level(active.layer, outgoing, 1.0));
                    commands.push(self.level(incoming.layer, incoming_level, 0.0));
                }
                self.active = Some(incoming);
            } else {
                // A late load holds the current last frame; the existing Once Hold transport
                // supplies it without inventing a second playback timeline.
                self.active = Some(active);
            }
        } else if completed {
            if self.looping {
                self.fail("The playlist has no remaining playable entries", commands);
            } else {
                self.terminal = Some(RunnerPhase::Completed);
                commands.extend(self.stop());
            }
        } else {
            self.active = Some(active);
        }
    }

    fn start_first(
        &mut self,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        commands: &mut Vec<CommandKind>,
    ) {
        if self.pending.is_none() {
            if let Some(index) = self.find_playable(self.next_start, self.looping, catalog) {
                self.prepare(index, self.pair[0], output, catalog, now, commands);
            } else {
                self.fail("The playlist has no playable entries", commands);
                return;
            }
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| self.pending_ready(pending, output))
        {
            let pending = self.pending.take().expect("checked above");
            self.restart(pending.layer, commands);
            commands.push(self.level(pending.layer, 1.0, 1.0));
            self.active = Some(Active {
                index: pending.index,
                layer: pending.layer,
                play: 1,
                started: now,
                ready: false,
            });
        }
    }

    fn check_pending(&mut self, output: &OutputState, catalog: &CatalogSnapshot, now: Timestamp) {
        let Some(pending) = &self.pending else { return };
        let reason = if self.resolve(pending.index, catalog).is_err() {
            Some("The next clip is missing, parked or disabled")
        } else if output.layers[pending.layer].reset_trigger_id == pending.reset_trigger
            && output.layers[pending.layer].address == pending.address
            && output.layers[pending.layer].source_status.is_failed()
        {
            Some("The next clip failed to load or decode")
        } else if !self.pending_ready(pending, output)
            && now.since(pending.requested) >= LOAD_TIMEOUT
        {
            Some("The next clip did not load within 10 seconds")
        } else {
            None
        };
        if let Some(reason) = reason {
            let pending = self.pending.take().expect("checked above");
            self.issue(format!("Entry {}: {reason}", pending.index + 1));
            self.excluded.insert(pending.index);
            self.next_start = pending.index + 1;
        }
    }

    fn prepare_after(
        &mut self,
        current: usize,
        layer: usize,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        commands: &mut Vec<CommandKind>,
    ) {
        if let Some(index) = self.find_playable(current + 1, self.looping, catalog) {
            self.prepare(index, layer, output, catalog, now, commands);
        }
    }

    fn find_playable(
        &mut self,
        start: usize,
        wrap: bool,
        catalog: &CatalogSnapshot,
    ) -> Option<usize> {
        let count = self.playlist.entries.len();
        for offset in 0..count {
            let index = start + offset;
            if !wrap && index >= count {
                break;
            }
            let index = index % count;
            if self.excluded.contains(&index) {
                continue;
            }
            match self.resolve(index, catalog) {
                Ok(_) => return Some(index),
                Err(reason) => {
                    self.issue(format!("Entry {}: {reason}", index + 1));
                    self.excluded.insert(index);
                }
            }
        }
        None
    }

    fn resolve<'a>(
        &self,
        index: usize,
        catalog: &'a CatalogSnapshot,
    ) -> Result<(MediaAddress, &'a CatalogItem), String> {
        let entry = &self.playlist.entries[index];
        if entry.plays == 0 {
            return Err("Plays must be greater than zero".into());
        }
        let (_, item) = catalog
            .item(entry.asset_id)
            .ok_or("The media asset is missing")?;
        if !item.enabled {
            return Err("The media asset is disabled".into());
        }
        let address = catalog
            .address_of(entry.asset_id)
            .ok_or("The media asset is parked")?;
        let duration = match item.kind {
            ItemKind::Image => Some(
                entry
                    .hold_millis
                    .filter(|millis| *millis > 0)
                    .ok_or("A still image requires a positive display duration")?,
            ),
            ItemKind::Video => item.duration_millis,
        };
        if duration.is_some_and(|millis| entry.fade_millis > millis) {
            return Err("The outgoing fade is longer than the clip's display duration".into());
        }
        Ok((address, item))
    }

    fn prepare(
        &mut self,
        index: usize,
        layer: usize,
        output: &OutputState,
        catalog: &CatalogSnapshot,
        now: Timestamp,
        commands: &mut Vec<CommandKind>,
    ) {
        let Ok((address, _)) = self.resolve(index, catalog) else {
            return;
        };
        let controls = LayerPreset {
            play_mode: PlayMode::Stop,
            dimmer: 0.0,
            volume: 0.0,
            ..self.playlist.entries[index].transform.layer_preset()
        }
        .controls(address);
        commands.push(CommandKind::SetLayerControls {
            output: self.output,
            layer,
            controls: Box::new(controls),
        });
        commands.push(CommandKind::ResetLayer {
            output: self.output,
            layer,
        });
        commands.push(CommandKind::ReportSourceStatus {
            output: self.output,
            layer,
            status: SourceStatus::Loading,
        });
        self.pending = Some(Pending {
            index,
            layer,
            address,
            reset_trigger: output.layers[layer].reset_trigger_id.wrapping_add(1),
            requested: now,
        });
        self.next_start = index;
    }

    fn pending_ready(&self, pending: &Pending, output: &OutputState) -> bool {
        let layer = &output.layers[pending.layer];
        layer.address == pending.address
            && layer.reset_trigger_id == pending.reset_trigger
            && layer.source_status == SourceStatus::Ready
    }

    fn restart(&self, layer: usize, commands: &mut Vec<CommandKind>) {
        commands.push(CommandKind::SetLayerControls {
            output: self.output,
            layer,
            controls: Box::new(LayerControls {
                play_mode: Some(PlayMode::Once {
                    end_state: OnceEndState::Hold,
                }),
                ..Default::default()
            }),
        });
        commands.push(CommandKind::ResetLayer {
            output: self.output,
            layer,
        });
        commands.push(CommandKind::ReportSourceStatus {
            output: self.output,
            layer,
            status: SourceStatus::Loading,
        });
    }

    fn advance_transition(&mut self, now: Timestamp, commands: &mut Vec<CommandKind>) {
        let transition = self.transition.as_ref().expect("checked by caller");
        let active = self
            .active
            .as_ref()
            .expect("a transition has an incoming clip");
        let progress = (now.since(transition.started).as_secs_f64()
            / transition.duration.as_secs_f64())
        .clamp(0.0, 1.0) as f32;
        let (outgoing, incoming) =
            crossfade_levels(transition.outgoing_layer, active.layer, progress);
        commands.push(self.level(transition.outgoing_layer, outgoing, 1.0 - progress));
        commands.push(self.level(active.layer, incoming, progress));
        if progress >= 1.0 {
            commands.push(self.clear(transition.outgoing_layer));
            self.transition = None;
        }
    }

    fn level(&self, layer: usize, dimmer: f32, volume: f32) -> CommandKind {
        CommandKind::SetLayerControls {
            output: self.output,
            layer,
            controls: Box::new(LayerControls {
                dimmer: Some(dimmer),
                volume: Some(volume),
                ..Default::default()
            }),
        }
    }

    fn clear(&self, layer: usize) -> CommandKind {
        CommandKind::SetLayerControls {
            output: self.output,
            layer,
            controls: Box::new(LayerControls {
                address: Some(MediaAddress::BLANK),
                play_mode: Some(PlayMode::Stop),
                dimmer: Some(0.0),
                volume: Some(0.0),
                ..Default::default()
            }),
        }
    }

    fn other(&self, layer: usize) -> usize {
        if self.pair[0] == layer {
            self.pair[1]
        } else {
            self.pair[0]
        }
    }

    fn issue(&mut self, issue: String) {
        if self.issues.len() == MAX_ISSUES {
            self.issues.remove(0);
        }
        self.issues.push(issue);
    }

    fn last_issue(&self) -> Option<&str> {
        self.issues.last().map(String::as_str)
    }

    fn fail(&mut self, message: &str, commands: &mut Vec<CommandKind>) {
        if self.terminal.is_some() {
            return;
        }
        self.issue(message.into());
        self.terminal = Some(RunnerPhase::Failed);
        self.pending = None;
        self.transition = None;
        commands.extend(self.stop());
    }

    pub(crate) fn status(&self, now: Timestamp) -> RunnerStatus {
        let progress = self.transition.as_ref().map_or(0.0, |transition| {
            (now.since(transition.started).as_secs_f64() / transition.duration.as_secs_f64())
                .clamp(0.0, 1.0) as f32
        });
        let index = self
            .active
            .as_ref()
            .map(|active| active.index)
            .or_else(|| self.pending.as_ref().map(|pending| pending.index));
        RunnerStatus {
            phase: self.terminal.unwrap_or(if self.transition.is_some() {
                RunnerPhase::Fading
            } else if self.active.is_some() {
                RunnerPhase::Playing
            } else {
                RunnerPhase::Loading
            }),
            entry_id: index.map(|index| self.playlist.entries[index].id.to_string()),
            entry_index: index,
            play: self.active.as_ref().map_or(0, |active| active.play),
            layer: self
                .active
                .as_ref()
                .map(|active| active.layer + 1)
                .or_else(|| self.pending.as_ref().map(|pending| pending.layer + 1)),
            transition_progress: progress,
            error: self.last_issue().map(str::to_owned),
        }
    }
}

/// Source-over compositing requires the lower clip to stay fully opaque while the upper clip
/// fades. Opposing ramps would leave only 75% opacity at the midpoint of two opaque sources.
/// Transparent sources retain their normal source-over semantics; they do not become opaque.
fn crossfade_levels(outgoing_layer: usize, incoming_layer: usize, progress: f32) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    if incoming_layer > outgoing_layer {
        (1.0, progress)
    } else {
        (1.0 - progress, 1.0)
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
