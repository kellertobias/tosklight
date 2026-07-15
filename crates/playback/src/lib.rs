#![forbid(unsafe_code)]
//! Tracking cue lists, live playback state, phasers, and HTP/LTP arbitration.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use light_core::{
    AttributeKey, AttributeValue, CueListId, FixtureId, MergeMode, SharedClock, SystemClock,
    TimedValue,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

type AttributeAddress = (FixtureId, AttributeKey);

pub const MAX_PLAYBACKS: u16 = 1_000;
pub const MAX_PLAYBACK_PAGES: u8 = 127;
pub const MAX_PAGE_SLOTS: u8 = 127;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTarget {
    CueList { cue_list_id: CueListId },
    Group { group_id: String },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackButtonAction {
    On,
    Off,
    Toggle,
    Go,
    GoMinus,
    Flash,
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackFaderMode {
    #[default]
    Master,
    Temp,
    Speed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackDefinition {
    pub number: u16,
    pub name: String,
    pub target: PlaybackTarget,
    #[serde(default)]
    pub buttons: [PlaybackButtonAction; 3],
    #[serde(default)]
    pub fader: PlaybackFaderMode,
    #[serde(default = "default_true")]
    pub go_activates: bool,
    #[serde(default = "default_true")]
    pub auto_off: bool,
    #[serde(default)]
    pub xfade_millis: u64,
}

fn default_true() -> bool {
    true
}

impl PlaybackDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_PLAYBACKS).contains(&self.number) {
            return Err("playback number must be within 1-1000".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err("playback name must contain 1-80 characters".into());
        }
        if self.xfade_millis > 60_000 {
            return Err("playback x-fade must not exceed 60 seconds".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackPage {
    pub number: u8,
    pub name: String,
    #[serde(default)]
    pub slots: HashMap<u8, u16>,
}

impl PlaybackPage {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_PLAYBACK_PAGES).contains(&self.number) {
            return Err("page number must be within 1-127".into());
        }
        if self.slots.iter().any(|(slot, playback)| {
            !(1..=MAX_PAGE_SLOTS).contains(slot) || !(1..=MAX_PLAYBACKS).contains(playback)
        }) {
            return Err("page slots must be within 1-127 and reference playbacks 1-1000".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CueChange {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    /// `None` is an explicit release, needed to implement cue-only when the attribute had no
    /// tracked value before the target cue.
    pub value: Option<AttributeValue>,
    #[serde(default)]
    pub automatic_restore: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}

impl CueChange {
    pub fn set(fixture_id: FixtureId, attribute: AttributeKey, value: AttributeValue) -> Self {
        Self {
            fixture_id,
            attribute,
            value: Some(value),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        }
    }

    fn address(&self) -> AttributeAddress {
        (self.fixture_id, self.attribute.clone())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cue {
    pub number: f64,
    pub name: String,
    pub changes: Vec<CueChange>,
    pub fade_millis: u64,
    pub delay_millis: u64,
    pub trigger: CueTrigger,
    #[serde(default)]
    pub phasers: Vec<AttributePhaser>,
    #[serde(default)]
    pub group_changes: Vec<GroupCueChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupCueChange {
    pub group_id: String,
    pub attribute: AttributeKey,
    pub value: Option<AttributeValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}

impl Cue {
    pub fn new(number: f64) -> Self {
        Self {
            number,
            name: String::new(),
            changes: Vec::new(),
            fade_millis: 0,
            delay_millis: 0,
            trigger: CueTrigger::Manual,
            phasers: Vec::new(),
            group_changes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CueTrigger {
    Manual,
    Follow { delay_millis: u64 },
    Wait { delay_millis: u64 },
    Timecode { frame: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueListMode {
    Sequence,
    Chaser,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CueList {
    pub id: CueListId,
    pub name: String,
    pub priority: i16,
    pub mode: CueListMode,
    pub looped: bool,
    #[serde(default = "default_chaser_step_millis")]
    pub chaser_step_millis: u64,
    #[serde(default)]
    pub speed_group: Option<String>,
    pub cues: Vec<Cue>,
}

fn default_chaser_step_millis() -> u64 {
    1_000
}

impl CueList {
    pub fn validate(&self) -> Result<(), String> {
        if self.cues.is_empty() {
            return Err("a cue list must contain at least one cue".into());
        }
        let mut previous = f64::NEG_INFINITY;
        for cue in &self.cues {
            if !cue.number.is_finite() || cue.number <= previous {
                return Err("cue numbers must be finite and strictly increasing".into());
            }
            previous = cue.number;
            let mut addresses = HashSet::new();
            for change in &cue.changes {
                if !addresses.insert(change.address()) {
                    return Err(format!(
                        "cue {} contains duplicate fixture attributes",
                        cue.number
                    ));
                }
            }
            for attribute_phaser in &cue.phasers {
                attribute_phaser.phaser.validate()?;
                if attribute_phaser.fixture_ids.is_empty() && attribute_phaser.group_ids.is_empty()
                {
                    return Err(format!(
                        "cue {} contains a phaser without fixtures",
                        cue.number
                    ));
                }
            }
        }
        Ok(())
    }

    /// Reconstructs a cue's tracked state exactly as sequential playback would produce it.
    pub fn state_at_index(&self, index: usize) -> HashMap<AttributeAddress, AttributeValue> {
        let mut state = HashMap::new();
        for cue in self.cues.iter().take(index.saturating_add(1)) {
            apply_changes(&mut state, &cue.changes);
        }
        state
    }

    pub fn state_at_number(&self, number: f64) -> HashMap<AttributeAddress, AttributeValue> {
        let index = self.cues.iter().rposition(|cue| cue.number <= number);
        index
            .map(|index| self.state_at_index(index))
            .unwrap_or_default()
    }

    /// Stores values cue-only and writes automatic restore/release changes into the following cue.
    /// Explicit changes already present in the following cue always win over generated restores.
    pub fn store_cue_only(&mut self, index: usize, changes: Vec<CueChange>) -> Result<(), String> {
        if index >= self.cues.len() {
            return Err("cue index is out of range".into());
        }
        if changes.iter().any(|change| change.value.is_none()) {
            return Err("cue-only input must contain values, not releases".into());
        }
        let previous = index
            .checked_sub(1)
            .map(|previous| self.state_at_index(previous))
            .unwrap_or_default();
        let addresses: HashSet<_> = changes.iter().map(CueChange::address).collect();
        self.cues[index]
            .changes
            .retain(|existing| !addresses.contains(&existing.address()));
        self.cues[index].changes.extend(changes);
        if let Some(next) = self.cues.get_mut(index + 1) {
            next.changes.retain(|change| {
                !(change.automatic_restore && addresses.contains(&change.address()))
            });
            let explicit: HashSet<_> = next
                .changes
                .iter()
                .filter(|change| !change.automatic_restore)
                .map(CueChange::address)
                .collect();
            for address in addresses.difference(&explicit) {
                next.changes.push(CueChange {
                    fixture_id: address.0,
                    attribute: address.1.clone(),
                    value: previous.get(address).cloned(),
                    automatic_restore: true,
                    fade_millis: None,
                    delay_millis: None,
                });
            }
        }
        Ok(())
    }
}

fn apply_changes(state: &mut HashMap<AttributeAddress, AttributeValue>, changes: &[CueChange]) {
    for change in changes {
        match &change.value {
            Some(value) => {
                state.insert(change.address(), value.clone());
            }
            None => {
                state.remove(&change.address());
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivePlayback {
    #[serde(default)]
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub cue_index: usize,
    pub previous_index: Option<usize>,
    pub paused: bool,
    pub activated_at: DateTime<Utc>,
    pub paused_at: Option<DateTime<Utc>>,
    #[serde(default = "default_master")]
    pub master: f32,
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub master_transition: Option<PlaybackMasterTransition>,
    #[serde(default)]
    pub temporary: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub flash_restore_off: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackMasterTransition {
    pub from: f32,
    pub to: f32,
    pub started_at: DateTime<Utc>,
    pub duration_millis: u64,
    pub release_after: bool,
}

fn default_master() -> f32 {
    1.0
}

#[derive(Clone, Debug)]
pub struct PlaybackEngine {
    cue_lists: HashMap<CueListId, CueList>,
    active: HashMap<CueListId, ActivePlayback>,
    speed_groups_bpm: [u16; 5],
    sequence_master_fade_millis: u64,
    definitions: HashMap<u16, PlaybackDefinition>,
    clock: SharedClock,
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }
}

impl PlaybackEngine {
    pub fn with_clock(clock: SharedClock) -> Self {
        Self {
            cue_lists: HashMap::new(),
            active: HashMap::new(),
            speed_groups_bpm: [120, 90, 60, 30, 15],
            sequence_master_fade_millis: 0,
            definitions: HashMap::new(),
            clock,
        }
    }

    pub fn clock(&self) -> SharedClock {
        Arc::clone(&self.clock)
    }
    pub fn set_control_timing(
        &mut self,
        speed_groups_bpm: [u16; 5],
        sequence_master_fade_millis: u64,
    ) {
        self.speed_groups_bpm = speed_groups_bpm.map(|bpm| bpm.clamp(1, 999));
        self.sequence_master_fade_millis = sequence_master_fade_millis.min(60_000);
    }
    pub fn register(&mut self, cue_list: CueList) -> Result<(), String> {
        cue_list.validate()?;
        self.cue_lists.insert(cue_list.id, cue_list);
        Ok(())
    }
    pub fn register_definition(&mut self, definition: PlaybackDefinition) -> Result<(), String> {
        definition.validate()?;
        if self.definitions.contains_key(&definition.number) {
            return Err("duplicate playback number".into());
        }
        if self
            .definitions
            .values()
            .any(|other| other.target == definition.target)
        {
            return Err("a cue list or group may only belong to one playback".into());
        }
        if let PlaybackTarget::CueList { cue_list_id } = definition.target
            && !self.cue_lists.contains_key(&cue_list_id)
        {
            return Err("playback cue list does not exist".into());
        }
        self.definitions.insert(definition.number, definition);
        Ok(())
    }

    pub fn definition(&self, number: u16) -> Option<&PlaybackDefinition> {
        self.definitions.get(&number)
    }

    pub fn go_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .clone();
        let PlaybackTarget::CueList { cue_list_id } = definition.target else {
            return Err("group playback does not have cues".into());
        };
        let was_active = self
            .active
            .get(&cue_list_id)
            .is_some_and(|playback| playback.enabled);
        self.go(cue_list_id)?;
        let result = self
            .active
            .get_mut(&cue_list_id)
            .expect("go inserted active playback");
        result.playback_number = Some(number);
        if definition.go_activates && !was_active {
            result.master = 1.0;
            result.enabled = true;
        }
        self.auto_off_overwritten();
        self.active
            .get(&cue_list_id)
            .ok_or_else(|| "playback was automatically switched off".into())
    }

    pub fn back_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        self.back(id)
    }

    pub fn on(&mut self, number: u16) -> Result<(), String> {
        let id = self.cue_list_for(number)?;
        if !self.active.contains_key(&id) {
            self.go(id)?;
        }
        let active = self.active.get_mut(&id).unwrap();
        active.playback_number = Some(number);
        active.master = 1.0;
        active.enabled = true;
        active.temporary = false;
        active.master_transition = None;
        self.auto_off_overwritten();
        Ok(())
    }

    pub fn off(&mut self, number: u16) -> Result<bool, String> {
        let id = self.cue_list_for(number)?;
        Ok(self
            .active
            .get_mut(&id)
            .map(|playback| {
                let was = playback.enabled;
                playback.enabled = false;
                playback.flash = false;
                playback.master_transition = None;
                was
            })
            .unwrap_or(false))
    }
    pub fn toggle(&mut self, number: u16) -> Result<bool, String> {
        let id = self.cue_list_for(number)?;
        if self
            .active
            .get(&id)
            .is_some_and(|playback| playback.enabled)
        {
            self.off(number)?;
            Ok(false)
        } else {
            self.on(number)?;
            Ok(true)
        }
    }
    pub fn set_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        let id = self.cue_list_for(number)?;
        if value > 0.0 && !self.active.contains_key(&id) {
            self.go(id)?;
        }
        let temporary = self.definitions[&number].fader == PlaybackFaderMode::Temp;
        if let Some(active) = self.active.get_mut(&id) {
            active.playback_number = Some(number);
            active.master = value;
            active.master_transition = None;
            active.temporary = temporary;
            if value > 0.0 {
                active.enabled = true;
            }
        }
        self.auto_off_overwritten();
        Ok(())
    }
    pub fn set_flash(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        let id = self.cue_list_for(number)?;
        if pressed && !self.active.contains_key(&id) {
            self.go(id)?;
        }
        if let Some(active) = self.active.get_mut(&id) {
            active.playback_number = Some(number);
            if pressed {
                active.flash_restore_off = !active.enabled;
                active.enabled = true;
                active.flash = true;
            } else {
                active.flash = false;
                if active.flash_restore_off {
                    active.enabled = false;
                    active.flash_restore_off = false;
                }
            }
        }
        Ok(())
    }
    pub fn button(&mut self, number: u16, button: u8, pressed: bool) -> Result<(), String> {
        let action = *self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .buttons
            .get(button.checked_sub(1).ok_or("button must be within 1-3")? as usize)
            .ok_or("button must be within 1-3")?;
        if !pressed && action != PlaybackButtonAction::Flash {
            return Ok(());
        }
        match action {
            PlaybackButtonAction::On => self.on(number),
            PlaybackButtonAction::Off => self.off(number).map(|_| ()),
            PlaybackButtonAction::Toggle => self.toggle(number).map(|_| ()),
            PlaybackButtonAction::Go => self.go_playback(number).map(|_| ()),
            PlaybackButtonAction::GoMinus => self.back_playback(number).map(|_| ()),
            PlaybackButtonAction::Flash => self.set_flash(number, pressed),
            PlaybackButtonAction::None => Ok(()),
        }
    }
    pub fn xfade(&mut self, number: u16, on: bool) -> Result<(), String> {
        let duration = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .xfade_millis;
        let id = self.cue_list_for(number)?;
        if on && !self.active.contains_key(&id) {
            self.go(id)?;
            self.active.get_mut(&id).unwrap().master = 0.0;
        }
        let active = self.active.get_mut(&id).ok_or("playback is not active")?;
        if on {
            active.enabled = true;
        }
        active.playback_number = Some(number);
        if duration == 0 {
            active.master = if on { 1.0 } else { 0.0 };
            if !on {
                self.active.get_mut(&id).unwrap().enabled = false;
            }
        } else {
            active.master_transition = Some(PlaybackMasterTransition {
                from: active.master,
                to: if on { 1.0 } else { 0.0 },
                started_at: self.clock.now(),
                duration_millis: duration,
                release_after: !on,
            });
        }
        Ok(())
    }
    fn cue_list_for(&self, number: u16) -> Result<CueListId, String> {
        match &self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .target
        {
            PlaybackTarget::CueList { cue_list_id } => Ok(*cue_list_id),
            PlaybackTarget::Group { .. } => {
                Err("operation is not available for a group playback".into())
            }
        }
    }

    pub fn go(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.go_at(id, self.clock.now())
    }

    pub fn go_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<&ActivePlayback, String> {
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let playback = match self.active.entry(id) {
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(ActivePlayback {
                playback_number: None,
                cue_list_id: id,
                cue_index: 0,
                previous_index: None,
                paused: false,
                activated_at: now,
                paused_at: None,
                master: 1.0,
                flash: false,
                master_transition: None,
                temporary: false,
                enabled: true,
                flash_restore_off: false,
            }),
            std::collections::hash_map::Entry::Occupied(entry) => {
                let playback = entry.into_mut();
                let resumed = playback.paused;
                if playback.paused {
                    if let Some(paused_at) = playback.paused_at.take() {
                        playback.activated_at += now - paused_at;
                    }
                    playback.paused = false;
                } else if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index += 1;
                } else if cue_list.looped {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = 0;
                }
                if !resumed {
                    playback.activated_at = now;
                }
                playback
            }
        };
        Ok(playback)
    }

    pub fn jump(&mut self, id: CueListId, cue_number: f64) -> Result<&ActivePlayback, String> {
        self.jump_at(id, cue_number, self.clock.now())
    }

    pub fn jump_at(
        &mut self,
        id: CueListId,
        cue_number: f64,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let index = cue_list
            .cues
            .iter()
            .position(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let playback = self.active.entry(id).or_insert(ActivePlayback {
            playback_number: None,
            cue_list_id: id,
            cue_index: index,
            previous_index: None,
            paused: false,
            activated_at: now,
            paused_at: None,
            master: 1.0,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: true,
            flash_restore_off: false,
        });
        if playback.cue_index != index {
            playback.previous_index = Some(playback.cue_index);
        }
        playback.cue_index = index;
        playback.paused = false;
        playback.paused_at = None;
        playback.activated_at = now;
        Ok(playback)
    }

    pub fn back(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.back_at(id, self.clock.now())
    }
    pub fn back_at(
        &mut self,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let playback = self.active.get_mut(&id).ok_or("cue list is not active")?;
        playback.previous_index = Some(playback.cue_index);
        playback.cue_index = playback.cue_index.saturating_sub(1);
        playback.activated_at = now;
        playback.paused = false;
        playback.paused_at = None;
        Ok(playback)
    }
    pub fn pause(&mut self, id: CueListId) -> Result<(), String> {
        self.pause_at(id, self.clock.now())
    }
    pub fn pause_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<(), String> {
        let playback = self.active.get_mut(&id).ok_or("cue list is not active")?;
        if !playback.paused {
            playback.paused = true;
            playback.paused_at = Some(now);
        }
        Ok(())
    }
    pub fn release(&mut self, id: CueListId) -> bool {
        self.active.remove(&id).is_some()
    }
    pub fn active(&self) -> Vec<ActivePlayback> {
        self.active
            .values()
            .filter(|playback| playback.enabled)
            .cloned()
            .collect()
    }

    fn auto_off_overwritten(&mut self) {
        let full: Vec<_> = self
            .active
            .values()
            .filter(|p| p.enabled && p.master >= 1.0 && !p.flash && !p.temporary)
            .map(|p| (p.cue_list_id, p.activated_at))
            .collect();
        let mut release = Vec::new();
        for playback in self.active.values() {
            if !playback.enabled {
                continue;
            }
            let Some(number) = playback.playback_number else {
                continue;
            };
            if !self.definitions.get(&number).is_some_and(|d| d.auto_off) {
                continue;
            }
            let own = self.cue_lists[&playback.cue_list_id].state_at_index(playback.cue_index);
            if own.is_empty() {
                continue;
            }
            let own_list = &self.cue_lists[&playback.cue_list_id];
            let covered = own.iter().all(|(address, own_value)| {
                full.iter().any(|(other, changed)| {
                    if *other == playback.cue_list_id {
                        return false;
                    }
                    let other_list = &self.cue_lists[other];
                    let Some(other_value) = other_list
                        .state_at_index(self.active[other].cue_index)
                        .get(address)
                        .cloned()
                    else {
                        return false;
                    };
                    if other_list.priority != own_list.priority {
                        other_list.priority > own_list.priority
                    } else if address.1.is_intensity() {
                        other_value.normalized().unwrap_or(0.0)
                            > own_value.normalized().unwrap_or(0.0)
                    } else {
                        *changed > playback.activated_at
                    }
                })
            });
            if covered {
                release.push(playback.cue_list_id);
            }
        }
        for id in release {
            if let Some(playback) = self.active.get_mut(&id) {
                playback.enabled = false;
            }
        }
    }
    pub fn restore_active(&mut self, playbacks: impl IntoIterator<Item = ActivePlayback>) {
        for mut playback in playbacks {
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(last) = cue_list.cues.len().checked_sub(1) else {
                continue;
            };
            playback.cue_index = playback.cue_index.min(last);
            playback.previous_index = playback.previous_index.map(|index| index.min(last));
            self.active.insert(playback.cue_list_id, playback);
        }
    }

    pub fn tick(&mut self, now: DateTime<Utc>, timecode_frame: Option<u64>) {
        let mut transition_releases = Vec::new();
        for playback in self.active.values_mut() {
            let Some(transition) = playback.master_transition.clone() else {
                continue;
            };
            let progress = if transition.duration_millis == 0 {
                1.0
            } else {
                ((now - transition.started_at).num_milliseconds().max(0) as f32
                    / transition.duration_millis as f32)
                    .clamp(0.0, 1.0)
            };
            playback.master = transition.from + (transition.to - transition.from) * progress;
            if progress >= 1.0 {
                playback.master_transition = None;
                if transition.release_after {
                    transition_releases.push(playback.cue_list_id);
                }
            }
        }
        for id in transition_releases {
            if let Some(playback) = self.active.get_mut(&id) {
                playback.enabled = false;
            }
        }
        let ids: Vec<_> = self.active.keys().copied().collect();
        for id in ids {
            let Some(cue_list) = self.cue_lists.get(&id) else {
                continue;
            };
            let Some(playback) = self.active.get_mut(&id) else {
                continue;
            };
            if !playback.enabled {
                continue;
            }
            if playback.paused {
                continue;
            }
            if let Some(frame) = timecode_frame
                && let Some(index) = cue_list
                    .cues
                    .iter()
                    .enumerate()
                    .filter_map(|(index, cue)| match cue.trigger {
                        CueTrigger::Timecode { frame: cue_frame } if cue_frame <= frame => {
                            Some(index)
                        }
                        _ => None,
                    })
                    .next_back()
                && index != playback.cue_index
            {
                playback.previous_index = Some(playback.cue_index);
                playback.cue_index = index;
                playback.activated_at = now;
                continue;
            }
            let elapsed = (now - playback.activated_at).num_milliseconds().max(0) as u64;
            let current = &cue_list.cues[playback.cue_index];
            let automatic_delay = match (&cue_list.mode, &current.trigger) {
                (CueListMode::Chaser, _) => Some(
                    cue_list
                        .speed_group
                        .as_ref()
                        .map(|group| {
                            let index = group
                                .as_bytes()
                                .first()
                                .copied()
                                .unwrap_or(b'A')
                                .saturating_sub(b'A')
                                .min(4) as usize;
                            60_000 / u64::from(self.speed_groups_bpm[index])
                        })
                        .unwrap_or(cue_list.chaser_step_millis),
                ),
                (_, CueTrigger::Follow { delay_millis })
                | (_, CueTrigger::Wait { delay_millis }) => {
                    Some(current.delay_millis + current.fade_millis + delay_millis)
                }
                _ => None,
            };
            if automatic_delay.is_some_and(|delay| elapsed >= delay) {
                playback.previous_index = Some(playback.cue_index);
                if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.cue_index += 1;
                } else if cue_list.looped {
                    playback.cue_index = 0;
                }
                playback.activated_at = now;
            }
        }
    }

    pub fn contributions(&self) -> Vec<TimedValue> {
        self.contributions_at(self.clock.now())
    }

    pub fn contributions_at(&self, now: DateTime<Utc>) -> Vec<TimedValue> {
        let mut values = Vec::new();
        for playback in self.active.values() {
            if !playback.enabled {
                continue;
            }
            let cue_list = &self.cue_lists[&playback.cue_list_id];
            let target = cue_list.state_at_index(playback.cue_index);
            let previous = playback
                .previous_index
                .map(|index| cue_list.state_at_index(index))
                .unwrap_or_default();
            let cue = &cue_list.cues[playback.cue_index];
            let effective_now = playback.paused_at.unwrap_or(now);
            let elapsed = (effective_now - playback.activated_at)
                .num_milliseconds()
                .max(0) as u64;
            let cue_fade_millis = if cue_list.mode == CueListMode::Sequence && cue.fade_millis == 0
            {
                self.sequence_master_fade_millis
            } else {
                cue.fade_millis
            };
            let timing = cue
                .changes
                .iter()
                .map(|change| (change.address(), (change.fade_millis, change.delay_millis)))
                .collect::<HashMap<_, _>>();
            let addresses: HashSet<_> = previous.keys().chain(target.keys()).cloned().collect();
            for (fixture_id, attribute) in addresses {
                let (fade_override, delay_override) = timing
                    .get(&(fixture_id, attribute.clone()))
                    .copied()
                    .unwrap_or((None, None));
                let fade_millis = fade_override.unwrap_or(cue_fade_millis);
                let delay_millis = delay_override.unwrap_or(cue.delay_millis);
                let progress = if elapsed < delay_millis {
                    0.0
                } else if fade_millis == 0 {
                    1.0
                } else {
                    ((elapsed - delay_millis) as f32 / fade_millis as f32).clamp(0.0, 1.0)
                };
                let value = interpolate(
                    previous.get(&(fixture_id, attribute.clone())),
                    target.get(&(fixture_id, attribute.clone())),
                    progress,
                );
                let Some(value) = value else {
                    continue;
                };
                let value = if attribute.is_intensity() {
                    value
                        .normalized()
                        .map(|level| {
                            AttributeValue::Normalized(
                                level * if playback.flash { 1.0 } else { playback.master },
                            )
                        })
                        .unwrap_or(value)
                } else {
                    value
                };
                values.push(TimedValue {
                    fixture_id,
                    merge_mode: if attribute.is_intensity() {
                        MergeMode::Htp
                    } else {
                        MergeMode::Ltp
                    },
                    attribute,
                    value,
                    priority: cue_list.priority,
                    changed_at: playback.activated_at,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                });
            }
            let phaser_elapsed = (effective_now
                - playback.activated_at
                - ChronoDuration::milliseconds(cue.delay_millis as i64))
            .num_milliseconds()
            .max(0) as f64
                / 1000.0;
            for attribute_phaser in &cue.phasers {
                for (index, fixture_id) in attribute_phaser.fixture_ids.iter().enumerate() {
                    let sampled = attribute_phaser.phaser.sample(
                        phaser_elapsed,
                        index,
                        attribute_phaser.fixture_ids.len(),
                    );
                    let base = target
                        .get(&(*fixture_id, attribute_phaser.attribute.clone()))
                        .and_then(AttributeValue::normalized)
                        .unwrap_or(0.0);
                    let level = match attribute_phaser.phaser.mode {
                        PhaserMode::Absolute => sampled,
                        PhaserMode::Relative => base + sampled,
                    }
                    .clamp(0.0, 1.0);
                    values.push(TimedValue {
                        fixture_id: *fixture_id,
                        attribute: attribute_phaser.attribute.clone(),
                        value: AttributeValue::Normalized(level),
                        priority: cue_list.priority,
                        changed_at: playback.activated_at,
                        merge_mode: if attribute_phaser.attribute.is_intensity() {
                            MergeMode::Htp
                        } else {
                            MergeMode::Ltp
                        },
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    });
                }
            }
        }
        values
    }
}

fn interpolate(
    from: Option<&AttributeValue>,
    to: Option<&AttributeValue>,
    progress: f32,
) -> Option<AttributeValue> {
    if progress >= 1.0 {
        return to.cloned();
    }
    match (from, to) {
        (Some(AttributeValue::Normalized(from)), Some(AttributeValue::Normalized(to))) => {
            Some(AttributeValue::Normalized(from + (to - from) * progress))
        }
        (None, Some(AttributeValue::Normalized(to))) => {
            Some(AttributeValue::Normalized(to * progress))
        }
        (Some(AttributeValue::Normalized(from)), None) => {
            Some(AttributeValue::Normalized(from * (1.0 - progress)))
        }
        (Some(from), _) => Some(from.clone()),
        (None, Some(to)) if progress >= 1.0 => Some(to.clone()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaserMode {
    Absolute,
    Relative,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaserCurve {
    Step,
    Linear,
    Sine,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaserStep {
    pub position: f32,
    pub value: f32,
    pub curve_to_next: PhaserCurve,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phaser {
    pub mode: PhaserMode,
    pub steps: Vec<PhaserStep>,
    pub cycles_per_minute: f32,
    pub phase_start_degrees: f32,
    pub phase_end_degrees: f32,
    pub width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributePhaser {
    pub fixture_ids: Vec<FixtureId>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    pub attribute: AttributeKey,
    pub phaser: Phaser,
}

impl Phaser {
    pub fn validate(&self) -> Result<(), String> {
        if self.steps.is_empty() {
            return Err("a phaser needs at least one step".into());
        }
        if !self.cycles_per_minute.is_finite() || self.cycles_per_minute <= 0.0 {
            return Err("phaser speed must be positive".into());
        }
        if !(0.0..=1.0).contains(&self.width) || self.width == 0.0 {
            return Err("phaser width must be within (0,1]".into());
        }
        let mut previous = -1.0;
        for step in &self.steps {
            if !(0.0..1.0).contains(&step.position)
                || step.position <= previous
                || !step.value.is_finite()
            {
                return Err("phaser steps must be finite and strictly ordered within [0,1)".into());
            }
            previous = step.position;
        }
        Ok(())
    }

    pub fn sample(&self, elapsed_seconds: f64, fixture_index: usize, fixture_count: usize) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let spread = if fixture_count <= 1 {
            0.0
        } else {
            fixture_index as f32 / (fixture_count - 1) as f32
        };
        let degrees =
            self.phase_start_degrees + (self.phase_end_degrees - self.phase_start_degrees) * spread;
        let mut phase = ((elapsed_seconds * f64::from(self.cycles_per_minute) / 60.0) as f32
            + degrees / 360.0)
            .rem_euclid(1.0);
        if phase > self.width {
            phase = 0.0;
        } else {
            phase /= self.width;
        }
        let current_index = self
            .steps
            .iter()
            .rposition(|step| step.position <= phase)
            .unwrap_or(self.steps.len() - 1);
        let current = &self.steps[current_index];
        let next = &self.steps[(current_index + 1) % self.steps.len()];
        let span = if next.position > current.position {
            next.position - current.position
        } else {
            1.0 - current.position + next.position
        };
        let distance = if phase >= current.position {
            phase - current.position
        } else {
            1.0 - current.position + phase
        };
        let mut progress = if span > 0.0 {
            (distance / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        progress = match current.curve_to_next {
            PhaserCurve::Step => 0.0,
            PhaserCurve::Linear => progress,
            PhaserCurve::Sine => (1.0 - (std::f32::consts::PI * progress).cos()) * 0.5,
        };
        current.value + (next.value - current.value) * progress
    }
}

pub fn resolve(
    values: impl IntoIterator<Item = TimedValue>,
) -> HashMap<AttributeAddress, AttributeValue> {
    let mut winners: HashMap<AttributeAddress, TimedValue> = HashMap::new();
    for candidate in values {
        let key = (candidate.fixture_id, candidate.attribute.clone());
        let replace = match winners.get(&key) {
            None => true,
            Some(current) if candidate.priority != current.priority => {
                candidate.priority > current.priority
            }
            Some(current) if candidate.merge_mode == MergeMode::Htp => {
                candidate.value.normalized().unwrap_or(0.0)
                    > current.value.normalized().unwrap_or(0.0)
            }
            Some(current) => candidate.changed_at > current.changed_at,
        };
        if replace {
            winners.insert(key, candidate);
        }
    }
    winners
        .into_iter()
        .map(|(key, value)| (key, value.value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(fixture: FixtureId, attribute: &str, value: f32) -> CueChange {
        CueChange::set(
            fixture,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        )
    }
    fn list(cues: Vec<Cue>) -> CueList {
        CueList {
            id: CueListId::new(),
            name: "Main".into(),
            priority: 10,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            cues,
        }
    }
    fn definition(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
        PlaybackDefinition {
            number,
            name: format!("Playback {number}"),
            target: PlaybackTarget::CueList { cue_list_id },
            buttons: [
                PlaybackButtonAction::Go,
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Flash,
            ],
            fader: PlaybackFaderMode::Master,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
        }
    }

    #[test]
    fn pool_master_scales_intensity_without_scaling_ltp_attributes() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "intensity", 1.0));
        cue.changes.push(value(fixture, "pan", 0.8));
        let list = list(vec![cue]);
        let id = list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.set_master(1, 0.5).unwrap();
        let values = engine.contributions();
        assert_eq!(
            values
                .iter()
                .find(|value| value.attribute.is_intensity())
                .unwrap()
                .value,
            AttributeValue::Normalized(0.5)
        );
        assert_eq!(
            values
                .iter()
                .find(|value| value.attribute.0 == "pan")
                .unwrap()
                .value,
            AttributeValue::Normalized(0.8)
        );
    }

    #[test]
    fn full_newer_playback_auto_offs_covered_playback_but_99_percent_does_not() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "pan", 0.2));
        let mut second = Cue::new(1.0);
        second.changes.push(value(fixture, "pan", 0.8));
        let first = list(vec![first]);
        let first_id = first.id;
        let second = list(vec![second]);
        let second_id = second.id;
        let mut engine = PlaybackEngine::default();
        engine.register(first).unwrap();
        engine.register(second).unwrap();
        engine.register_definition(definition(1, first_id)).unwrap();
        engine
            .register_definition(definition(2, second_id))
            .unwrap();
        engine.on(1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        engine.set_master(2, 0.99).unwrap();
        assert_eq!(engine.active().len(), 2);
        engine.set_master(2, 1.0).unwrap();
        assert_eq!(engine.active().len(), 1);
        assert_eq!(engine.active()[0].playback_number, Some(2));
    }

    #[test]
    fn page_and_pool_validation_enforce_public_ranges() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "pan", 0.1));
        let list = list(vec![cue]);
        let mut invalid = definition(1001, list.id);
        assert!(invalid.validate().is_err());
        invalid.number = 1;
        assert!(invalid.validate().is_ok());
        assert!(
            PlaybackPage {
                number: 0,
                name: "Bad".into(),
                slots: HashMap::new()
            }
            .validate()
            .is_err()
        );
        assert!(
            PlaybackPage {
                number: 127,
                name: "Last".into(),
                slots: HashMap::from([(127, 1000)])
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn toggle_retains_cue_and_flash_restores_off_state() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "pan", 0.1));
        let mut two = Cue::new(2.0);
        two.changes.push(value(fixture, "pan", 0.2));
        let list = list(vec![one, two]);
        let id = list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();
        assert_eq!(engine.active()[0].cue_index, 1);
        assert!(!engine.toggle(1).unwrap());
        assert!(engine.active().is_empty());
        engine.set_flash(1, true).unwrap();
        assert_eq!(engine.active()[0].cue_index, 1);
        engine.set_flash(1, false).unwrap();
        assert!(engine.active().is_empty());
        assert!(engine.toggle(1).unwrap());
        assert_eq!(engine.active()[0].cue_index, 1);
    }

    #[test]
    fn tracked_direct_jump_equals_sequential_state() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "intensity", 1.0));
        let mut two = Cue::new(2.0);
        two.changes.push(value(fixture, "pan", 0.5));
        let three = Cue::new(3.0);
        let list = list(vec![one, two, three]);
        assert_eq!(list.state_at_number(3.0), list.state_at_index(2));
        assert_eq!(list.state_at_index(2).len(), 2);
    }

    #[test]
    fn zero_delay_zero_fade_cue_is_active_at_go_timestamp() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "pan", 0.25));
        let cue_list = list(vec![cue]);
        let cue_list_id = cue_list.id;
        let now = Utc::now();
        let mut playback = PlaybackEngine::default();
        playback.register(cue_list).unwrap();
        playback.go_at(cue_list_id, now).unwrap();
        let contribution = playback.contributions_at(now);
        assert_eq!(contribution.len(), 1);
        assert_eq!(contribution[0].value, AttributeValue::Normalized(0.25));
    }

    #[test]
    fn cue_only_restores_previous_value_in_following_cue() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "intensity", 0.2));
        let two = Cue::new(2.0);
        let three = Cue::new(3.0);
        let mut list = list(vec![one, two, three]);
        list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
            .unwrap();
        assert_eq!(
            list.state_at_index(1)[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(1.0)
        );
        assert_eq!(
            list.state_at_index(2)[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.2)
        );
    }

    #[test]
    fn cue_only_releases_new_attribute_in_following_cue() {
        let fixture = FixtureId::new();
        let mut list = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
        list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
            .unwrap();
        assert!(list.state_at_index(2).is_empty());
    }

    #[test]
    fn explicit_next_cue_change_beats_automatic_restore() {
        let fixture = FixtureId::new();
        let one = Cue::new(1.0);
        let two = Cue::new(2.0);
        let mut three = Cue::new(3.0);
        three.changes.push(value(fixture, "intensity", 0.7));
        let mut list = list(vec![one, two, three]);
        list.store_cue_only(1, vec![value(fixture, "intensity", 1.0)])
            .unwrap();
        assert_eq!(
            list.state_at_index(2)[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.7)
        );
    }

    #[test]
    fn priority_then_htp_resolution() {
        let fixture = FixtureId::new();
        let now = Utc::now();
        let make = |level, priority| TimedValue {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(level),
            priority,
            changed_at: now,
            merge_mode: MergeMode::Htp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        };
        assert_eq!(
            resolve([make(1.0, 1), make(0.2, 2)])[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.2)
        );
        assert_eq!(
            resolve([make(0.4, 2), make(0.8, 2)])[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.8)
        );
    }

    #[test]
    fn phaser_interpolates_and_distributes_phase() {
        let phaser = Phaser {
            mode: PhaserMode::Absolute,
            steps: vec![
                PhaserStep {
                    position: 0.0,
                    value: 0.0,
                    curve_to_next: PhaserCurve::Linear,
                },
                PhaserStep {
                    position: 0.5,
                    value: 1.0,
                    curve_to_next: PhaserCurve::Linear,
                },
            ],
            cycles_per_minute: 60.0,
            phase_start_degrees: 0.0,
            phase_end_degrees: 180.0,
            width: 1.0,
        };
        assert!((phaser.sample(0.25, 0, 2) - 0.5).abs() < 0.001);
        assert!((phaser.sample(0.0, 1, 2) - 1.0).abs() < 0.001);
    }

    fn contribution_level(engine: &PlaybackEngine, at: DateTime<Utc>, fixture: FixtureId) -> f32 {
        engine
            .contributions_at(at)
            .into_iter()
            .find(|value| value.fixture_id == fixture && value.attribute.is_intensity())
            .and_then(|value| value.value.normalized())
            .unwrap_or(-1.0)
    }

    #[test]
    fn fades_from_zero_and_between_tracked_states() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.fade_millis = 1_000;
        first.changes.push(value(fixture, "intensity", 1.0));
        let mut second = Cue::new(2.0);
        second.fade_millis = 1_000;
        second.changes.push(value(fixture, "intensity", 0.0));
        let cue_list = list(vec![first, second]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(500),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );
        engine
            .go_at(id, started + ChronoDuration::seconds(1))
            .unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(1_500),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn cue_changes_keep_independent_fade_and_delay_times() {
        let first = FixtureId::new();
        let second = FixtureId::new();
        let mut cue = Cue::new(1.0);
        let mut immediate = value(first, "intensity", 1.0);
        immediate.fade_millis = Some(1_000);
        immediate.delay_millis = Some(0);
        cue.changes.push(immediate);
        let mut delayed = value(second, "intensity", 1.0);
        delayed.fade_millis = Some(1_000);
        delayed.delay_millis = Some(500);
        cue.changes.push(delayed);
        let cue_list = list(vec![cue]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        assert!(
            (contribution_level(&engine, started + ChronoDuration::milliseconds(500), first) - 0.5)
                .abs()
                < 0.01
        );
        assert!(
            contribution_level(&engine, started + ChronoDuration::milliseconds(500), second).abs()
                < 0.01
        );
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(1_000),
                second
            ) - 0.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn pause_freezes_and_resume_continues_fade() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.fade_millis = 1_000;
        cue.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![cue]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        engine
            .pause_at(id, started + ChronoDuration::milliseconds(250))
            .unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(800),
                fixture
            ) - 0.25)
                .abs()
                < 0.01
        );
        engine
            .go_at(id, started + ChronoDuration::milliseconds(800))
            .unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(1_050),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn follow_chaser_and_timecode_advance_without_manual_go() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.trigger = CueTrigger::Follow { delay_millis: 100 };
        first.changes.push(value(fixture, "intensity", 0.2));
        let mut second = Cue::new(2.0);
        second.trigger = CueTrigger::Timecode { frame: 250 };
        second.changes.push(value(fixture, "intensity", 0.8));
        let mut cue_list = list(vec![first, second]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list.clone()).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        engine.tick(started + ChronoDuration::milliseconds(99), None);
        assert_eq!(engine.active()[0].cue_index, 0);
        engine.tick(started + ChronoDuration::milliseconds(100), None);
        assert_eq!(engine.active()[0].cue_index, 1);
        engine.release(id);
        cue_list.mode = CueListMode::Chaser;
        cue_list.chaser_step_millis = 50;
        let mut chaser = PlaybackEngine::default();
        chaser.register(cue_list).unwrap();
        chaser.go_at(id, started).unwrap();
        chaser.tick(started + ChronoDuration::milliseconds(50), None);
        assert_eq!(chaser.active()[0].cue_index, 1);
        chaser.jump_at(id, 1.0, started).unwrap();
        chaser.tick(started, Some(250));
        assert_eq!(chaser.active()[0].cue_index, 1);
    }

    #[test]
    fn attribute_phaser_is_a_normal_playback_contribution() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.phasers.push(AttributePhaser {
            fixture_ids: vec![fixture],
            group_ids: vec![],
            attribute: AttributeKey::intensity(),
            phaser: Phaser {
                mode: PhaserMode::Absolute,
                steps: vec![
                    PhaserStep {
                        position: 0.0,
                        value: 0.0,
                        curve_to_next: PhaserCurve::Linear,
                    },
                    PhaserStep {
                        position: 0.5,
                        value: 1.0,
                        curve_to_next: PhaserCurve::Linear,
                    },
                ],
                cycles_per_minute: 60.0,
                phase_start_degrees: 0.0,
                phase_end_degrees: 0.0,
                width: 1.0,
            },
        });
        let cue_list = list(vec![cue]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(250),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn backend_speed_groups_drive_assigned_chasers() {
        let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
        cue_list.mode = CueListMode::Chaser;
        cue_list.speed_group = Some("B".into());
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.set_control_timing([60, 120, 30, 15, 10], 0);
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        engine.tick(started + ChronoDuration::milliseconds(499), None);
        assert_eq!(engine.active()[0].cue_index, 0);
        engine.tick(started + ChronoDuration::milliseconds(500), None);
        assert_eq!(engine.active()[0].cue_index, 1);

        let mut fifth = list(vec![Cue::new(1.0), Cue::new(2.0)]);
        fifth.mode = CueListMode::Chaser;
        fifth.speed_group = Some("E".into());
        let fifth_id = fifth.id;
        engine.register(fifth).unwrap();
        engine.go_at(fifth_id, started).unwrap();
        engine.tick(started + ChronoDuration::milliseconds(5_999), None);
        assert_eq!(
            engine
                .active()
                .iter()
                .find(|active| active.cue_list_id == fifth_id)
                .unwrap()
                .cue_index,
            0
        );
        engine.tick(started + ChronoDuration::milliseconds(6_000), None);
        assert_eq!(
            engine
                .active()
                .iter()
                .find(|active| active.cue_list_id == fifth_id)
                .unwrap()
                .cue_index,
            1
        );
    }

    #[test]
    fn sequence_master_fade_only_fills_missing_cue_fades() {
        let fixture = FixtureId::new();
        let mut fallback = Cue::new(1.0);
        fallback.changes.push(value(fixture, "intensity", 1.0));
        let mut cue_list = list(vec![fallback]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.set_control_timing([120, 90, 60, 30, 15], 1_000);
        engine.register(cue_list.clone()).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(500),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );

        cue_list.cues[0].fade_millis = 2_000;
        let mut explicit = PlaybackEngine::default();
        explicit.set_control_timing([120, 90, 60, 30, 15], 1_000);
        explicit.register(cue_list).unwrap();
        explicit.go_at(id, started).unwrap();
        assert!(
            (contribution_level(
                &explicit,
                started + ChronoDuration::milliseconds(500),
                fixture
            ) - 0.25)
                .abs()
                < 0.01
        );
    }
}
