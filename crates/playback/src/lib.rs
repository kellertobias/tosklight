#![forbid(unsafe_code)]
//! Tracking cue lists, live playback state, phasers, and HTP/LTP arbitration.

mod automatic;

pub use automatic::{
    AutomaticPlaybackTransition, AutomaticPlaybackTransitionCause, PlaybackCueReference,
    PlaybackTickResult,
};

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
use uuid::Uuid;

type AttributeAddress = (FixtureId, AttributeKey);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PlaybackKey {
    Number(u16),
    CueList(CueListId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TemporaryPlaybackKind {
    Flash,
    TempButton,
    TempFader,
    Swap,
}

pub const MAX_PLAYBACKS: u16 = 1_000;
pub const MAX_PLAYBACK_PAGES: u8 = 127;
pub const MAX_PAGE_SLOTS: u8 = 127;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTarget {
    CueList { cue_list_id: CueListId },
    Group { group_id: String },
    SpeedGroup { group: String },
    ProgrammerFade,
    CueFade,
    GrandMaster,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackButtonAction {
    On,
    Off,
    Toggle,
    Go,
    GoMinus,
    FastForward,
    FastRewind,
    Flash,
    Temp,
    Swap,
    Select,
    SelectContents,
    SelectDereferenced,
    Learn,
    Double,
    Half,
    Pause,
    Blackout,
    PauseDynamics,
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
    XFade,
    DirectBpm,
    CenteredRelative,
    LearnedPercentage,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlashReleaseMode {
    #[default]
    ReleaseAll,
    ReleaseIntensityOnly,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaybackDefinition {
    pub number: u16,
    pub name: String,
    pub target: PlaybackTarget,
    #[serde(default)]
    pub buttons: [PlaybackButtonAction; 3],
    #[serde(default = "default_button_count")]
    pub button_count: u8,
    #[serde(default)]
    pub fader: PlaybackFaderMode,
    #[serde(default = "default_true")]
    pub has_fader: bool,
    #[serde(default = "default_true")]
    pub go_activates: bool,
    #[serde(default = "default_true")]
    pub auto_off: bool,
    #[serde(default)]
    pub xfade_millis: u64,
    #[serde(default = "default_playback_color")]
    pub color: String,
    #[serde(default)]
    pub flash_release: FlashReleaseMode,
    #[serde(default)]
    pub protect_from_swap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_image: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_button_count() -> u8 {
    3
}
fn default_playback_color() -> String {
    "#20c997".into()
}

impl PlaybackDefinition {
    pub fn default_buttons(target: &PlaybackTarget) -> [PlaybackButtonAction; 3] {
        match target {
            PlaybackTarget::CueList { .. } => [
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ],
            PlaybackTarget::Group { .. } => [
                PlaybackButtonAction::Select,
                PlaybackButtonAction::SelectDereferenced,
                PlaybackButtonAction::Flash,
            ],
            PlaybackTarget::SpeedGroup { .. } => [
                PlaybackButtonAction::Double,
                PlaybackButtonAction::Half,
                PlaybackButtonAction::Learn,
            ],
            PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => [
                PlaybackButtonAction::Double,
                PlaybackButtonAction::Half,
                PlaybackButtonAction::Off,
            ],
            PlaybackTarget::GrandMaster => [
                PlaybackButtonAction::Blackout,
                PlaybackButtonAction::PauseDynamics,
                PlaybackButtonAction::Flash,
            ],
        }
    }

    pub fn default_fader(target: &PlaybackTarget) -> PlaybackFaderMode {
        match target {
            PlaybackTarget::SpeedGroup { .. } => PlaybackFaderMode::LearnedPercentage,
            _ => PlaybackFaderMode::Master,
        }
    }

    pub fn reset_incompatible_layout(&mut self) {
        if self.layout_is_compatible() {
            return;
        }
        self.buttons = Self::default_buttons(&self.target);
        self.fader = Self::default_fader(&self.target);
    }

    pub fn layout_is_compatible(&self) -> bool {
        let buttons_compatible = self.buttons.iter().enumerate().all(|(index, action)| {
            if index >= usize::from(self.button_count) {
                return *action == PlaybackButtonAction::None;
            }
            match &self.target {
                PlaybackTarget::CueList { .. } => matches!(
                    action,
                    PlaybackButtonAction::On
                        | PlaybackButtonAction::Off
                        | PlaybackButtonAction::Toggle
                        | PlaybackButtonAction::Go
                        | PlaybackButtonAction::GoMinus
                        | PlaybackButtonAction::FastForward
                        | PlaybackButtonAction::FastRewind
                        | PlaybackButtonAction::Pause
                        | PlaybackButtonAction::Flash
                        | PlaybackButtonAction::Temp
                        | PlaybackButtonAction::Swap
                        | PlaybackButtonAction::Select
                        | PlaybackButtonAction::SelectContents
                        | PlaybackButtonAction::None
                ),
                PlaybackTarget::Group { .. } => matches!(
                    action,
                    PlaybackButtonAction::Select
                        | PlaybackButtonAction::SelectDereferenced
                        | PlaybackButtonAction::Flash
                        | PlaybackButtonAction::None
                ),
                PlaybackTarget::SpeedGroup { .. } => matches!(
                    action,
                    PlaybackButtonAction::Learn
                        | PlaybackButtonAction::Double
                        | PlaybackButtonAction::Half
                        | PlaybackButtonAction::Pause
                        | PlaybackButtonAction::None
                ),
                PlaybackTarget::ProgrammerFade | PlaybackTarget::CueFade => matches!(
                    action,
                    PlaybackButtonAction::Double
                        | PlaybackButtonAction::Half
                        | PlaybackButtonAction::Off
                        | PlaybackButtonAction::None
                ),
                PlaybackTarget::GrandMaster => matches!(
                    action,
                    PlaybackButtonAction::Blackout
                        | PlaybackButtonAction::Flash
                        | PlaybackButtonAction::PauseDynamics
                        | PlaybackButtonAction::None
                ),
            }
        });
        let fader_compatible = match &self.target {
            PlaybackTarget::CueList { .. } => matches!(
                self.fader,
                PlaybackFaderMode::Master | PlaybackFaderMode::Temp | PlaybackFaderMode::XFade
            ),
            PlaybackTarget::SpeedGroup { .. } => matches!(
                self.fader,
                PlaybackFaderMode::DirectBpm
                    | PlaybackFaderMode::CenteredRelative
                    | PlaybackFaderMode::LearnedPercentage
            ),
            PlaybackTarget::Group { .. }
            | PlaybackTarget::ProgrammerFade
            | PlaybackTarget::CueFade
            | PlaybackTarget::GrandMaster => self.fader == PlaybackFaderMode::Master,
        };
        buttons_compatible && fader_compatible
    }

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
        if self.button_count > 3 {
            return Err("playback button count must be within 0-3".into());
        }
        if !self.layout_is_compatible() {
            return Err("playback layout is incompatible with its function".into());
        }
        if let PlaybackTarget::SpeedGroup { group } = &self.target
            && !matches!(
                group.to_ascii_uppercase().as_str(),
                "A" | "B" | "C" | "D" | "E"
            )
        {
            return Err("Speed Group must be A-E".into());
        }
        let bytes = self.color.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
            return Err("playback color must be a six-digit hexadecimal color".into());
        }
        if self.presentation_icon.is_some() && self.presentation_image.is_some() {
            return Err(
                "playback presentation accepts either an icon or an image, not both".into(),
            );
        }
        for (name, value) in [
            ("icon", self.presentation_icon.as_deref()),
            ("image", self.presentation_image.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty() || value.len() > 1_024) {
                return Err(format!(
                    "playback presentation {name} must contain 1-1024 characters"
                ));
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for PlaybackDefinition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredPlaybackDefinition {
            number: u16,
            name: String,
            target: PlaybackTarget,
            #[serde(default)]
            buttons: Option<[PlaybackButtonAction; 3]>,
            #[serde(default = "default_button_count")]
            button_count: u8,
            #[serde(default)]
            fader: Option<PlaybackFaderMode>,
            #[serde(default = "default_true")]
            has_fader: bool,
            #[serde(default = "default_true")]
            go_activates: bool,
            #[serde(default = "default_true")]
            auto_off: bool,
            #[serde(default)]
            xfade_millis: u64,
            #[serde(default = "default_playback_color")]
            color: String,
            #[serde(default)]
            flash_release: FlashReleaseMode,
            #[serde(default)]
            protect_from_swap: bool,
            #[serde(default)]
            presentation_icon: Option<String>,
            #[serde(default)]
            presentation_image: Option<String>,
        }

        let stored = StoredPlaybackDefinition::deserialize(deserializer)?;
        let buttons = stored
            .buttons
            .unwrap_or_else(|| PlaybackDefinition::default_buttons(&stored.target));
        let mut fader = stored
            .fader
            .unwrap_or_else(|| PlaybackDefinition::default_fader(&stored.target));
        if matches!(stored.target, PlaybackTarget::SpeedGroup { .. })
            && fader == PlaybackFaderMode::Speed
        {
            fader = PlaybackFaderMode::LearnedPercentage;
        }
        Ok(Self {
            number: stored.number,
            name: stored.name,
            target: stored.target,
            buttons,
            button_count: stored.button_count,
            fader,
            has_fader: stored.has_fader,
            go_activates: stored.go_activates,
            auto_off: stored.auto_off,
            xfade_millis: stored.xfade_millis,
            color: stored.color,
            flash_release: stored.flash_release,
            protect_from_swap: stored.protect_from_swap,
            presentation_icon: stored.presentation_icon,
            presentation_image: stored.presentation_image,
        })
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
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub number: f64,
    pub name: String,
    pub changes: Vec<CueChange>,
    pub fade_millis: u64,
    pub delay_millis: u64,
    pub trigger: CueTrigger,
    /// Marks an operator-recorded Cue-only Cue so an appended following Cue can generate the
    /// required automatic restore/release delta after a save, refresh, or reopen.
    #[serde(default)]
    pub cue_only: bool,
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
    #[serde(default)]
    pub automatic_restore: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_millis: Option<u64>,
}

impl Cue {
    pub fn new(number: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            number,
            name: String::new(),
            changes: Vec::new(),
            fade_millis: 0,
            delay_millis: 0,
            trigger: CueTrigger::Manual,
            cue_only: false,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntensityPriorityMode {
    #[default]
    Htp,
    Ltp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapMode {
    Off,
    Tracking,
    Reset,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartMode {
    #[default]
    FirstCue,
    ContinueCurrentCue,
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
    #[serde(default)]
    pub intensity_priority_mode: IntensityPriorityMode,
    /// `None` is the legacy representation; `looped` is then migrated at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_mode: Option<WrapMode>,
    #[serde(default)]
    pub restart_mode: RestartMode,
    #[serde(default)]
    pub force_cue_timing: bool,
    #[serde(default)]
    pub disable_cue_timing: bool,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub chaser_xfade_millis: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chaser_xfade_percent: Option<u8>,
    #[serde(default = "default_speed_multiplier")]
    pub speed_multiplier: f32,
    pub cues: Vec<Cue>,
}

fn default_chaser_step_millis() -> u64 {
    1_000
}
fn default_speed_multiplier() -> f32 {
    1.0
}
fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn cue_completion_millis(cue_list: &CueList, cue: &Cue, sequence_master_fade_millis: u64) -> u64 {
    if cue_list.disable_cue_timing {
        return 0;
    }
    let cue_fade_millis = if cue_list.mode == CueListMode::Sequence && cue.fade_millis == 0 {
        sequence_master_fade_millis
    } else {
        cue.fade_millis
    };
    if cue_list.force_cue_timing {
        return cue.delay_millis.saturating_add(cue_fade_millis);
    }
    let fixture_values = cue.changes.iter().map(|change| {
        change
            .delay_millis
            .unwrap_or(cue.delay_millis)
            .saturating_add(change.fade_millis.unwrap_or(cue_fade_millis))
    });
    let group_values = cue.group_changes.iter().map(|change| {
        change
            .delay_millis
            .unwrap_or(cue.delay_millis)
            .saturating_add(change.fade_millis.unwrap_or(cue_fade_millis))
    });
    fixture_values
        .chain(group_values)
        .max()
        .unwrap_or_else(|| cue.delay_millis.saturating_add(cue_fade_millis))
}

fn effective_chaser_step_millis(cue_list: &CueList, speed_groups_bpm: &[f64; 5]) -> u64 {
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
            (60_000.0 / speed_groups_bpm[index] / f64::from(cue_list.speed_multiplier)).round()
                as u64
        })
        .unwrap_or(cue_list.chaser_step_millis)
        .max(1)
}

pub fn effective_chaser_xfade_percent(cue_list: &CueList, speed_groups_bpm: &[f64; 5]) -> u8 {
    cue_list.chaser_xfade_percent.unwrap_or_else(|| {
        let step = effective_chaser_step_millis(cue_list, speed_groups_bpm);
        ((cue_list.chaser_xfade_millis.saturating_mul(100) + step / 2) / step).min(100) as u8
    })
}

pub fn effective_chaser_xfade_millis(cue_list: &CueList, speed_groups_bpm: &[f64; 5]) -> u64 {
    if cue_list.disable_cue_timing {
        return 0;
    }
    let step = effective_chaser_step_millis(cue_list, speed_groups_bpm);
    (step.saturating_mul(u64::from(effective_chaser_xfade_percent(
        cue_list,
        speed_groups_bpm,
    ))) + 50)
        / 100
}

impl CueList {
    pub fn migrate_legacy_chaser_xfade(&mut self, speed_groups_bpm: &[f64; 5]) {
        if self.chaser_xfade_percent.is_some() {
            self.chaser_xfade_millis = 0;
            return;
        }
        self.chaser_xfade_percent = Some(effective_chaser_xfade_percent(self, speed_groups_bpm));
        self.chaser_xfade_millis = 0;
    }
    pub fn effective_wrap_mode(&self) -> WrapMode {
        self.wrap_mode.unwrap_or(if self.looped {
            WrapMode::Tracking
        } else {
            WrapMode::Off
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.speed_multiplier.is_finite() || !(0.01..=100.0).contains(&self.speed_multiplier) {
            return Err("speed multiplier must be within 0.01-100".into());
        }
        if self
            .chaser_xfade_percent
            .is_some_and(|percent| percent > 100)
        {
            return Err("chaser x-fade percent must be within 0-100".into());
        }
        if self.chaser_xfade_percent.is_none() && self.chaser_xfade_millis > 60_000 {
            return Err("chaser x-fade must not exceed 60 seconds".into());
        }
        if let Some(group) = &self.speed_group
            && !matches!(group.as_str(), "A" | "B" | "C" | "D" | "E")
        {
            return Err("speed group must be A-E".into());
        }
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
        self.cues[index].cue_only = true;
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
    /// Last physical control position. On deliberately does not move this value.
    #[serde(default = "default_master")]
    pub fader_position: f32,
    /// Off at a non-zero physical position latches the fader until it reaches zero.
    #[serde(default)]
    pub fader_pickup_required: bool,
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
    /// Fast navigation bypasses Cue and per-attribute delay/fade for only this transition.
    #[serde(default)]
    pub transition_timing_bypassed: bool,
    /// A one-transition fallback supplied by an atomic Preload GO. Explicit Cue and
    /// per-attribute timings remain authoritative; this replaces only the Cue Fade master.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_fade_fallback_millis: Option<u64>,
    #[serde(default)]
    pub manual_xfade_position: f32,
    #[serde(default)]
    pub manual_xfade_direction: ManualXFadeDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_xfade_from_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_xfade_to_index: Option<usize>,
    #[serde(default)]
    pub manual_xfade_progress: f32,
    /// While set, forward navigation has wrapped in Tracking mode and the final
    /// tracked state remains the base until a Cue explicitly changes it.
    #[serde(default)]
    pub tracking_wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_cue_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_cue_number: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_hold: Option<DeletedCueHold>,
    /// When navigation resolves a deleted-active Cue hold, this preserves the rendered held
    /// contribution as the source of the destination Cue's normal fade. It is cleared by the
    /// next navigation or activation operation and is never written into Cue data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_transition_source: Option<Vec<TimedValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_number: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualXFadeDirection {
    #[default]
    TowardsHigh,
    TowardsLow,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaybackRuntimeStatus {
    #[serde(flatten)]
    pub playback: ActivePlayback,
    pub normal_next_cue_id: Option<Uuid>,
    pub normal_next_cue_number: Option<f64>,
    pub effective_next_cue_id: Option<Uuid>,
    pub effective_next_cue_number: Option<f64>,
    pub effective_next_is_loaded: bool,
    pub temporary_active: bool,
    pub temporary_master: f32,
    pub swap_active: bool,
}

/// A Position-family value which an active Cuelist can safely preposition while its fixture is
/// dark. The engine owns the resolved-dark clock and turns these look-ahead records into runtime
/// contributions; Cue data is never modified.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackTargetValue {
    pub attribute: AttributeKey,
    pub current: AttributeValue,
    pub target: AttributeValue,
    pub fade_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackCandidate {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub current_cue_id: Uuid,
    pub current_cue_number: f64,
    pub target_cue_id: Uuid,
    pub target_cue_number: f64,
    pub fixture_id: FixtureId,
    pub priority: i16,
    pub values: Vec<MoveInBlackTargetValue>,
}

/// Stable identity of the playback whose sequence master applies to a contribution. Keeping this
/// separate from `TimedValue` lets the engine retain source-specific master semantics without
/// leaking playback concerns into programmer and show data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SequenceMasterSource {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub temporary: bool,
}

#[derive(Clone, Debug)]
pub struct PlaybackContribution {
    pub value: TimedValue,
    pub sequence_master: f32,
    pub source: SequenceMasterSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeletedCueHold {
    pub deleted_number: f64,
    pub previous_number: Option<f64>,
    pub next_number: Option<f64>,
    pub contributions: Vec<TimedValue>,
}

fn advance_chaser_steps(playback: &mut ActivePlayback, cue_list: &CueList, steps: u64) -> u64 {
    if steps == 0 {
        return 0;
    }
    playback.deleted_cue_transition_source = None;
    let start = playback.cue_index as u128;
    let total = start + u128::from(steps);
    let last = cue_list.cues.len() - 1;
    if cue_list.effective_wrap_mode() == WrapMode::Off {
        playback.cue_index =
            usize::try_from(total.min(last as u128)).expect("clamped Cue index fits usize");
        playback.previous_index = Some(if total > last as u128 {
            last
        } else {
            playback.cue_index.saturating_sub(1)
        });
    } else {
        let cue_count = cue_list.cues.len() as u128;
        playback.cue_index =
            usize::try_from(total % cue_count).expect("modulo Cue index fits usize");
        playback.previous_index =
            Some(usize::try_from((total - 1) % cue_count).expect("modulo Cue index fits usize"));
        if cue_list.effective_wrap_mode() == WrapMode::Tracking && total >= cue_count {
            playback.tracking_wrap = true;
        } else if cue_list.effective_wrap_mode() == WrapMode::Reset {
            playback.tracking_wrap = false;
        }
    }
    playback.current_cue_number = Some(cue_list.cues[playback.cue_index].number);
    playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
    if cue_list.effective_wrap_mode() == WrapMode::Off {
        steps.min(last.saturating_sub(start as usize) as u64)
    } else {
        steps
    }
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

fn reset_manual_transition(playback: &mut ActivePlayback) {
    playback.transition_timing_bypassed = false;
    playback.transition_fade_fallback_millis = None;
    playback.manual_xfade_from_index = None;
    playback.manual_xfade_to_index = None;
    playback.manual_xfade_progress = 0.0;
}

fn new_active_playback(
    playback_number: Option<u16>,
    cue_list: &CueList,
    now: DateTime<Utc>,
    master: f32,
    enabled: bool,
) -> ActivePlayback {
    ActivePlayback {
        playback_number,
        cue_list_id: cue_list.id,
        cue_index: 0,
        previous_index: None,
        paused: false,
        activated_at: now,
        paused_at: None,
        master,
        fader_position: master,
        fader_pickup_required: false,
        flash: false,
        master_transition: None,
        temporary: false,
        enabled,
        flash_restore_off: false,
        transition_timing_bypassed: false,
        transition_fade_fallback_millis: None,
        manual_xfade_position: 0.0,
        manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
        manual_xfade_from_index: None,
        manual_xfade_to_index: None,
        manual_xfade_progress: 0.0,
        tracking_wrap: false,
        current_cue_id: cue_list.cues.first().map(|cue| cue.id),
        current_cue_number: cue_list.cues.first().map(|cue| cue.number),
        deleted_cue_hold: None,
        deleted_cue_transition_source: None,
        loaded_cue_id: None,
        loaded_cue_number: None,
    }
}

#[derive(Clone, Debug)]
pub struct PlaybackEngine {
    cue_lists: HashMap<CueListId, CueList>,
    active: HashMap<PlaybackKey, ActivePlayback>,
    temporary: HashMap<(u16, TemporaryPlaybackKind), ActivePlayback>,
    swap_held: HashSet<u16>,
    dynamics_paused_at: Option<DateTime<Utc>>,
    speed_groups_bpm: [f64; 5],
    speed_groups_paused: [bool; 5],
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
            temporary: HashMap::new(),
            swap_held: HashSet::new(),
            dynamics_paused_at: None,
            speed_groups_bpm: [120.0, 90.0, 60.0, 30.0, 15.0],
            speed_groups_paused: [false; 5],
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
        speed_groups_bpm: [f64; 5],
        sequence_master_fade_millis: u64,
    ) {
        let next_speed_groups_bpm = speed_groups_bpm.map(|bpm| {
            if bpm.is_finite() {
                bpm.clamp(0.1, 999.0)
            } else {
                120.0
            }
        });
        let now = self.clock.now();
        for playback in self.active.values_mut() {
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            if cue_list.mode != CueListMode::Chaser || cue_list.speed_group.is_none() {
                continue;
            }
            let old_step = effective_chaser_step_millis(cue_list, &self.speed_groups_bpm);
            let next_step = effective_chaser_step_millis(cue_list, &next_speed_groups_bpm);
            if old_step == next_step {
                continue;
            }
            let phase_at = playback.paused_at.unwrap_or(now);
            let elapsed = (phase_at - playback.activated_at).num_milliseconds().max(0) as u64;
            let completed_steps = elapsed / old_step;
            advance_chaser_steps(playback, cue_list, completed_steps);
            let old_phase = elapsed % old_step;
            let next_phase =
                ((old_phase as f64 / old_step as f64) * next_step as f64).round() as i64;
            playback.activated_at = phase_at - ChronoDuration::milliseconds(next_phase);
        }
        self.speed_groups_bpm = next_speed_groups_bpm;
        self.sequence_master_fade_millis = sequence_master_fade_millis.min(60_000);
    }
    pub fn set_speed_groups_paused(&mut self, paused: [bool; 5]) {
        self.speed_groups_paused = paused;
    }
    pub fn dynamics_paused(&self) -> bool {
        self.dynamics_paused_at.is_some()
    }
    pub fn dynamics_paused_since(&self) -> Option<DateTime<Utc>> {
        self.dynamics_paused_at
    }
    pub fn restore_dynamics_paused_since(&mut self, paused_at: Option<DateTime<Utc>>) {
        self.dynamics_paused_at = paused_at;
    }
    pub fn set_dynamics_paused(&mut self, paused: bool) {
        let now = self.clock.now();
        match (paused, self.dynamics_paused_at) {
            (true, None) => self.dynamics_paused_at = Some(now),
            (false, Some(paused_at)) => {
                let shift_timestamp = |timestamp: &mut DateTime<Utc>| {
                    if *timestamp <= paused_at {
                        *timestamp += now - paused_at;
                    } else {
                        *timestamp = now;
                    }
                };
                for playback in self.active.values_mut().chain(self.temporary.values_mut()) {
                    shift_timestamp(&mut playback.activated_at);
                    if let Some(paused) = &mut playback.paused_at {
                        shift_timestamp(paused);
                    }
                    if let Some(transition) = &mut playback.master_transition {
                        shift_timestamp(&mut transition.started_at);
                    }
                }
                self.dynamics_paused_at = None;
            }
            _ => {}
        }
    }
    pub fn toggle_dynamics_paused(&mut self) -> bool {
        let paused = !self.dynamics_paused();
        self.set_dynamics_paused(paused);
        paused
    }
    pub fn register(&mut self, mut cue_list: CueList) -> Result<(), String> {
        cue_list.validate()?;
        cue_list.migrate_legacy_chaser_xfade(&self.speed_groups_bpm);
        self.cue_lists.insert(cue_list.id, cue_list);
        Ok(())
    }
    pub fn register_definition(&mut self, definition: PlaybackDefinition) -> Result<(), String> {
        definition.validate()?;
        if self.definitions.contains_key(&definition.number) {
            return Err("duplicate playback number".into());
        }
        let cue_list_id = match &definition.target {
            PlaybackTarget::CueList { cue_list_id } => Some(*cue_list_id),
            _ => None,
        };
        if let Some(cue_list_id) = cue_list_id {
            if !self.cue_lists.contains_key(&cue_list_id) {
                return Err("playback cue list does not exist".into());
            }
            let first_assignment = !self.definitions.values().any(|existing| matches!(existing.target, PlaybackTarget::CueList { cue_list_id: existing_id } if existing_id == cue_list_id));
            if first_assignment
                && let Some(mut playback) = self.active.remove(&PlaybackKey::CueList(cue_list_id))
            {
                playback.playback_number = Some(definition.number);
                self.active
                    .insert(PlaybackKey::Number(definition.number), playback);
            }
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
        let key = PlaybackKey::Number(number);
        let was_active = self
            .active
            .get(&key)
            .is_some_and(|playback| playback.enabled);
        let has_loaded_cue = self
            .active
            .get(&key)
            .is_some_and(|playback| playback.loaded_cue_id.is_some());
        if definition.go_activates && !was_active && !has_loaded_cue {
            self.on(number)?;
            return self
                .active
                .get(&key)
                .ok_or_else(|| "playback was automatically switched off".into());
        }
        self.go_at_key(key, cue_list_id, self.clock.now())?;
        let result = self
            .active
            .get_mut(&key)
            .expect("go inserted active playback");
        result.playback_number = Some(number);
        if definition.go_activates && !was_active {
            result.master = 1.0;
            result.enabled = true;
        }
        self.auto_off_overwritten();
        self.active
            .get(&key)
            .ok_or_else(|| "playback was automatically switched off".into())
    }

    pub fn back_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        self.back_at_key(PlaybackKey::Number(number), id, self.clock.now())
    }

    pub fn fast_forward_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        self.go_playback(number)?;
        let playback = self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn fast_rewind_playback(&mut self, number: u16) -> Result<&ActivePlayback, String> {
        self.back_playback(number)?;
        let playback = self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .ok_or("playback is not active")?;
        playback.transition_timing_bypassed = true;
        Ok(playback)
    }

    pub fn goto_playback(
        &mut self,
        number: u16,
        cue_number: f64,
    ) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        if !self.cue_lists[&id]
            .cues
            .iter()
            .any(|cue| cue.number == cue_number)
        {
            return Err("cue does not exist".into());
        }
        let key = PlaybackKey::Number(number);
        self.jump_at_key(key, id, cue_number, self.clock.now())?;
        let playback = self.active.get_mut(&key).unwrap();
        playback.playback_number = Some(number);
        playback.master = 1.0;
        playback.enabled = true;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        self.auto_off_overwritten();
        self.active
            .get(&key)
            .ok_or_else(|| "playback was automatically switched off".into())
    }

    pub fn load_playback(
        &mut self,
        number: u16,
        cue_number: f64,
    ) -> Result<&ActivePlayback, String> {
        let id = self.cue_list_for(number)?;
        let cue = self.cue_lists[&id]
            .cues
            .iter()
            .find(|cue| cue.number == cue_number)
            .ok_or("cue does not exist")?;
        let key = PlaybackKey::Number(number);
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: Some(number),
            cue_list_id: id,
            cue_index: 0,
            previous_index: None,
            paused: false,
            activated_at: self.clock.now(),
            paused_at: None,
            master: 0.0,
            fader_position: 0.0,
            fader_pickup_required: false,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: false,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            transition_fade_fallback_millis: None,
            manual_xfade_position: 0.0,
            manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
            manual_xfade_from_index: None,
            manual_xfade_to_index: None,
            manual_xfade_progress: 0.0,
            tracking_wrap: false,
            current_cue_id: None,
            current_cue_number: None,
            deleted_cue_hold: None,
            deleted_cue_transition_source: None,
            loaded_cue_id: None,
            loaded_cue_number: None,
        });
        playback.playback_number = Some(number);
        playback.loaded_cue_id = Some(cue.id);
        playback.loaded_cue_number = Some(cue.number);
        Ok(playback)
    }

    pub fn on(&mut self, number: u16) -> Result<(), String> {
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
        }
        let should_restart_first = self.active.get(&key).is_some_and(|active| {
            if active.enabled {
                return false;
            }
            if self.cue_lists[&id].restart_mode == RestartMode::FirstCue {
                return true;
            }
            match active.current_cue_id {
                Some(current_id) => !self.cue_lists[&id]
                    .cues
                    .iter()
                    .any(|cue| cue.id == current_id),
                None => !active.current_cue_number.is_some_and(|current_number| {
                    self.cue_lists[&id]
                        .cues
                        .iter()
                        .any(|cue| cue.number == current_number)
                }),
            }
        });
        if should_restart_first {
            let active = self.active.get_mut(&key).unwrap();
            active.previous_index = None;
            active.cue_index = 0;
            active.current_cue_id = Some(self.cue_lists[&id].cues[0].id);
            active.current_cue_number = Some(self.cue_lists[&id].cues[0].number);
            active.deleted_cue_hold = None;
            active.deleted_cue_transition_source = None;
            active.activated_at = self.clock.now();
        }
        let active = self.active.get_mut(&key).unwrap();
        active.playback_number = Some(number);
        active.master = 1.0;
        active.enabled = true;
        active.temporary = false;
        active.fader_pickup_required = false;
        active.master_transition = None;
        active.deleted_cue_transition_source = None;
        reset_manual_transition(active);
        self.auto_off_overwritten();
        Ok(())
    }

    pub fn off(&mut self, number: u16) -> Result<bool, String> {
        self.cue_list_for(number)?;
        Ok(self
            .active
            .get_mut(&PlaybackKey::Number(number))
            .map(|playback| {
                let was = playback.enabled;
                playback.enabled = false;
                playback.flash = false;
                playback.fader_pickup_required = playback.fader_position > 0.0;
                playback.master_transition = None;
                playback.deleted_cue_hold = None;
                playback.deleted_cue_transition_source = None;
                playback.loaded_cue_id = None;
                playback.loaded_cue_number = None;
                was
            })
            .unwrap_or(false))
    }
    pub fn toggle(&mut self, number: u16) -> Result<bool, String> {
        self.cue_list_for(number)?;
        if self
            .active
            .get(&PlaybackKey::Number(number))
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
        self.set_master_inner(number, value, false)
    }

    /// Set the authoritative level through a virtual fader supplied by a remote control
    /// protocol. Faderless/button-only layouts intentionally have no local fader, but their
    /// playback master remains a valid runtime control and feedback source.
    pub fn set_virtual_master(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_master_inner(number, value, true)
    }

    fn set_master_inner(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback master must be within 0-1".into());
        }
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?;
        if !definition.has_fader && !allow_faderless {
            return Err("playback does not have a fader".into());
        }
        match definition.fader {
            PlaybackFaderMode::Temp => return self.set_temp_fader(number, value),
            PlaybackFaderMode::XFade => {
                return self.set_manual_xfade_inner(number, value, allow_faderless);
            }
            PlaybackFaderMode::Master => {}
            _ => return Err("fader mode is not handled by the Cuelist engine".into()),
        }
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if let Some(active) = self.active.get_mut(&key) {
            active.fader_position = value;
            if active.fader_pickup_required {
                if value == 0.0 {
                    active.fader_pickup_required = false;
                    active.master = 0.0;
                }
                return Ok(());
            }
        }
        if value > 0.0 && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
        }
        if let Some(active) = self.active.get_mut(&key) {
            active.playback_number = Some(number);
            active.master = value;
            active.fader_position = value;
            active.master_transition = None;
            active.temporary = false;
            if value > 0.0 {
                active.enabled = true;
            }
        }
        self.auto_off_overwritten();
        Ok(())
    }
    pub fn set_flash(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.cue_list_for(number)?;
        if pressed {
            let playback = self.temporary_playback(number, 1.0, true)?;
            self.temporary
                .insert((number, TemporaryPlaybackKind::Flash), playback);
        } else {
            let released = self
                .temporary
                .remove(&(number, TemporaryPlaybackKind::Flash));
            if self.definitions[&number].flash_release == FlashReleaseMode::ReleaseIntensityOnly
                && let Some(mut released) = released
            {
                let key = PlaybackKey::Number(number);
                let active = self.active.entry(key).or_insert_with(|| {
                    released.temporary = false;
                    released.flash = false;
                    released
                });
                active.enabled = true;
                active.master = 0.0;
                active.flash = false;
                active.temporary = false;
                active.flash_restore_off = false;
            }
        }
        Ok(())
    }

    fn temporary_playback(
        &self,
        number: u16,
        master: f32,
        flash: bool,
    ) -> Result<ActivePlayback, String> {
        let cue_list_id = self.cue_list_for(number)?;
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let now = self.clock.now();
        let mut playback = self
            .active
            .get(&PlaybackKey::Number(number))
            .cloned()
            .unwrap_or_else(|| new_active_playback(Some(number), cue_list, now, master, true));
        playback.playback_number = Some(number);
        playback.enabled = true;
        playback.temporary = true;
        playback.flash = flash;
        playback.master = master;
        playback.fader_position = master;
        playback.fader_pickup_required = false;
        playback.master_transition = None;
        playback.activated_at = now + ChronoDuration::microseconds(1);
        playback.paused = false;
        playback.paused_at = None;
        playback.previous_index = None;
        playback.transition_timing_bypassed = true;
        playback.transition_fade_fallback_millis = None;
        playback.manual_xfade_from_index = None;
        playback.manual_xfade_to_index = None;
        playback.manual_xfade_progress = 0.0;
        Ok(playback)
    }

    pub fn toggle_temp(&mut self, number: u16) -> Result<bool, String> {
        let key = (number, TemporaryPlaybackKind::TempButton);
        if self.temporary.remove(&key).is_some() {
            return Ok(false);
        }
        let playback = self.temporary_playback(number, 1.0, false)?;
        self.temporary.insert(key, playback);
        Ok(true)
    }

    pub fn set_temp_button(&mut self, number: u16, active: bool) -> Result<(), String> {
        let key = (number, TemporaryPlaybackKind::TempButton);
        if active {
            if !self.temporary.contains_key(&key) {
                let playback = self.temporary_playback(number, 1.0, false)?;
                self.temporary.insert(key, playback);
            }
        } else {
            self.cue_list_for(number)?;
            self.temporary.remove(&key);
        }
        Ok(())
    }

    pub fn set_temp_fader(&mut self, number: u16, value: f32) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("playback Temp fader must be within 0-1".into());
        }
        let key = (number, TemporaryPlaybackKind::TempFader);
        if value == 0.0 {
            self.temporary.remove(&key);
            return Ok(());
        }
        if let Some(playback) = self.temporary.get_mut(&key) {
            playback.master = value;
            playback.fader_position = value;
        } else {
            let playback = self.temporary_playback(number, value, false)?;
            self.temporary.insert(key, playback);
        }
        Ok(())
    }

    pub fn set_swap(&mut self, number: u16, pressed: bool) -> Result<(), String> {
        self.cue_list_for(number)?;
        let key = (number, TemporaryPlaybackKind::Swap);
        if pressed {
            let playback = self.temporary_playback(number, 1.0, true)?;
            self.temporary.insert(key, playback);
            self.swap_held.insert(number);
        } else {
            let released = self.temporary.remove(&key);
            self.swap_held.remove(&number);
            if self.definitions[&number].flash_release == FlashReleaseMode::ReleaseIntensityOnly
                && let Some(mut released) = released
            {
                let active = self
                    .active
                    .entry(PlaybackKey::Number(number))
                    .or_insert_with(|| {
                        released.temporary = false;
                        released.flash = false;
                        released
                    });
                active.enabled = true;
                active.master = 0.0;
                active.temporary = false;
                active.flash = false;
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
        if button > self.definitions[&number].button_count {
            return Err("button is not present on this playback".into());
        }
        if !pressed
            && !matches!(
                action,
                PlaybackButtonAction::Flash | PlaybackButtonAction::Swap
            )
        {
            return Ok(());
        }
        match action {
            PlaybackButtonAction::On => self.on(number),
            PlaybackButtonAction::Off => self.off(number).map(|_| ()),
            PlaybackButtonAction::Toggle => self.toggle(number).map(|_| ()),
            PlaybackButtonAction::Go => self.go_playback(number).map(|_| ()),
            PlaybackButtonAction::GoMinus => self.back_playback(number).map(|_| ()),
            PlaybackButtonAction::FastForward => self.fast_forward_playback(number).map(|_| ()),
            PlaybackButtonAction::FastRewind => self.fast_rewind_playback(number).map(|_| ()),
            PlaybackButtonAction::Flash => self.set_flash(number, pressed),
            PlaybackButtonAction::Temp => {
                if pressed {
                    self.toggle_temp(number).map(|_| ())
                } else {
                    Ok(())
                }
            }
            PlaybackButtonAction::Swap => self.set_swap(number, pressed),
            PlaybackButtonAction::Select
            | PlaybackButtonAction::SelectContents
            | PlaybackButtonAction::SelectDereferenced
            | PlaybackButtonAction::Learn
            | PlaybackButtonAction::Double
            | PlaybackButtonAction::Half
            | PlaybackButtonAction::Pause
            | PlaybackButtonAction::Blackout
            | PlaybackButtonAction::PauseDynamics => Ok(()),
            PlaybackButtonAction::None => Ok(()),
        }
    }

    pub fn set_manual_xfade(&mut self, number: u16, value: f32) -> Result<(), String> {
        self.set_manual_xfade_inner(number, value, false)
    }

    fn set_manual_xfade_inner(
        &mut self,
        number: u16,
        value: f32,
        allow_faderless: bool,
    ) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("manual X-fade must be within 0-1".into());
        }
        let definition = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?;
        if definition.fader != PlaybackFaderMode::XFade
            || (!definition.has_fader && !allow_faderless)
        {
            return Err("playback is not configured for manual X-fade".into());
        }
        let cue_list_id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if !self.active.contains_key(&key) {
            let cue_list = self
                .cue_lists
                .get(&cue_list_id)
                .ok_or("playback cue list does not exist")?;
            self.active.insert(
                key,
                new_active_playback(Some(number), cue_list, self.clock.now(), 1.0, true),
            );
        }
        let cue_list = self
            .cue_lists
            .get(&cue_list_id)
            .ok_or("playback cue list does not exist")?;
        let active = self.active.get_mut(&key).expect("X-fade playback exists");
        active.playback_number = Some(number);
        active.enabled = true;
        active.fader_position = value;
        active.manual_xfade_position = value;
        let progress = match active.manual_xfade_direction {
            ManualXFadeDirection::TowardsHigh => value,
            ManualXFadeDirection::TowardsLow => 1.0 - value,
        };
        if active.manual_xfade_from_index.is_none() && progress > 0.0 {
            let next = if active.cue_index + 1 < cue_list.cues.len() {
                Some(active.cue_index + 1)
            } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                Some(0)
            } else {
                None
            };
            if let Some(next) = next {
                active.manual_xfade_from_index = Some(active.cue_index);
                active.manual_xfade_to_index = Some(next);
                active.transition_timing_bypassed = false;
            }
        }
        if active.manual_xfade_from_index.is_none() {
            active.manual_xfade_progress = 0.0;
            return Ok(());
        }
        active.manual_xfade_progress = progress.clamp(0.0, 1.0);
        if progress >= 1.0 {
            let target = active
                .manual_xfade_to_index
                .expect("manual X-fade target accompanies source");
            active.cue_index = target;
            active.current_cue_id = Some(cue_list.cues[target].id);
            active.current_cue_number = Some(cue_list.cues[target].number);
            active.previous_index = None;
            active.transition_timing_bypassed = true;
            active.tracking_wrap =
                target == 0 && cue_list.effective_wrap_mode() == WrapMode::Tracking;
            active.activated_at = self.clock.now();
            active.manual_xfade_from_index = None;
            active.manual_xfade_to_index = None;
            active.manual_xfade_progress = 0.0;
            active.manual_xfade_direction = match active.manual_xfade_direction {
                ManualXFadeDirection::TowardsHigh => ManualXFadeDirection::TowardsLow,
                ManualXFadeDirection::TowardsLow => ManualXFadeDirection::TowardsHigh,
            };
        }
        Ok(())
    }

    pub fn xfade(&mut self, number: u16, on: bool) -> Result<(), String> {
        let duration = self
            .definitions
            .get(&number)
            .ok_or("playback does not exist")?
            .xfade_millis;
        let id = self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if on && !self.active.contains_key(&key) {
            self.go_at_key(key, id, self.clock.now())?;
            self.active.get_mut(&key).unwrap().master = 0.0;
        }
        let active = self.active.get_mut(&key).ok_or("playback is not active")?;
        if on {
            active.enabled = true;
        }
        active.playback_number = Some(number);
        if duration == 0 {
            active.master = if on { 1.0 } else { 0.0 };
            if !on {
                self.active.get_mut(&key).unwrap().enabled = false;
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

    /// Applies the timing envelope owned by one atomic Preload GO after the retained action verb
    /// has executed against the playback's then-current state. This does not rewrite Cue data:
    /// explicit Cue/attribute timing still wins, while a zero Cue time falls back to Programmer
    /// Fade for this transition only.
    pub fn apply_preload_timing(
        &mut self,
        number: u16,
        action: &str,
        started_at: DateTime<Utc>,
        fallback_millis: u64,
        previous: Option<(bool, f32)>,
    ) -> Result<(), String> {
        self.cue_list_for(number)?;
        let key = PlaybackKey::Number(number);
        if let Some(playback) = self.active.get_mut(&key) {
            if playback.enabled && matches!(action, "go" | "go-minus" | "on" | "toggle") {
                playback.activated_at = started_at;
                playback.transition_timing_bypassed = false;
                playback.transition_fade_fallback_millis = Some(fallback_millis);
            }

            match (previous, playback.enabled) {
                (Some((false, _)), true)
                    if matches!(action, "go" | "on" | "toggle") && fallback_millis > 0 =>
                {
                    let target = playback.master;
                    playback.master = 0.0;
                    playback.master_transition = Some(PlaybackMasterTransition {
                        from: 0.0,
                        to: target,
                        started_at,
                        duration_millis: fallback_millis,
                        release_after: false,
                    });
                }
                (Some((true, previous_master)), false)
                    if matches!(action, "off" | "toggle") && fallback_millis > 0 =>
                {
                    playback.enabled = true;
                    playback.master = previous_master;
                    playback.master_transition = Some(PlaybackMasterTransition {
                        from: previous_master,
                        to: 0.0,
                        started_at,
                        duration_millis: fallback_millis,
                        release_after: true,
                    });
                }
                _ => {}
            }
        }

        if action == "temp-on"
            && let Some(playback) = self
                .temporary
                .get_mut(&(number, TemporaryPlaybackKind::TempButton))
        {
            let target = playback.master;
            playback.activated_at = started_at;
            playback.transition_timing_bypassed = false;
            playback.transition_fade_fallback_millis = Some(fallback_millis);
            if fallback_millis > 0 {
                playback.master = 0.0;
                playback.master_transition = Some(PlaybackMasterTransition {
                    from: 0.0,
                    to: target,
                    started_at,
                    duration_millis: fallback_millis,
                    release_after: false,
                });
            }
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
            _ => Err("operation is not available for this playback function".into()),
        }
    }

    fn key_for_cue_list(&self, id: CueListId) -> Result<PlaybackKey, String> {
        let assigned = self
            .definitions
            .values()
            .filter_map(|definition| match definition.target {
                PlaybackTarget::CueList { cue_list_id } if cue_list_id == id => {
                    Some(definition.number)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        match assigned.as_slice() {
            [] => Ok(PlaybackKey::CueList(id)),
            [number] => Ok(PlaybackKey::Number(*number)),
            _ => Err(
                "cue list is assigned to multiple playbacks; address a concrete playback".into(),
            ),
        }
    }

    pub fn go(&mut self, id: CueListId) -> Result<&ActivePlayback, String> {
        self.go_at(id, self.clock.now())
    }

    pub fn go_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<&ActivePlayback, String> {
        let key = self.key_for_cue_list(id)?;
        self.go_at_key(key, id, now)
    }

    fn go_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let cue_list = self.cue_lists.get(&id).ok_or("cue list does not exist")?;
        let playback = match self.active.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(ActivePlayback {
                playback_number: None,
                cue_list_id: id,
                cue_index: 0,
                previous_index: None,
                paused: false,
                activated_at: now,
                paused_at: None,
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                flash: false,
                master_transition: None,
                temporary: false,
                enabled: true,
                flash_restore_off: false,
                transition_timing_bypassed: false,
                transition_fade_fallback_millis: None,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_from_index: None,
                manual_xfade_to_index: None,
                manual_xfade_progress: 0.0,
                tracking_wrap: false,
                current_cue_id: Some(cue_list.cues[0].id),
                current_cue_number: Some(cue_list.cues[0].number),
                deleted_cue_hold: None,
                deleted_cue_transition_source: None,
                loaded_cue_id: None,
                loaded_cue_number: None,
            }),
            std::collections::hash_map::Entry::Occupied(entry) => {
                let playback = entry.into_mut();
                if let Some(loaded) = playback.loaded_cue_id.take() {
                    let index = cue_list
                        .cues
                        .iter()
                        .position(|cue| cue.id == loaded)
                        .ok_or("loaded cue no longer exists")?;
                    if playback.enabled && playback.current_cue_number.is_some() {
                        playback.previous_index = Some(playback.cue_index);
                    } else {
                        playback.previous_index = None;
                    }
                    playback.cue_index = index;
                    playback.current_cue_id = Some(cue_list.cues[index].id);
                    playback.current_cue_number = Some(cue_list.cues[index].number);
                    playback.loaded_cue_number = None;
                    playback.deleted_cue_transition_source = None;
                    playback.tracking_wrap = false;
                    playback.paused = false;
                    playback.paused_at = None;
                    playback.activated_at = now;
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                if let Some(hold) = playback.deleted_cue_hold.take() {
                    if let Some(next) = hold.next_number
                        && let Some(index) = cue_list.cues.iter().position(|cue| cue.number == next)
                    {
                        playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                        playback.previous_index = None;
                        playback.cue_index = index;
                        playback.current_cue_id = Some(cue_list.cues[index].id);
                        playback.current_cue_number = Some(next);
                        playback.tracking_wrap = false;
                        playback.activated_at = now;
                    } else {
                        playback.deleted_cue_hold = Some(hold);
                    }
                    reset_manual_transition(playback);
                    return Ok(playback);
                }
                playback.deleted_cue_transition_source = None;
                let resumed = playback.paused;
                if playback.paused {
                    if let Some(paused_at) = playback.paused_at.take() {
                        playback.activated_at += now - paused_at;
                    }
                    playback.paused = false;
                } else if playback.cue_index + 1 < cue_list.cues.len() {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index += 1;
                } else if cue_list.effective_wrap_mode() != WrapMode::Off {
                    playback.previous_index = Some(playback.cue_index);
                    playback.cue_index = 0;
                    playback.tracking_wrap = cue_list.effective_wrap_mode() == WrapMode::Tracking;
                }
                if !resumed {
                    playback.activated_at = now;
                }
                playback.current_cue_number = Some(cue_list.cues[playback.cue_index].number);
                playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
                playback
            }
        };
        reset_manual_transition(playback);
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
        let key = self.key_for_cue_list(id)?;
        self.jump_at_key(key, id, cue_number, now)
    }

    fn jump_at_key(
        &mut self,
        key: PlaybackKey,
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
        let playback = self.active.entry(key).or_insert(ActivePlayback {
            playback_number: None,
            cue_list_id: id,
            cue_index: index,
            previous_index: None,
            paused: false,
            activated_at: now,
            paused_at: None,
            master: 1.0,
            fader_position: 1.0,
            fader_pickup_required: false,
            flash: false,
            master_transition: None,
            temporary: false,
            enabled: true,
            flash_restore_off: false,
            transition_timing_bypassed: false,
            transition_fade_fallback_millis: None,
            manual_xfade_position: 0.0,
            manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
            manual_xfade_from_index: None,
            manual_xfade_to_index: None,
            manual_xfade_progress: 0.0,
            tracking_wrap: false,
            current_cue_id: Some(cue_list.cues[index].id),
            current_cue_number: Some(cue_list.cues[index].number),
            deleted_cue_hold: None,
            deleted_cue_transition_source: None,
            loaded_cue_id: None,
            loaded_cue_number: None,
        });
        if playback.cue_index != index {
            playback.previous_index = Some(playback.cue_index);
        }
        playback.cue_index = index;
        playback.current_cue_id = Some(cue_list.cues[index].id);
        playback.current_cue_number = Some(cue_number);
        playback.deleted_cue_hold = None;
        playback.deleted_cue_transition_source = None;
        playback.loaded_cue_id = None;
        playback.loaded_cue_number = None;
        playback.tracking_wrap = false;
        playback.paused = false;
        playback.paused_at = None;
        playback.activated_at = now;
        reset_manual_transition(playback);
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
        let key = self.key_for_cue_list(id)?;
        self.back_at_key(key, id, now)
    }
    fn back_at_key(
        &mut self,
        key: PlaybackKey,
        id: CueListId,
        now: DateTime<Utc>,
    ) -> Result<&ActivePlayback, String> {
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        reset_manual_transition(playback);
        if let Some(hold) = playback.deleted_cue_hold.take() {
            if let Some(previous) = hold.previous_number
                && let Some(index) = self.cue_lists[&id]
                    .cues
                    .iter()
                    .position(|cue| cue.number == previous)
            {
                playback.deleted_cue_transition_source = Some(hold.contributions.clone());
                playback.previous_index = None;
                playback.cue_index = index;
                playback.current_cue_id = Some(self.cue_lists[&id].cues[index].id);
                playback.current_cue_number = Some(previous);
                playback.tracking_wrap = false;
                playback.activated_at = now;
                playback.paused = false;
                playback.paused_at = None;
            } else {
                playback.deleted_cue_hold = Some(hold);
            }
            return Ok(playback);
        }
        playback.deleted_cue_transition_source = None;
        playback.previous_index = Some(playback.cue_index);
        playback.cue_index = playback.cue_index.saturating_sub(1);
        playback.current_cue_id = Some(self.cue_lists[&id].cues[playback.cue_index].id);
        playback.current_cue_number = Some(self.cue_lists[&id].cues[playback.cue_index].number);
        playback.tracking_wrap = false;
        playback.activated_at = now;
        playback.paused = false;
        playback.paused_at = None;
        Ok(playback)
    }
    pub fn pause(&mut self, id: CueListId) -> Result<(), String> {
        self.pause_at(id, self.clock.now())
    }
    pub fn pause_playback(&mut self, number: u16) -> Result<(), String> {
        let now = self.clock.now();
        let key = PlaybackKey::Number(number);
        let playback = self.active.get_mut(&key).ok_or("playback is not active")?;
        if !playback.paused {
            playback.paused = true;
            playback.paused_at = Some(now);
        }
        Ok(())
    }
    pub fn pause_at(&mut self, id: CueListId, now: DateTime<Utc>) -> Result<(), String> {
        let key = self.key_for_cue_list(id)?;
        let playback = self.active.get_mut(&key).ok_or("cue list is not active")?;
        if !playback.paused {
            playback.paused = true;
            playback.paused_at = Some(now);
        }
        Ok(())
    }
    pub fn release(&mut self, id: CueListId) -> bool {
        self.key_for_cue_list(id)
            .ok()
            .is_some_and(|key| self.active.remove(&key).is_some())
    }
    pub fn active(&self) -> Vec<ActivePlayback> {
        self.active
            .values()
            .filter(|playback| playback.enabled)
            .chain(self.temporary.values())
            .cloned()
            .collect()
    }
    pub fn runtime(&self) -> Vec<ActivePlayback> {
        let mut runtime = self.active.values().cloned().collect::<Vec<_>>();
        runtime.sort_by_key(|playback| playback.playback_number.unwrap_or(u16::MAX));
        runtime
    }
    pub fn runtime_status(&self) -> Vec<PlaybackRuntimeStatus> {
        let mut runtime = self.runtime();
        for ((number, _), temporary) in &self.temporary {
            if runtime
                .iter()
                .any(|playback| playback.playback_number == Some(*number))
            {
                continue;
            }
            let mut inactive = temporary.clone();
            inactive.enabled = false;
            inactive.master = 0.0;
            inactive.temporary = false;
            inactive.flash = false;
            runtime.push(inactive);
        }
        runtime.sort_by_key(|playback| playback.playback_number.unwrap_or(u16::MAX));
        runtime
            .into_iter()
            .map(|mut playback| {
                let number = playback.playback_number;
                let temporary_master = number
                    .map(|number| {
                        self.temporary
                            .iter()
                            .filter(|((candidate, _), _)| *candidate == number)
                            .map(|(_, playback)| playback.master)
                            .fold(0.0_f32, f32::max)
                    })
                    .unwrap_or(0.0);
                let temporary_active = temporary_master > 0.0
                    || number.is_some_and(|number| {
                        self.temporary
                            .keys()
                            .any(|(candidate, _)| *candidate == number)
                    });
                let swap_active = number.is_some_and(|number| self.swap_held.contains(&number));
                playback.flash = number.is_some_and(|number| {
                    self.temporary
                        .contains_key(&(number, TemporaryPlaybackKind::Flash))
                });
                let cue_list = self.cue_lists.get(&playback.cue_list_id);
                let normal = cue_list.and_then(|list| {
                    if let Some(hold) = &playback.deleted_cue_hold {
                        return hold
                            .next_number
                            .and_then(|number| list.cues.iter().find(|cue| cue.number == number));
                    }
                    if playback.current_cue_id.is_none() && playback.current_cue_number.is_none() {
                        return list.cues.first();
                    }
                    let index = playback
                        .current_cue_id
                        .and_then(|id| list.cues.iter().position(|cue| cue.id == id))
                        .or_else(|| {
                            playback.current_cue_number.and_then(|number| {
                                list.cues.iter().position(|cue| cue.number == number)
                            })
                        })
                        .unwrap_or(playback.cue_index.min(list.cues.len().saturating_sub(1)));
                    list.cues.get(index + 1).or_else(|| {
                        (list.effective_wrap_mode() != WrapMode::Off)
                            .then(|| list.cues.first())
                            .flatten()
                    })
                });
                let loaded = cue_list.and_then(|list| {
                    playback
                        .loaded_cue_id
                        .and_then(|id| list.cues.iter().find(|cue| cue.id == id))
                });
                let effective = loaded.or(normal);
                PlaybackRuntimeStatus {
                    normal_next_cue_id: normal.map(|cue| cue.id),
                    normal_next_cue_number: normal.map(|cue| cue.number),
                    effective_next_cue_id: effective.map(|cue| cue.id),
                    effective_next_cue_number: effective.map(|cue| cue.number),
                    effective_next_is_loaded: loaded.is_some(),
                    temporary_active,
                    temporary_master,
                    swap_active,
                    playback,
                }
            })
            .collect()
    }

    /// Reconstructs the next eventual lit Position state for every fixture whose current tracked
    /// state is dark. Look-ahead deliberately stops at the end of the Cuelist; wrap behavior needs
    /// separate boundary tests before it may cross that edge.
    pub fn move_in_black_candidates(&self) -> Vec<MoveInBlackCandidate> {
        let mut candidates = Vec::new();
        for playback in self.active.values().filter(|playback| playback.enabled) {
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(current_index) = playback
                .current_cue_id
                .and_then(|id| cue_list.cues.iter().position(|cue| cue.id == id))
                .or_else(|| {
                    playback.current_cue_number.and_then(|number| {
                        cue_list.cues.iter().position(|cue| cue.number == number)
                    })
                })
                .or_else(|| {
                    cue_list
                        .cues
                        .get(playback.cue_index)
                        .map(|_| playback.cue_index)
                })
            else {
                continue;
            };
            let current_cue = &cue_list.cues[current_index];
            let current_state = cue_list.state_at_index(current_index);
            let fixtures = current_state
                .keys()
                .map(|(fixture_id, _)| *fixture_id)
                .collect::<HashSet<_>>();
            for fixture_id in fixtures {
                let current_intensity = current_state
                    .iter()
                    .filter(|((candidate, attribute), _)| {
                        *candidate == fixture_id && attribute.is_intensity()
                    })
                    .filter_map(|(_, value)| value.normalized())
                    .fold(0.0_f32, f32::max);
                if current_intensity != 0.0 {
                    continue;
                }
                let Some((target_index, target_state)) = cue_list
                    .cues
                    .iter()
                    .enumerate()
                    .skip(current_index + 1)
                    .find_map(|(index, _)| {
                        let state = cue_list.state_at_index(index);
                        let intensity = state
                            .iter()
                            .filter(|((candidate, attribute), _)| {
                                *candidate == fixture_id && attribute.is_intensity()
                            })
                            .filter_map(|(_, value)| value.normalized())
                            .fold(0.0_f32, f32::max);
                        (intensity > 0.0).then_some((index, state))
                    })
                else {
                    continue;
                };
                let target_cue = &cue_list.cues[target_index];
                let cue_fade_millis = if cue_list.disable_cue_timing {
                    0
                } else if cue_list.mode == CueListMode::Chaser {
                    effective_chaser_xfade_millis(cue_list, &self.speed_groups_bpm)
                } else if target_cue.fade_millis == 0 {
                    self.sequence_master_fade_millis
                } else {
                    target_cue.fade_millis
                };
                let timing = target_cue
                    .changes
                    .iter()
                    .filter(|change| change.fixture_id == fixture_id)
                    .map(|change| (change.attribute.clone(), change.fade_millis))
                    .collect::<HashMap<_, _>>();
                let position_attributes = current_state
                    .keys()
                    .chain(target_state.keys())
                    .filter(|(candidate, attribute)| {
                        *candidate == fixture_id && attribute.is_position()
                    })
                    .map(|(_, attribute)| attribute.clone())
                    .collect::<HashSet<_>>();
                let mut values = position_attributes
                    .into_iter()
                    .filter_map(|attribute| {
                        let current = current_state
                            .get(&(fixture_id, attribute.clone()))
                            .cloned()
                            .unwrap_or(AttributeValue::Normalized(0.0));
                        let target = target_state
                            .get(&(fixture_id, attribute.clone()))
                            .cloned()
                            .unwrap_or(AttributeValue::Normalized(0.0));
                        if current == target {
                            return None;
                        }
                        let fade_millis = if cue_list.disable_cue_timing {
                            0
                        } else if cue_list.force_cue_timing {
                            cue_fade_millis
                        } else {
                            timing
                                .get(&attribute)
                                .copied()
                                .flatten()
                                .unwrap_or(cue_fade_millis)
                        };
                        Some(MoveInBlackTargetValue {
                            attribute,
                            current,
                            target,
                            fade_millis,
                        })
                    })
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| left.attribute.cmp(&right.attribute));
                if !values.is_empty() {
                    candidates.push(MoveInBlackCandidate {
                        playback_number: playback.playback_number,
                        cue_list_id: cue_list.id,
                        current_cue_id: current_cue.id,
                        current_cue_number: current_cue.number,
                        target_cue_id: target_cue.id,
                        target_cue_number: target_cue.number,
                        fixture_id,
                        priority: cue_list.priority,
                        values,
                    });
                }
            }
        }
        candidates.sort_by(|left, right| {
            left.playback_number
                .cmp(&right.playback_number)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
        });
        candidates
    }

    fn auto_off_overwritten(&mut self) {
        let full: Vec<_> = self
            .active
            .iter()
            .filter(|(_, p)| p.enabled && p.master >= 1.0 && !p.flash && !p.temporary)
            .map(|(key, p)| (*key, p.cue_list_id, p.activated_at))
            .collect();
        let mut release = Vec::new();
        for (own_key, playback) in &self.active {
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
                full.iter().any(|(other_key, other, changed)| {
                    if other_key == own_key {
                        return false;
                    }
                    let other_list = &self.cue_lists[other];
                    let Some(other_value) = other_list
                        .state_at_index(self.active[other_key].cue_index)
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
                release.push(*own_key);
            }
        }
        for key in release {
            if let Some(playback) = self.active.get_mut(&key) {
                playback.enabled = false;
            }
        }
    }
    pub fn restore_active(&mut self, playbacks: impl IntoIterator<Item = ActivePlayback>) {
        for mut playback in playbacks {
            if let Some(number) = playback.playback_number
                && !self.definitions.get(&number).is_some_and(|definition| {
                    matches!(
                        definition.target,
                        PlaybackTarget::CueList { cue_list_id }
                            if cue_list_id == playback.cue_list_id
                    )
                })
            {
                continue;
            }
            let Some(cue_list) = self.cue_lists.get(&playback.cue_list_id) else {
                continue;
            };
            let Some(last) = cue_list.cues.len().checked_sub(1) else {
                continue;
            };
            if playback.deleted_cue_hold.is_none()
                && let Some(index) = playback
                    .current_cue_id
                    .and_then(|id| cue_list.cues.iter().position(|cue| cue.id == id))
                    .or_else(|| {
                        playback.current_cue_number.and_then(|number| {
                            cue_list.cues.iter().position(|cue| cue.number == number)
                        })
                    })
            {
                playback.cue_index = index;
                playback.current_cue_id = Some(cue_list.cues[index].id);
                playback.current_cue_number = Some(cue_list.cues[index].number);
            } else {
                playback.cue_index = playback.cue_index.min(last);
            }
            playback.previous_index = playback.previous_index.map(|index| index.min(last));
            playback.manual_xfade_from_index = playback
                .manual_xfade_from_index
                .map(|index| index.min(last));
            playback.manual_xfade_to_index =
                playback.manual_xfade_to_index.map(|index| index.min(last));
            playback.manual_xfade_position = playback.manual_xfade_position.clamp(0.0, 1.0);
            playback.manual_xfade_progress = playback.manual_xfade_progress.clamp(0.0, 1.0);
            if let Some(loaded) = playback.loaded_cue_id
                && let Some(cue) = cue_list.cues.iter().find(|cue| cue.id == loaded)
            {
                playback.loaded_cue_number = Some(cue.number);
            } else if playback.loaded_cue_id.is_some() {
                playback.loaded_cue_id = None;
                playback.loaded_cue_number = None;
            }
            let key = playback
                .playback_number
                .map(PlaybackKey::Number)
                .unwrap_or(PlaybackKey::CueList(playback.cue_list_id));
            self.active.insert(key, playback);
        }
    }

    pub fn active_for_snapshot(
        &self,
        next_lists: &[CueList],
        now: DateTime<Utc>,
    ) -> Vec<ActivePlayback> {
        self.active
            .iter()
            .map(|(key, value)| {
                let mut playback = value.clone();
                let Some(old_list) = self.cue_lists.get(&playback.cue_list_id) else {
                    return playback;
                };
                let infer_legacy_current = playback.enabled
                    || playback.current_cue_number.is_some()
                    || playback.current_cue_id.is_some();
                let current_id = playback.current_cue_id.or_else(|| {
                    infer_legacy_current
                        .then(|| old_list.cues.get(playback.cue_index).map(|cue| cue.id))
                        .flatten()
                });
                playback.current_cue_id = current_id;
                let number = playback.current_cue_number.or_else(|| {
                    infer_legacy_current
                        .then(|| old_list.cues.get(playback.cue_index).map(|cue| cue.number))
                        .flatten()
                });
                playback.current_cue_number = number;
                let Some(number) = number else {
                    return playback;
                };
                let Some(next) = next_lists
                    .iter()
                    .find(|list| list.id == playback.cue_list_id)
                else {
                    return playback;
                };
                if let Some(index) =
                    current_id.and_then(|id| next.cues.iter().position(|cue| cue.id == id))
                {
                    playback.cue_index = index;
                    playback.current_cue_number = Some(next.cues[index].number);
                    return playback;
                }
                if !playback.enabled {
                    playback.cue_index = 0;
                    playback.previous_index = None;
                    playback.current_cue_id = None;
                    playback.current_cue_number = None;
                    playback.deleted_cue_hold = None;
                    playback.deleted_cue_transition_source = None;
                    return playback;
                }
                let previous_number = next
                    .cues
                    .iter()
                    .rfind(|cue| cue.number < number)
                    .map(|cue| cue.number);
                let next_number = next
                    .cues
                    .iter()
                    .find(|cue| cue.number > number)
                    .map(|cue| cue.number);
                let mut isolated = PlaybackEngine {
                    cue_lists: self.cue_lists.clone(),
                    active: HashMap::from([(*key, playback.clone())]),
                    temporary: HashMap::new(),
                    swap_held: HashSet::new(),
                    dynamics_paused_at: None,
                    speed_groups_bpm: self.speed_groups_bpm,
                    speed_groups_paused: self.speed_groups_paused,
                    sequence_master_fade_millis: self.sequence_master_fade_millis,
                    definitions: self.definitions.clone(),
                    clock: Arc::clone(&self.clock),
                };
                isolated.active.get_mut(key).unwrap().deleted_cue_hold = None;
                isolated
                    .active
                    .get_mut(key)
                    .unwrap()
                    .deleted_cue_transition_source = None;
                playback.deleted_cue_transition_source = None;
                playback.deleted_cue_hold = Some(DeletedCueHold {
                    deleted_number: number,
                    previous_number,
                    next_number,
                    contributions: isolated.contributions_at(now),
                });
                playback
            })
            .collect()
    }

    pub fn contributions(&self) -> Vec<TimedValue> {
        self.contributions_at(self.clock.now())
    }

    pub fn contributions_at(&self, now: DateTime<Utc>) -> Vec<TimedValue> {
        self.contributions_at_with_snap(now, |_, _| false)
    }

    pub fn contributions_at_with_snap(
        &self,
        now: DateTime<Utc>,
        is_snap: impl Fn(FixtureId, &AttributeKey) -> bool,
    ) -> Vec<TimedValue> {
        self.contributions_with_context_at(now, is_snap)
            .into_iter()
            .map(|contribution| contribution.value)
            .collect()
    }

    /// Resolve active Cue values while retaining the exact playback master which owns each
    /// contribution. The engine uses this metadata only after normal HTP/LTP arbitration.
    pub fn contributions_with_context_at(
        &self,
        now: DateTime<Utc>,
        is_snap: impl Fn(FixtureId, &AttributeKey) -> bool,
    ) -> Vec<PlaybackContribution> {
        let mut values = Vec::new();
        let dynamics_now = self.dynamics_paused_at.unwrap_or(now);
        let suppressed = |playback: &ActivePlayback| {
            let Some(number) = playback.playback_number else {
                return false;
            };
            self.swap_held.iter().any(|source| {
                *source != number
                    && !self
                        .definitions
                        .get(&number)
                        .is_some_and(|definition| definition.protect_from_swap)
            })
        };
        for playback in self
            .active
            .values()
            .chain(self.temporary.values())
            .filter(|playback| !suppressed(playback))
        {
            if !playback.enabled {
                continue;
            }
            let source = SequenceMasterSource {
                playback_number: playback.playback_number,
                cue_list_id: playback.cue_list_id,
                temporary: playback.temporary,
            };
            let sequence_master = if playback.flash {
                1.0
            } else {
                playback.master.clamp(0.0, 1.0)
            };
            let snap_sequence_master = if playback.flash {
                1.0
            } else {
                playback
                    .master_transition
                    .as_ref()
                    .map(|transition| transition.to)
                    .unwrap_or(playback.master)
                    .clamp(0.0, 1.0)
            };
            if let Some(hold) = &playback.deleted_cue_hold {
                values.extend(hold.contributions.iter().cloned().map(|value| {
                    let contribution_master = if is_snap(value.fixture_id, &value.attribute) {
                        snap_sequence_master
                    } else {
                        sequence_master
                    };
                    PlaybackContribution {
                        value,
                        sequence_master: contribution_master,
                        source,
                    }
                }));
                continue;
            }
            let cue_list = &self.cue_lists[&playback.cue_list_id];
            let target_index = playback.manual_xfade_to_index.unwrap_or(playback.cue_index);
            let target = if playback.tracking_wrap && playback.manual_xfade_to_index.is_none() {
                let mut state = cue_list.state_at_index(cue_list.cues.len() - 1);
                for cue in cue_list.cues.iter().take(target_index + 1) {
                    apply_changes(&mut state, &cue.changes);
                }
                state
            } else {
                cue_list.state_at_index(target_index)
            };
            let previous = if let Some(index) = playback.manual_xfade_from_index {
                cue_list.state_at_index(index)
            } else if let Some(source) = &playback.deleted_cue_transition_source {
                let intensity_scale = if playback.flash { 1.0 } else { playback.master };
                source
                    .iter()
                    .map(|timed| {
                        let value = if timed.attribute.is_intensity() {
                            timed
                                .value
                                .normalized()
                                .map(|level| {
                                    AttributeValue::Normalized(if intensity_scale > 0.0 {
                                        (level / intensity_scale).clamp(0.0, 1.0)
                                    } else {
                                        0.0
                                    })
                                })
                                .unwrap_or_else(|| timed.value.clone())
                        } else {
                            timed.value.clone()
                        };
                        ((timed.fixture_id, timed.attribute.clone()), value)
                    })
                    .collect()
            } else {
                playback
                    .previous_index
                    .map(|index| cue_list.state_at_index(index))
                    .unwrap_or_default()
            };
            let cue = &cue_list.cues[target_index];
            let effective_now = playback.paused_at.unwrap_or(dynamics_now);
            let elapsed = (effective_now - playback.activated_at)
                .num_milliseconds()
                .max(0) as u64;
            let cue_fade_millis = if cue_list.disable_cue_timing {
                0
            } else if cue_list.mode == CueListMode::Chaser {
                effective_chaser_xfade_millis(cue_list, &self.speed_groups_bpm)
            } else if cue_list.mode == CueListMode::Sequence && cue.fade_millis == 0 {
                playback
                    .transition_fade_fallback_millis
                    .unwrap_or(self.sequence_master_fade_millis)
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
                let (fade_millis, delay_millis) = if cue_list.disable_cue_timing {
                    (0, 0)
                } else if cue_list.force_cue_timing {
                    (cue_fade_millis, cue.delay_millis)
                } else {
                    (
                        fade_override.unwrap_or(cue_fade_millis),
                        delay_override.unwrap_or(cue.delay_millis),
                    )
                };
                let snap = is_snap(fixture_id, &attribute);
                let progress = if playback.manual_xfade_from_index.is_some() {
                    if snap {
                        1.0
                    } else {
                        playback.manual_xfade_progress
                    }
                } else if playback.transition_timing_bypassed {
                    1.0
                } else if elapsed < delay_millis {
                    0.0
                } else if snap || fade_millis == 0 {
                    1.0
                } else {
                    ((elapsed - delay_millis) as f32 / fade_millis as f32).clamp(0.0, 1.0)
                };
                let contribution_master = if snap {
                    snap_sequence_master
                } else {
                    sequence_master
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
                        .map(|level| AttributeValue::Normalized(level * contribution_master))
                        .unwrap_or(value)
                } else {
                    value
                };
                values.push(PlaybackContribution {
                    value: TimedValue {
                        fixture_id,
                        merge_mode: if attribute.is_intensity() {
                            if cue_list.intensity_priority_mode == IntensityPriorityMode::Htp {
                                MergeMode::Htp
                            } else {
                                MergeMode::Ltp
                            }
                        } else {
                            MergeMode::Ltp
                        },
                        attribute,
                        value,
                        priority: cue_list.priority,
                        changed_at: playback.activated_at,
                        programmer_order: 0,
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    },
                    sequence_master: contribution_master,
                    source,
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
                    let contribution_master = if is_snap(*fixture_id, &attribute_phaser.attribute) {
                        snap_sequence_master
                    } else {
                        sequence_master
                    };
                    let sampled = attribute_phaser.phaser.sample(
                        phaser_elapsed,
                        index,
                        attribute_phaser.fixture_ids.len(),
                    );
                    let base = target
                        .get(&(*fixture_id, attribute_phaser.attribute.clone()))
                        .and_then(AttributeValue::normalized)
                        .unwrap_or(0.0);
                    let mut level = match attribute_phaser.phaser.mode {
                        PhaserMode::Absolute => sampled,
                        PhaserMode::Relative => base + sampled,
                    }
                    .clamp(0.0, 1.0);
                    if attribute_phaser.attribute.is_intensity() {
                        level *= contribution_master;
                    }
                    values.push(PlaybackContribution {
                        value: TimedValue {
                            fixture_id: *fixture_id,
                            attribute: attribute_phaser.attribute.clone(),
                            value: AttributeValue::Normalized(level),
                            priority: cue_list.priority,
                            changed_at: playback.activated_at,
                            programmer_order: 0,
                            merge_mode: if attribute_phaser.attribute.is_intensity() {
                                if cue_list.intensity_priority_mode == IntensityPriorityMode::Htp {
                                    MergeMode::Htp
                                } else {
                                    MergeMode::Ltp
                                }
                            } else {
                                MergeMode::Ltp
                            },
                            fade: false,
                            fade_millis: None,
                            delay_millis: None,
                        },
                        sequence_master: contribution_master,
                        source,
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
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Off),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1_000,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_group: None,
            speed_multiplier: 1.0,
            cues,
        }
    }
    fn definition(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
        PlaybackDefinition {
            number,
            name: format!("Playback {number}"),
            target: PlaybackTarget::CueList { cue_list_id },
            buttons: [
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader: true,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: default_playback_color(),
            flash_release: FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }

    #[test]
    fn preload_transition_uses_one_timestamp_and_programmer_fade_only_as_fallback() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "intensity", 0.0));
        let mut second = Cue::new(2.0);
        second.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![first, second]);
        let id = cue_list.id;
        let started = chrono::DateTime::parse_from_rfc3339("2026-07-16T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let clock = Arc::new(light_core::ManualClock::new(started));
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 7_000);
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();

        let committed_at = started + ChronoDuration::milliseconds(750);
        clock.set(committed_at);
        let previous = engine
            .runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(1))
            .map(|playback| (playback.enabled, playback.master));
        engine.go_playback(1).unwrap();
        engine
            .apply_preload_timing(1, "go", committed_at, 2_000, previous)
            .unwrap();

        let active = &engine.runtime()[0];
        assert_eq!(active.activated_at, committed_at);
        assert_eq!(active.transition_fade_fallback_millis, Some(2_000));
        assert!(
            (contribution_level(
                &engine,
                committed_at + ChronoDuration::milliseconds(1_000),
                fixture,
            ) - 0.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn explicit_cue_time_remains_authoritative_for_a_preload_transition() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "intensity", 0.0));
        let mut second = Cue::new(2.0);
        second.fade_millis = 500;
        second.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![first, second]);
        let id = cue_list.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        let previous = engine
            .runtime()
            .into_iter()
            .find(|playback| playback.playback_number == Some(1))
            .map(|playback| (playback.enabled, playback.master));
        engine.go_playback(1).unwrap();
        engine
            .apply_preload_timing(1, "go", started, 2_000, previous)
            .unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(250),
                fixture,
            ) - 0.5)
                .abs()
                < 0.01
        );
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
    fn virtual_master_controls_faderless_playback_without_adding_a_local_fader() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![cue]);
        let cue_list_id = cue_list.id;
        let mut playback = definition(1, cue_list_id);
        playback.has_fader = false;
        playback.button_count = 1;
        playback.buttons = [
            PlaybackButtonAction::Toggle,
            PlaybackButtonAction::None,
            PlaybackButtonAction::None,
        ];
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(playback).unwrap();

        assert_eq!(
            engine.set_master(1, 0.5),
            Err("playback does not have a fader".into())
        );
        engine.set_virtual_master(1, 0.5).unwrap();
        let runtime = &engine.runtime()[0];
        assert!(runtime.enabled);
        assert_eq!(runtime.master, 0.5);
        assert_eq!(runtime.fader_position, 0.5);
        assert_eq!(
            engine.contributions()[0].value,
            AttributeValue::Normalized(0.5)
        );
    }

    #[test]
    fn virtual_master_drives_faderless_manual_xfade_without_enabling_local_input() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "intensity", 0.0));
        let mut second = Cue::new(2.0);
        second.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![first, second]);
        let cue_list_id = cue_list.id;
        let mut playback = definition(1, cue_list_id);
        playback.fader = PlaybackFaderMode::XFade;
        playback.has_fader = false;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(playback).unwrap();

        assert_eq!(
            engine.set_master(1, 0.5),
            Err("playback does not have a fader".into())
        );
        assert_eq!(
            engine.set_manual_xfade(1, 0.5),
            Err("playback is not configured for manual X-fade".into())
        );
        engine.set_virtual_master(1, 0.5).unwrap();
        let runtime = &engine.runtime()[0];
        assert!(runtime.enabled);
        assert_eq!(runtime.fader_position, 0.5);
        assert_eq!(runtime.manual_xfade_position, 0.5);
        assert_eq!(runtime.manual_xfade_progress, 0.5);
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.5)
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
        let mut list = list(vec![one, two]);
        list.restart_mode = RestartMode::ContinueCurrentCue;
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
    fn legacy_layout_defaults_are_target_specific_and_invalid_layouts_are_rejected() {
        let cue_list_id = CueListId::new();
        let legacy = serde_json::json!({
            "number": 1,
            "name": "Legacy",
            "target": { "type": "cue_list", "cue_list_id": cue_list_id }
        });
        let definition: PlaybackDefinition = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            definition.buttons,
            [
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ]
        );
        assert_eq!(definition.fader, PlaybackFaderMode::Master);
        assert_eq!(definition.button_count, 3);
        assert!(definition.has_fader);

        let mut pausable = definition.clone();
        pausable.buttons[2] = PlaybackButtonAction::Pause;
        assert!(pausable.validate().is_ok());

        let mut incompatible = definition.clone();
        incompatible.target = PlaybackTarget::GrandMaster;
        assert!(incompatible.validate().is_err());
        incompatible.reset_incompatible_layout();
        assert_eq!(
            incompatible.buttons,
            [
                PlaybackButtonAction::Blackout,
                PlaybackButtonAction::PauseDynamics,
                PlaybackButtonAction::Flash,
            ]
        );
        assert!(incompatible.validate().is_ok());

        assert_eq!(
            PlaybackDefinition::default_buttons(&PlaybackTarget::Group {
                group_id: "front".into(),
            }),
            [
                PlaybackButtonAction::Select,
                PlaybackButtonAction::SelectDereferenced,
                PlaybackButtonAction::Flash,
            ]
        );
        for target in [PlaybackTarget::ProgrammerFade, PlaybackTarget::CueFade] {
            assert_eq!(
                PlaybackDefinition::default_buttons(&target),
                [
                    PlaybackButtonAction::Double,
                    PlaybackButtonAction::Half,
                    PlaybackButtonAction::Off,
                ]
            );
        }

        incompatible.button_count = 1;
        incompatible.buttons[1] = PlaybackButtonAction::None;
        incompatible.buttons[2] = PlaybackButtonAction::None;
        incompatible.has_fader = false;
        incompatible.presentation_icon = Some("star".into());
        assert!(incompatible.validate().is_ok());
        incompatible.presentation_image = Some("asset://background".into());
        assert!(incompatible.validate().is_err());
    }

    #[test]
    fn fast_navigation_bypasses_only_the_current_transition_timing() {
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "pan", 0.2));
        let mut two = Cue::new(2.0);
        two.fade_millis = 10_000;
        two.delay_millis = 5_000;
        let mut change = value(fixture, "pan", 0.8);
        change.fade_millis = Some(8_000);
        change.delay_millis = Some(4_000);
        two.changes.push(change);
        let cue_list = list(vec![one, two]);
        let cue_list_id = cue_list.id;
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(cue_list).unwrap();
        engine
            .register_definition(definition(1, cue_list_id))
            .unwrap();
        engine.on(1).unwrap();
        clock.advance_millis(20_000);
        engine.fast_forward_playback(1).unwrap();
        let pan = engine
            .contributions()
            .into_iter()
            .find(|value| value.attribute.0 == "pan")
            .unwrap();
        assert_eq!(pan.value, AttributeValue::Normalized(0.8));
        let stored = &engine.cue_lists[&cue_list_id].cues[1];
        assert_eq!((stored.fade_millis, stored.delay_millis), (10_000, 5_000));
        assert_eq!(stored.changes[0].fade_millis, Some(8_000));
        assert_eq!(stored.changes[0].delay_millis, Some(4_000));

        engine.fast_rewind_playback(1).unwrap();
        let pan = engine
            .contributions()
            .into_iter()
            .find(|value| value.attribute.0 == "pan")
            .unwrap();
        assert_eq!(pan.value, AttributeValue::Normalized(0.2));
    }

    #[test]
    fn off_requires_zero_pickup_without_moving_the_recorded_fader() {
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![cue]);
        let cue_list_id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine
            .register_definition(definition(1, cue_list_id))
            .unwrap();
        engine.set_master(1, 0.6).unwrap();
        engine.on(1).unwrap();
        engine.off(1).unwrap();
        let runtime = &engine.runtime()[0];
        assert_eq!(runtime.fader_position, 0.6);
        assert!(runtime.fader_pickup_required);

        engine.set_master(1, 0.9).unwrap();
        assert!(!engine.runtime()[0].enabled);
        assert_eq!(engine.runtime()[0].master, 1.0);
        engine.set_master(1, 0.0).unwrap();
        assert!(!engine.runtime()[0].fader_pickup_required);
        assert!(!engine.runtime()[0].enabled);
        engine.set_master(1, 0.4).unwrap();
        assert!(engine.runtime()[0].enabled);
        assert_eq!(engine.runtime()[0].master, 0.4);
    }

    #[test]
    fn temp_is_a_separate_entry_and_never_auto_offs_the_underlying_playback() {
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let fixture = FixtureId::new();
        let mut a = Cue::new(1.0);
        a.changes.push(value(fixture, "pan", 0.2));
        let mut b = Cue::new(1.0);
        b.changes.push(value(fixture, "pan", 0.8));
        let a = list(vec![a]);
        let a_id = a.id;
        let b = list(vec![b]);
        let b_id = b.id;
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(a).unwrap();
        engine.register(b).unwrap();
        engine.register_definition(definition(1, a_id)).unwrap();
        engine.register_definition(definition(2, b_id)).unwrap();
        engine.on(1).unwrap();
        clock.advance_millis(1);
        assert!(engine.toggle_temp(2).unwrap());
        assert!(engine.runtime()[0].enabled);
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey("pan".into()))],
            AttributeValue::Normalized(0.8)
        );
        assert!(!engine.toggle_temp(2).unwrap());
        assert!(engine.runtime()[0].enabled);
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey("pan".into()))],
            AttributeValue::Normalized(0.2)
        );

        let mut temp_definition = engine.definitions[&2].clone();
        temp_definition.fader = PlaybackFaderMode::Temp;
        engine.definitions.insert(2, temp_definition);
        engine.set_master(2, 0.5).unwrap();
        assert_eq!(engine.runtime_status()[1].temporary_master, 0.5);
        engine.set_master(2, 0.0).unwrap();
        assert!(!engine.runtime_status().iter().any(|status| {
            status.playback.playback_number == Some(2) && status.temporary_active
        }));
    }

    #[test]
    fn flash_release_modes_and_swap_protection_preserve_normal_runtime() {
        let fixture_a = FixtureId::new();
        let fixture_b = FixtureId::new();
        let fixture_c = FixtureId::new();
        let make = |fixture, level| {
            let mut cue = Cue::new(1.0);
            cue.changes.push(value(fixture, "intensity", level));
            cue.changes.push(value(fixture, "pan", level));
            list(vec![cue])
        };
        let a = make(fixture_a, 0.2);
        let a_id = a.id;
        let b = make(fixture_b, 0.8);
        let b_id = b.id;
        let c = make(fixture_c, 0.6);
        let c_id = c.id;
        let mut engine = PlaybackEngine::default();
        engine.register(a).unwrap();
        engine.register(b).unwrap();
        engine.register(c).unwrap();
        engine.register_definition(definition(1, a_id)).unwrap();
        let mut b_definition = definition(2, b_id);
        b_definition.flash_release = FlashReleaseMode::ReleaseIntensityOnly;
        engine.register_definition(b_definition).unwrap();
        let mut c_definition = definition(3, c_id);
        c_definition.protect_from_swap = true;
        engine.register_definition(c_definition).unwrap();
        engine.on(1).unwrap();
        engine.on(3).unwrap();

        engine.set_flash(2, true).unwrap();
        let flash_status = engine
            .runtime_status()
            .into_iter()
            .find(|status| status.playback.playback_number == Some(2))
            .unwrap();
        assert!(flash_status.playback.flash);
        assert!(flash_status.temporary_active);
        engine.set_flash(2, false).unwrap();
        assert!(engine.runtime_status().into_iter().all(|status| {
            status.playback.playback_number != Some(2) || !status.playback.flash
        }));
        let b_runtime = engine
            .runtime()
            .into_iter()
            .find(|runtime| runtime.playback_number == Some(2))
            .unwrap();
        assert!(b_runtime.enabled);
        assert_eq!(b_runtime.master, 0.0);
        let b_values = engine
            .contributions()
            .into_iter()
            .filter(|value| value.fixture_id == fixture_b)
            .collect::<Vec<_>>();
        assert_eq!(
            b_values
                .iter()
                .find(|value| value.attribute.is_intensity())
                .unwrap()
                .value,
            AttributeValue::Normalized(0.0)
        );
        assert_eq!(
            b_values
                .iter()
                .find(|value| value.attribute.0 == "pan")
                .unwrap()
                .value,
            AttributeValue::Normalized(0.8)
        );

        let a_before = engine
            .runtime()
            .into_iter()
            .find(|runtime| runtime.playback_number == Some(1))
            .unwrap();
        engine.set_swap(2, true).unwrap();
        let fixtures = engine
            .contributions()
            .into_iter()
            .map(|value| value.fixture_id)
            .collect::<HashSet<_>>();
        assert!(!fixtures.contains(&fixture_a));
        assert!(fixtures.contains(&fixture_b));
        assert!(fixtures.contains(&fixture_c));
        engine.set_swap(2, false).unwrap();
        let a_after = engine
            .runtime()
            .into_iter()
            .find(|runtime| runtime.playback_number == Some(1))
            .unwrap();
        assert_eq!(a_after.cue_index, a_before.cue_index);
        assert_eq!(a_after.master, a_before.master);
        assert_eq!(a_after.activated_at, a_before.activated_at);
    }

    #[test]
    fn manual_xfade_uses_authoritative_alternating_progress_and_survives_restore() {
        let fixture = FixtureId::new();
        let cues = [0.0, 1.0, 0.5]
            .into_iter()
            .enumerate()
            .map(|(index, level)| {
                let mut cue = Cue::new(index as f64 + 1.0);
                cue.fade_millis = 30_000;
                cue.delay_millis = 10_000;
                cue.changes.push(value(fixture, "intensity", level));
                cue
            })
            .collect();
        let cue_list = list(cues);
        let cue_list_id = cue_list.id;
        let mut playback_definition = definition(1, cue_list_id);
        playback_definition.fader = PlaybackFaderMode::XFade;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine
            .register_definition(playback_definition.clone())
            .unwrap();
        engine.on(1).unwrap();
        engine.set_master(1, 0.25).unwrap();
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.25)
        );
        engine.set_master(1, 1.0).unwrap();
        assert_eq!(engine.runtime()[0].current_cue_number, Some(2.0));
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(1.0)
        );
        assert_eq!(
            engine.runtime()[0].manual_xfade_direction,
            ManualXFadeDirection::TowardsLow
        );
        engine.set_master(1, 1.0).unwrap();
        assert_eq!(engine.runtime()[0].current_cue_number, Some(2.0));
        engine.set_master(1, 0.5).unwrap();
        assert_eq!(
            resolve(engine.contributions())[&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.75)
        );

        let runtime = engine.runtime();
        let mut restored = PlaybackEngine::default();
        restored
            .register(engine.cue_lists[&cue_list_id].clone())
            .unwrap();
        restored.register_definition(playback_definition).unwrap();
        restored.restore_active(runtime);
        assert_eq!(restored.runtime()[0].manual_xfade_position, 0.5);
        assert_eq!(restored.runtime()[0].manual_xfade_progress, 0.5);
        restored.set_master(1, 0.0).unwrap();
        assert_eq!(restored.runtime()[0].current_cue_number, Some(3.0));
    }

    #[test]
    fn pause_dynamics_freezes_and_resumes_from_the_same_phase() {
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        cue.fade_millis = 1_000;
        cue.changes.push(value(fixture, "intensity", 1.0));
        let cue_list = list(vec![cue]);
        let cue_list_id = cue_list.id;
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(cue_list).unwrap();
        engine
            .register_definition(definition(1, cue_list_id))
            .unwrap();
        engine.on(1).unwrap();
        clock.advance_millis(500);
        let level = |engine: &PlaybackEngine| {
            resolve(engine.contributions())[&(fixture, AttributeKey::intensity())]
                .normalized()
                .unwrap()
        };
        assert!((level(&engine) - 0.5).abs() < 0.001);
        engine.set_dynamics_paused(true);
        clock.advance_millis(500);
        assert!((level(&engine) - 0.5).abs() < 0.001);
        engine.set_dynamics_paused(false);
        clock.advance_millis(250);
        assert!((level(&engine) - 0.75).abs() < 0.001);
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
        assert!(list.cues[1].cue_only);
        assert!(list.cues[2].changes[0].automatic_restore);
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
    fn legacy_cues_default_cue_only_and_group_restore_metadata_to_false() {
        let mut body = serde_json::to_value(Cue::new(1.0)).unwrap();
        body.as_object_mut().unwrap().remove("cue_only");
        body["group_changes"] = serde_json::json!([{
            "group_id": "1",
            "attribute": "intensity",
            "value": { "kind": "normalized", "value": 0.5 }
        }]);
        let cue: Cue = serde_json::from_value(body).unwrap();
        assert!(!cue.cue_only);
        assert!(!cue.group_changes[0].automatic_restore);
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
            programmer_order: 0,
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
        first.changes.push(value(fixture, "intensity", 0.2));
        let mut second = Cue::new(2.0);
        second.trigger = CueTrigger::Follow { delay_millis: 100 };
        second.changes.push(value(fixture, "intensity", 0.8));
        let mut third = Cue::new(3.0);
        third.trigger = CueTrigger::Timecode { frame: 250 };
        let mut cue_list = list(vec![first, second, third]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list.clone()).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();
        engine.tick(started + ChronoDuration::milliseconds(99), None);
        assert_eq!(engine.active()[0].cue_index, 0);
        engine.tick(started + ChronoDuration::milliseconds(100), None);
        assert_eq!(engine.active()[0].cue_index, 1);
        engine.tick(started + ChronoDuration::milliseconds(100), Some(250));
        assert_eq!(engine.active()[0].cue_index, 2);
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
        assert_eq!(chaser.active()[0].cue_index, 2);
    }

    #[test]
    fn legacy_looped_lists_migrate_to_tracking_wrap_defaults() {
        let mut encoded = serde_json::to_value(list(vec![Cue::new(1.0)])).unwrap();
        let object = encoded.as_object_mut().unwrap();
        for field in [
            "intensity_priority_mode",
            "wrap_mode",
            "restart_mode",
            "force_cue_timing",
            "disable_cue_timing",
            "chaser_xfade_millis",
            "chaser_xfade_percent",
            "speed_multiplier",
        ] {
            object.remove(field);
        }
        object.insert("looped".into(), true.into());
        let migrated: CueList = serde_json::from_value(encoded).unwrap();
        assert_eq!(migrated.effective_wrap_mode(), WrapMode::Tracking);
        assert_eq!(migrated.restart_mode, RestartMode::FirstCue);
        assert_eq!(migrated.intensity_priority_mode, IntensityPriorityMode::Htp);
        assert_eq!(migrated.speed_multiplier, 1.0);
    }

    #[test]
    fn legacy_chaser_xfade_migrates_once_to_stable_integer_percent() {
        let mut legacy = list(vec![Cue::new(1.0)]);
        legacy.mode = CueListMode::Chaser;
        legacy.chaser_step_millis = 1_000;
        legacy.chaser_xfade_millis = 255;
        legacy.chaser_xfade_percent = None;
        let mut encoded = serde_json::to_value(&legacy).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("chaser_xfade_percent");

        let mut migrated: CueList = serde_json::from_value(encoded).unwrap();
        migrated.migrate_legacy_chaser_xfade(&[120.0, 90.0, 60.0, 30.0, 15.0]);
        assert_eq!(migrated.chaser_xfade_percent, Some(26));
        assert_eq!(migrated.chaser_xfade_millis, 0);
        let normalized = serde_json::to_value(&migrated).unwrap();
        assert_eq!(normalized["chaser_xfade_percent"], 26);
        assert!(normalized.get("chaser_xfade_millis").is_none());
        let reloaded: CueList = serde_json::from_value(normalized).unwrap();
        assert_eq!(reloaded.chaser_xfade_percent, Some(26));
    }

    #[test]
    fn chaser_xfade_percent_tracks_live_step_duration_exactly() {
        let mut chaser = list(vec![Cue::new(1.0)]);
        chaser.mode = CueListMode::Chaser;
        chaser.speed_group = Some("A".into());
        chaser.chaser_xfade_percent = Some(50);
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 250);
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[60.0; 5]), 500);
        chaser.speed_multiplier = 2.0;
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 125);
        chaser.chaser_xfade_percent = Some(0);
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 0);
        chaser.chaser_xfade_percent = Some(100);
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 250);
        chaser.disable_cue_timing = true;
        assert_eq!(effective_chaser_xfade_millis(&chaser, &[120.0; 5]), 0);
        assert_eq!(chaser.chaser_xfade_percent, Some(100));
    }

    #[test]
    fn tracking_wrap_keeps_final_state_while_reset_wrap_releases_it() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "intensity", 0.2));
        let mut second = Cue::new(2.0);
        second.changes.push(value(fixture, "pan", 0.7));
        let mut tracking = list(vec![first.clone(), second.clone()]);
        tracking.wrap_mode = Some(WrapMode::Tracking);
        let id = tracking.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(tracking).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        assert!(
            engine
                .contributions_at(started)
                .iter()
                .any(|value| value.attribute.0 == "pan")
        );

        let mut reset = list(vec![first, second]);
        reset.wrap_mode = Some(WrapMode::Reset);
        let reset_id = reset.id;
        let mut reset_engine = PlaybackEngine::default();
        reset_engine.register(reset).unwrap();
        reset_engine.go_at(reset_id, started).unwrap();
        reset_engine.go_at(reset_id, started).unwrap();
        reset_engine.go_at(reset_id, started).unwrap();
        assert!(
            !reset_engine
                .contributions_at(started)
                .iter()
                .any(|value| value.attribute.0 == "pan")
        );
    }

    #[test]
    fn deleting_the_active_cue_holds_output_and_anchors_navigation() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "intensity", 0.1));
        let mut two = Cue::new(2.0);
        two.changes.push(value(fixture, "intensity", 0.6));
        let mut three = Cue::new(3.0);
        three.fade_millis = 1_000;
        three.changes.push(value(fixture, "intensity", 0.9));
        let original = list(vec![one.clone(), two, three.clone()]);
        let id = original.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(original).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        let mut replacement = list(vec![one, three]);
        replacement.id = id;
        let active = engine.active_for_snapshot(&[replacement.clone()], started);
        assert_eq!(
            active[0].deleted_cue_hold.as_ref().unwrap().deleted_number,
            2.0
        );
        let mut replaced = PlaybackEngine::default();
        replaced.register(replacement).unwrap();
        replaced.restore_active(active);
        assert_eq!(contribution_level(&replaced, started, fixture), 0.6);
        replaced.go_at(id, started).unwrap();
        assert_eq!(replaced.active()[0].current_cue_number, Some(3.0));
        assert_eq!(contribution_level(&replaced, started, fixture), 0.6);
        assert!(
            (contribution_level(
                &replaced,
                started + ChronoDuration::milliseconds(500),
                fixture
            ) - 0.75)
                .abs()
                < 0.001
        );
        assert_eq!(
            contribution_level(
                &replaced,
                started + ChronoDuration::milliseconds(1_000),
                fixture
            ),
            0.9
        );
        replaced
            .back_at(id, started + ChronoDuration::milliseconds(1_000))
            .unwrap();
        assert_eq!(replaced.active()[0].current_cue_number, Some(1.0));
    }

    #[test]
    fn go_activate_honors_restart_mode_when_playback_is_off() {
        for (restart_mode, expected_index) in [
            (RestartMode::FirstCue, 0),
            (RestartMode::ContinueCurrentCue, 1),
        ] {
            let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
            cue_list.restart_mode = restart_mode;
            let id = cue_list.id;
            let mut engine = PlaybackEngine::default();
            engine.register(cue_list).unwrap();
            engine.register_definition(definition(1, id)).unwrap();
            engine.go_playback(1).unwrap();
            engine.go_playback(1).unwrap();
            assert_eq!(engine.active()[0].cue_index, 1);
            engine.off(1).unwrap();
            engine.go_playback(1).unwrap();
            assert_eq!(engine.active()[0].cue_index, expected_index);
        }
    }

    #[test]
    fn continue_restart_falls_back_to_first_if_remembered_cue_was_deleted_while_off() {
        let mut original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
        original.restart_mode = RestartMode::ContinueCurrentCue;
        let id = original.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(original).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();
        engine.off(1).unwrap();

        let mut replacement = list(vec![Cue::new(1.0), Cue::new(3.0)]);
        replacement.id = id;
        replacement.restart_mode = RestartMode::ContinueCurrentCue;
        let active = engine.active_for_snapshot(&[replacement.clone()], started);
        assert!(active[0].deleted_cue_hold.is_none());
        assert!(active[0].current_cue_id.is_none());

        let mut replaced = PlaybackEngine::default();
        replaced.register(replacement).unwrap();
        replaced.register_definition(definition(1, id)).unwrap();
        replaced.restore_active(active);
        replaced.on(1).unwrap();
        assert_eq!(replaced.active()[0].cue_index, 0);
        assert_eq!(replaced.active()[0].current_cue_number, Some(1.0));
    }

    #[test]
    fn deleting_an_inactive_earlier_cue_preserves_current_identity() {
        let original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
        let id = original.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(original).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        engine.go_at(id, started).unwrap();
        let mut replacement = list(vec![Cue::new(2.0), Cue::new(3.0)]);
        replacement.id = id;
        let active = engine.active_for_snapshot(&[replacement.clone()], started);
        let mut replaced = PlaybackEngine::default();
        replaced.register(replacement).unwrap();
        replaced.restore_active(active);
        assert_eq!(replaced.active()[0].cue_index, 1);
        assert_eq!(replaced.active()[0].current_cue_number, Some(3.0));
    }

    #[test]
    fn restart_and_timing_settings_have_contract_precedence() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.fade_millis = 1_000;
        first.delay_millis = 500;
        let mut change = value(fixture, "intensity", 1.0);
        change.fade_millis = Some(2_000);
        change.delay_millis = Some(100);
        first.changes.push(change);
        let mut second = Cue::new(2.0);
        second.trigger = CueTrigger::Wait {
            delay_millis: 4_000,
        };
        second.changes.push(value(fixture, "intensity", 0.2));
        let mut cue_list = list(vec![first, second]);
        cue_list.force_cue_timing = true;
        let id = cue_list.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list.clone()).unwrap();
        engine.go_at(id, started).unwrap();
        assert!(
            (contribution_level(
                &engine,
                started + ChronoDuration::milliseconds(1_000),
                fixture
            ) - 0.5)
                .abs()
                < 0.01
        );
        engine.go_at(id, started).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.off(1).unwrap();
        engine.on(1).unwrap();
        assert_eq!(engine.active()[0].cue_index, 0);

        cue_list.disable_cue_timing = true;
        let immediate_id = cue_list.id;
        let mut immediate = PlaybackEngine::default();
        immediate.register(cue_list).unwrap();
        immediate.go_at(immediate_id, started).unwrap();
        assert_eq!(contribution_level(&immediate, started, fixture), 1.0);
        immediate.tick(started, None);
        assert_eq!(immediate.active()[0].cue_index, 1);
    }

    #[test]
    fn ltp_intensity_can_select_a_newer_lower_value() {
        let fixture = FixtureId::new();
        let mut high = Cue::new(1.0);
        high.changes.push(value(fixture, "intensity", 0.8));
        let mut low = Cue::new(1.0);
        low.changes.push(value(fixture, "intensity", 0.2));
        let mut high = list(vec![high]);
        high.intensity_priority_mode = IntensityPriorityMode::Ltp;
        let mut low = list(vec![low]);
        low.intensity_priority_mode = IntensityPriorityMode::Ltp;
        let high_id = high.id;
        let low_id = low.id;
        let started = Utc::now();
        let mut engine = PlaybackEngine::default();
        engine.register(high).unwrap();
        engine.register(low).unwrap();
        engine.go_at(high_id, started).unwrap();
        engine
            .go_at(low_id, started + ChronoDuration::milliseconds(1))
            .unwrap();
        assert_eq!(
            resolve(engine.contributions_at(started + ChronoDuration::milliseconds(1)))
                [&(fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.2)
        );
    }

    #[test]
    fn concrete_playbacks_share_a_cuelist_but_keep_independent_runtime() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "intensity", 0.1));
        let mut two = Cue::new(2.0);
        two.changes.push(value(fixture, "intensity", 0.5));
        let mut three = Cue::new(3.0);
        three.changes.push(value(fixture, "intensity", 0.9));
        let cue_list = list(vec![one, two, three]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.register_definition(definition(2, id)).unwrap();
        engine.goto_playback(1, 2.0).unwrap();
        engine.goto_playback(2, 3.0).unwrap();
        let runtime = engine.runtime();
        assert_eq!(
            runtime
                .iter()
                .find(|item| item.playback_number == Some(1))
                .unwrap()
                .current_cue_number,
            Some(2.0)
        );
        assert_eq!(
            runtime
                .iter()
                .find(|item| item.playback_number == Some(2))
                .unwrap()
                .current_cue_number,
            Some(3.0)
        );
        assert!(engine.go(id).unwrap_err().contains("multiple playbacks"));
    }

    #[test]
    fn load_is_silent_consumed_by_go_and_cleared_by_off() {
        let fixture = FixtureId::new();
        let mut one = Cue::new(1.0);
        one.changes.push(value(fixture, "intensity", 0.1));
        let mut two = Cue::new(2.0);
        two.changes.push(value(fixture, "intensity", 0.5));
        let mut three = Cue::new(3.0);
        three.changes.push(value(fixture, "intensity", 0.9));
        let cue_list = list(vec![one, two, three]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.load_playback(1, 2.0).unwrap();
        assert!(engine.active().is_empty());
        assert!(engine.contributions().is_empty());
        assert_eq!(engine.runtime()[0].loaded_cue_number, Some(2.0));
        engine.go_playback(1).unwrap();
        assert_eq!(engine.active()[0].current_cue_number, Some(2.0));
        assert_eq!(engine.active()[0].loaded_cue_number, None);
        engine.go_playback(1).unwrap();
        assert_eq!(engine.active()[0].current_cue_number, Some(3.0));
        engine.load_playback(1, 1.0).unwrap();
        engine.back_playback(1).unwrap();
        assert_eq!(
            engine.active()[0].loaded_cue_number,
            Some(1.0),
            "GO minus deliberately preserves Load"
        );
        engine.off(1).unwrap();
        assert_eq!(engine.runtime()[0].loaded_cue_number, None);
    }

    #[test]
    fn loaded_feedback_tracks_stable_identity_through_renumber_and_deletion() {
        let original = list(vec![Cue::new(1.0), Cue::new(2.0), Cue::new(3.0)]);
        let id = original.id;
        let loaded_id = original.cues[1].id;
        let mut engine = PlaybackEngine::default();
        engine.register(original.clone()).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.load_playback(1, 2.0).unwrap();
        let status = engine.runtime_status().remove(0);
        assert_eq!(
            (
                status.normal_next_cue_number,
                status.effective_next_cue_number,
                status.effective_next_is_loaded
            ),
            (Some(1.0), Some(2.0), true)
        );

        let mut renumbered = original.clone();
        renumbered.cues[1].number = 8.0;
        renumbered
            .cues
            .sort_by(|left, right| left.number.total_cmp(&right.number));
        let active = engine.active_for_snapshot(&[renumbered.clone()], Utc::now());
        let mut restored = PlaybackEngine::default();
        restored.register(renumbered.clone()).unwrap();
        restored.register_definition(definition(1, id)).unwrap();
        restored.restore_active(active);
        let status = restored.runtime_status().remove(0);
        assert_eq!(status.playback.loaded_cue_id, Some(loaded_id));
        assert_eq!(status.effective_next_cue_number, Some(8.0));

        renumbered.cues.retain(|cue| cue.id != loaded_id);
        let active = restored.active_for_snapshot(&[renumbered.clone()], Utc::now());
        let mut deleted = PlaybackEngine::default();
        deleted.register(renumbered).unwrap();
        deleted.register_definition(definition(1, id)).unwrap();
        deleted.restore_active(active);
        let status = deleted.runtime_status().remove(0);
        assert_eq!(status.playback.loaded_cue_id, None);
        assert!(!status.effective_next_is_loaded);
        assert_eq!(status.effective_next_cue_number, Some(1.0));
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
        engine.set_control_timing([60.0, 120.0, 30.0, 15.0, 10.0], 0);
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
    fn chaser_large_virtual_jump_matches_incremental_phase() {
        let mut cue_list = list(vec![
            Cue::new(1.0),
            Cue::new(2.0),
            Cue::new(3.0),
            Cue::new(4.0),
        ]);
        cue_list.mode = CueListMode::Chaser;
        cue_list.speed_group = Some("A".into());
        cue_list.speed_multiplier = 2.0;
        let id = cue_list.id;
        let started = Utc::now();

        let mut direct = PlaybackEngine::default();
        direct.register(cue_list.clone()).unwrap();
        direct.go_at(id, started).unwrap();
        direct.tick(started + ChronoDuration::milliseconds(1_000), None);

        let mut incremental = PlaybackEngine::default();
        incremental.register(cue_list).unwrap();
        incremental.go_at(id, started).unwrap();
        for millis in [250, 500, 750, 1_000] {
            incremental.tick(started + ChronoDuration::milliseconds(millis), None);
        }

        assert_eq!(
            direct.active()[0].cue_index,
            incremental.active()[0].cue_index
        );
        assert_eq!(
            direct.active()[0].previous_index,
            incremental.active()[0].previous_index
        );
        assert_eq!(
            direct.active()[0].activated_at,
            incremental.active()[0].activated_at
        );
    }

    #[test]
    fn decimal_speed_group_bpm_reaches_chaser_scheduling_without_integer_rounding() {
        let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
        cue_list.mode = CueListMode::Chaser;
        cue_list.speed_group = Some("B".into());
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.set_control_timing([120.0, 127.5, 60.0, 30.0, 15.0], 0);
        engine.register(cue_list).unwrap();
        let started = Utc::now();
        engine.go_at(id, started).unwrap();

        // 60,000 / 127.5 rounds to a 471 ms step. Integer-rounding the BPM to 128 would
        // incorrectly advance at 469 ms instead.
        engine.tick(started + ChronoDuration::milliseconds(470), None);
        assert_eq!(engine.active()[0].cue_index, 0);
        engine.tick(started + ChronoDuration::milliseconds(471), None);
        assert_eq!(engine.active()[0].cue_index, 1);
    }

    #[test]
    fn chaser_bpm_change_preserves_normalized_step_phase() {
        let mut cue_list = list(vec![Cue::new(1.0), Cue::new(2.0)]);
        cue_list.mode = CueListMode::Chaser;
        cue_list.speed_group = Some("A".into());
        let id = cue_list.id;
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(cue_list).unwrap();
        engine.go_at(id, started).unwrap();

        clock.set(started + ChronoDuration::milliseconds(250));
        engine.tick(started + ChronoDuration::milliseconds(250), None);
        engine.set_control_timing([60.0, 90.0, 60.0, 30.0, 15.0], 0);
        engine.tick(started + ChronoDuration::milliseconds(749), None);
        assert_eq!(engine.active()[0].cue_index, 0);
        engine.tick(started + ChronoDuration::milliseconds(750), None);
        assert_eq!(engine.active()[0].cue_index, 1);
        assert_eq!(
            engine.active()[0].activated_at,
            started + ChronoDuration::milliseconds(750)
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
        engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000);
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
        explicit.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000);
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

    #[test]
    fn move_in_black_looks_through_dark_cues_and_uses_future_position_timing() {
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "intensity", 1.0));
        first.changes.push(value(fixture, "pan", 0.2));
        first.changes.push(value(fixture, "color.red", 0.1));
        let mut dark = Cue::new(2.0);
        dark.changes.push(value(fixture, "intensity", 0.0));
        let another_dark = Cue::new(2.5);
        let mut lit = Cue::new(3.0);
        lit.changes.push(value(fixture, "intensity", 1.0));
        let mut pan = value(fixture, "pan", 0.8);
        pan.fade_millis = Some(3_000);
        lit.changes.push(pan);
        lit.changes.push(value(fixture, "color.red", 0.9));
        let cue_list = list(vec![first, dark, another_dark, lit]);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();

        let candidates = engine.move_in_black_candidates();
        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.current_cue_number, 2.0);
        assert_eq!(candidate.target_cue_number, 3.0);
        assert_eq!(
            candidate.values.len(),
            1,
            "only Position-family values move early"
        );
        assert_eq!(candidate.values[0].attribute.0, "pan");
        assert_eq!(candidate.values[0].current, AttributeValue::Normalized(0.2));
        assert_eq!(candidate.values[0].target, AttributeValue::Normalized(0.8));
        assert_eq!(candidate.values[0].fade_millis, 3_000);
    }

    #[test]
    fn move_in_black_does_not_look_across_the_end_of_a_cuelist() {
        let fixture = FixtureId::new();
        let mut lit = Cue::new(1.0);
        lit.changes.push(value(fixture, "intensity", 1.0));
        lit.changes.push(value(fixture, "pan", 0.8));
        let mut dark = Cue::new(2.0);
        dark.changes.push(value(fixture, "intensity", 0.0));
        dark.changes.push(value(fixture, "pan", 0.2));
        let mut cue_list = list(vec![lit, dark]);
        cue_list.wrap_mode = Some(WrapMode::Tracking);
        let id = cue_list.id;
        let mut engine = PlaybackEngine::default();
        engine.register(cue_list).unwrap();
        engine.register_definition(definition(1, id)).unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();
        assert!(engine.move_in_black_candidates().is_empty());
    }

    #[test]
    fn snap_attributes_bypass_cue_crossfades() {
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let fixture = FixtureId::new();
        let mut first = Cue::new(1.0);
        first.changes.push(value(fixture, "pan", 0.0));
        first.changes.push(value(fixture, "tilt", 0.0));
        let mut second = Cue::new(2.0);
        second.fade_millis = 1_000;
        second.changes.push(value(fixture, "pan", 1.0));
        second.changes.push(value(fixture, "tilt", 1.0));
        let cue_list = list(vec![first, second]);
        let cue_list_id = cue_list.id;
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(cue_list).unwrap();
        engine
            .register_definition(definition(1, cue_list_id))
            .unwrap();
        engine.go_playback(1).unwrap();
        engine.go_playback(1).unwrap();

        let halfway = started + ChronoDuration::milliseconds(500);
        clock.set(halfway);
        let values = resolve(
            engine.contributions_at_with_snap(halfway, |_, attribute| attribute.0 == "pan"),
        );
        assert_eq!(
            values[&(fixture, AttributeKey("pan".into()))],
            AttributeValue::Normalized(1.0)
        );
        assert_eq!(
            values[&(fixture, AttributeKey("tilt".into()))],
            AttributeValue::Normalized(0.5)
        );
    }

    #[test]
    fn snap_attributes_bypass_playback_master_crossfades() {
        let started = Utc::now();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let snap_fixture = FixtureId::new();
        let faded_fixture = FixtureId::new();
        let mut cue = Cue::new(1.0);
        for fixture in [snap_fixture, faded_fixture] {
            cue.changes.push(value(fixture, "intensity", 1.0));
        }
        let cue_list = list(vec![cue]);
        let cue_list_id = cue_list.id;
        let mut playback = definition(1, cue_list_id);
        playback.xfade_millis = 1_000;
        let mut engine = PlaybackEngine::with_clock(clock.clone());
        engine.register(cue_list).unwrap();
        engine.register_definition(playback).unwrap();
        engine.xfade(1, true).unwrap();

        let halfway = started + ChronoDuration::milliseconds(500);
        clock.set(halfway);
        engine.tick(halfway, None);
        let values = resolve(
            engine.contributions_at_with_snap(halfway, |fixture, _| fixture == snap_fixture),
        );
        assert_eq!(
            values[&(snap_fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(1.0)
        );
        assert_eq!(
            values[&(faded_fixture, AttributeKey::intensity())],
            AttributeValue::Normalized(0.5)
        );
    }
}
