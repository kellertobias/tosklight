use crate::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    pub(crate) fn address(&self) -> AttributeAddress {
        (self.fixture_id, self.attribute.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub number: CueNumber,
    pub name: String,
    /// Operator-authored context shown in Cue-list overview and current/next information blocks.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub information: String,
    pub changes: Vec<CueChange>,
    pub fade_millis: u64,
    pub delay_millis: u64,
    /// Independent master fade for Intensity values leaving the previous look. `None` preserves
    /// legacy behavior by following the effective in fade, including the Cue Fade fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_fade_millis: Option<u64>,
    /// Optional live link for Out Fade. The numeric value remains stored as the operator's last
    /// explicit value and becomes effective again when this link is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_fade_link: Option<CueOutFadeLink>,
    /// Independent master hold before Intensity values leave the previous look. `None` preserves
    /// legacy Cue behavior by following `delay_millis` until explicitly separated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_delay_millis: Option<u64>,
    /// Optional live link for Out Delay. The numeric value remains stored as the operator's last
    /// explicit value and becomes effective again when this link is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_delay_link: Option<CueOutDelayLink>,
    pub trigger: CueTrigger,
    /// Marks an operator-recorded Cue-only Cue so an appended following Cue can generate the
    /// required automatic restore/release delta after a save, refresh, or reopen.
    #[serde(default)]
    pub cue_only: bool,
    #[serde(default)]
    pub group_changes: Vec<GroupCueChange>,
    /// First-class Dynamic On/Off/FAT deltas. These track independently from ordinary static
    /// values so a later AT updates Current without stopping or restarting a Dynamic.
    #[serde(default)]
    pub dynamic_changes: Vec<CueDynamicChange>,
    /// Ordered non-value actions dispatched once when this Cue becomes current.
    #[serde(default)]
    pub actions: Vec<CueAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueOutFadeLink {
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueOutDelayLink {
    InFade,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CueAction {
    /// Take this stable Cue destination for the configured number of arrivals before ordinary
    /// forward progression resumes. The count is runtime-only and Cuelist-owned.
    Jump {
        cue_id: Uuid,
        count: u32,
    },
    TimecodeStart {
        timecode_id: TimecodeId,
        start: CueTimecodeStart,
    },
    TimecodeStop {
        timecode_id: TimecodeId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CueTimecodeStart {
    Frame { frame: TimecodeFrame },
    Marker { marker_id: TimecodeMarkerId },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CueDynamicChange {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: light_dynamics::DynamicSemanticValue,
    #[serde(default)]
    pub automatic_restore: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub fn new(number: CueNumber) -> Self {
        Self {
            id: Uuid::new_v4(),
            number,
            name: String::new(),
            information: String::new(),
            changes: Vec::new(),
            fade_millis: 0,
            delay_millis: 0,
            out_fade_millis: None,
            out_fade_link: None,
            out_delay_millis: None,
            out_delay_link: None,
            trigger: CueTrigger::Manual,
            cue_only: false,
            group_changes: Vec::new(),
            dynamic_changes: Vec::new(),
            actions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CueTrigger {
    Manual,
    Follow {
        delay_millis: u64,
    },
    Wait {
        delay_millis: u64,
    },
    Timecode {
        frame: u64,
    },
    /// After this Cue has completed, jump to the Cue with this stable identity.
    Link {
        cue_id: Uuid,
        delay_millis: u64,
    },
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Turn this Cuelist Off when an accepted Master fader value reaches zero.
    ///
    /// This is Cuelist-owned so every physical and virtual assignment shares one behavior.
    #[serde(default)]
    pub auto_off_at_zero: bool,
    /// Turn this Cuelist Off when a Flash gesture which started it is released.
    #[serde(default)]
    pub auto_off_flash_release: bool,
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

pub(crate) fn cue_completion_millis(
    cue_list: &CueList,
    compiled: &CompiledCueList,
    playback: &ActivePlayback,
    cue_fade_millis: u64,
    release_fade_millis: u64,
) -> u64 {
    if cue_list.disable_cue_timing
        || playback.transition_timing_bypassed
        || playback.manual_xfade_from_index.is_some()
    {
        return 0;
    }
    let target_index = playback.manual_xfade_to_index.unwrap_or(playback.cue_index);
    let Some(cue) = cue_list.cues.get(target_index) else {
        return 0;
    };
    let previous_index = playback.manual_xfade_from_index.or(playback.previous_index);
    let outgoing_cue = previous_index.and_then(|index| cue_list.cues.get(index));
    let outgoing_cue_fade_millis = outgoing_cue.map(|outgoing| {
        if outgoing.fade_millis == 0 {
            cue_fade_millis
        } else {
            outgoing.fade_millis
        }
    });
    let transition_source = playback
        .deleted_cue_transition_source
        .as_ref()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    (
                        (value.fixture_id, value.attribute.clone()),
                        completion_source_value(value, playback),
                    )
                })
                .collect::<HashMap<_, _>>()
        });
    let latest_index = previous_index.map_or(target_index, |index| index.max(target_index));
    let fixture_values = compiled
        .attributes_through(latest_index)
        .iter()
        .filter_map(|attribute| {
            let previous = transition_source
                .as_ref()
                .and_then(|source| {
                    source.get(&(attribute.fixture_id(), attribute.attribute().clone()))
                })
                .or_else(|| previous_index.and_then(|index| attribute.value(index, false)));
            let target = attribute.value(target_index, playback.tracking_wrap);
            (previous != target).then(|| {
                let outgoing = is_outgoing_intensity(attribute.attribute(), previous, target);
                let (fade, delay) = effective_attribute_timing(
                    cue_list,
                    cue,
                    cue_fade_millis,
                    outgoing_cue.zip(outgoing_cue_fade_millis),
                    attribute.timing(target_index),
                    outgoing,
                    release_fade_millis,
                );
                delay.saturating_add(fade)
            })
        });
    let group_values = cue.group_changes.iter().map(|change| {
        change
            .delay_millis
            .unwrap_or(cue.delay_millis)
            .saturating_add(change.fade_millis.unwrap_or(cue_fade_millis))
    });
    let dynamic_values = cue.dynamic_changes.iter().map(|change| {
        let timing = match &change.value {
            light_dynamics::DynamicSemanticValue::Static { timing, .. }
            | light_dynamics::DynamicSemanticValue::DynamicOn { timing, .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { timing, .. }
            | light_dynamics::DynamicSemanticValue::FixAt { timing, .. } => *timing,
            light_dynamics::DynamicSemanticValue::Release => Default::default(),
        };
        timing
            .delay_millis
            .unwrap_or(cue.delay_millis)
            .saturating_add(timing.fade_millis.unwrap_or(cue_fade_millis))
    });
    fixture_values
        .chain(group_values)
        .chain(dynamic_values)
        .chain(std::iter::once(playback.external_completion_millis))
        .max()
        .unwrap_or(0)
}

pub(crate) fn effective_attribute_timing(
    cue_list: &CueList,
    cue: &Cue,
    cue_fade_millis: u64,
    outgoing_cue: Option<(&Cue, u64)>,
    timing: Option<(Option<u64>, Option<u64>)>,
    outgoing_intensity: bool,
    release_fade_millis: u64,
) -> (u64, u64) {
    if cue_list.disable_cue_timing {
        return (0, 0);
    }
    let master = if outgoing_intensity && cue_list.mode == CueListMode::Sequence {
        let (cue, fade_millis) = outgoing_cue.unwrap_or((cue, cue_fade_millis));
        effective_cue_out_timing(cue_list, cue, fade_millis, release_fade_millis)
    } else {
        (cue_fade_millis, cue.delay_millis)
    };
    if cue_list.force_cue_timing {
        return master;
    }
    let (fade, delay) = timing.unwrap_or((None, None));
    (fade.unwrap_or(master.0), delay.unwrap_or(master.1))
}

pub(crate) fn effective_cue_out_timing(
    cue_list: &CueList,
    cue: &Cue,
    effective_in_fade_millis: u64,
    release_fade_millis: u64,
) -> (u64, u64) {
    if cue_list.disable_cue_timing {
        return (0, 0);
    }
    let fade = match cue.out_fade_link {
        Some(CueOutFadeLink::Release) => release_fade_millis,
        None => cue.out_fade_millis.unwrap_or(effective_in_fade_millis),
    };
    let delay = match cue.out_delay_link {
        Some(CueOutDelayLink::InFade) => effective_in_fade_millis,
        None => cue.out_delay_millis.unwrap_or(cue.delay_millis),
    };
    (fade, delay)
}

pub(crate) fn is_outgoing_intensity(
    attribute: &AttributeKey,
    previous: Option<&AttributeValue>,
    target: Option<&AttributeValue>,
) -> bool {
    if !attribute.is_intensity() {
        return false;
    }
    let previous = previous.and_then(AttributeValue::normalized).unwrap_or(0.0);
    let target = target.and_then(AttributeValue::normalized).unwrap_or(0.0);
    target < previous
}

fn completion_source_value(value: &TimedValue, playback: &ActivePlayback) -> AttributeValue {
    if !value.attribute.is_intensity() {
        return value.value.clone();
    }
    let master = if playback.flash { 1.0 } else { playback.master };
    value
        .value
        .normalized()
        .map(|level| {
            AttributeValue::Normalized(if master > 0.0 {
                (level / master).clamp(0.0, 1.0)
            } else {
                0.0
            })
        })
        .unwrap_or_else(|| value.value.clone())
}

pub(crate) fn effective_cue_fade_millis(
    cue_list: &CueList,
    cue: &Cue,
    playback: &ActivePlayback,
    sequence_master_fade_millis: u64,
    speed_groups_bpm: &[f64; 5],
) -> u64 {
    if cue_list.disable_cue_timing {
        0
    } else if cue_list.mode == CueListMode::Chaser {
        effective_chaser_xfade_millis(cue_list, speed_groups_bpm)
    } else if cue.fade_millis == 0 {
        playback
            .transition_fade_fallback_millis
            .unwrap_or(sequence_master_fade_millis)
    } else {
        cue.fade_millis
    }
}

pub(crate) fn effective_chaser_step_millis(cue_list: &CueList, speed_groups_bpm: &[f64; 5]) -> u64 {
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
        let mut cue_ids = HashSet::new();
        for cue in &self.cues {
            if !cue_ids.insert(cue.id) {
                return Err("cue identities must be unique within a cue list".into());
            }
        }
        let mut previous: Option<&CueNumber> = None;
        for cue in &self.cues {
            if previous.is_some_and(|previous| cue.number <= *previous) {
                return Err(
                    "Cue numbers must be unique and strictly increasing; legacy decimal Cue numbers may need manual renumbering after canonicalization"
                        .into(),
                );
            }
            previous = Some(&cue.number);
            let mut addresses = HashSet::new();
            for change in &cue.changes {
                if !addresses.insert(change.address()) {
                    return Err(format!(
                        "cue {} contains duplicate fixture attributes",
                        cue.number
                    ));
                }
            }
            let mut jump_seen = false;
            for action in &cue.actions {
                let timecode_id = match action {
                    CueAction::Jump { cue_id, count } => {
                        if jump_seen {
                            return Err(format!(
                                "cue {} contains more than one jump point",
                                cue.number
                            ));
                        }
                        jump_seen = true;
                        if *count == 0 {
                            return Err(format!(
                                "cue {} jump count must be greater than zero",
                                cue.number
                            ));
                        }
                        if !cue_ids.contains(cue_id) {
                            return Err(format!(
                                "cue {} jumps to a missing cue identity",
                                cue.number
                            ));
                        }
                        continue;
                    }
                    CueAction::TimecodeStart {
                        timecode_id, start, ..
                    } => {
                        if matches!(start, CueTimecodeStart::Marker { marker_id } if marker_id.0.is_nil())
                        {
                            return Err(format!("cue {} has a nil Timecode marker id", cue.number));
                        }
                        timecode_id
                    }
                    CueAction::TimecodeStop { timecode_id } => timecode_id,
                };
                if timecode_id.0.is_nil() {
                    return Err(format!("cue {} has a nil Timecode id", cue.number));
                }
            }
            if let CueTrigger::Link { cue_id, .. } = cue.trigger {
                if cue_id == cue.id {
                    return Err(format!("cue {} cannot link to itself", cue.number));
                }
                if !cue_ids.contains(&cue_id) {
                    return Err(format!(
                        "cue {} links to a missing cue identity",
                        cue.number
                    ));
                }
            }
        }
        self.validate_link_cycles()?;
        Ok(())
    }

    fn validate_link_cycles(&self) -> Result<(), String> {
        let links = self
            .cues
            .iter()
            .filter_map(|cue| match cue.trigger {
                CueTrigger::Link { cue_id, .. } => Some((cue.id, cue_id)),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        for cue in &self.cues {
            let mut visited = HashSet::new();
            let mut current = cue.id;
            while let Some(next) = links.get(&current).copied() {
                if !visited.insert(current) {
                    return Err("cue link cycle is not allowed".into());
                }
                current = next;
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

    pub fn state_at_number(&self, number: &CueNumber) -> HashMap<AttributeAddress, AttributeValue> {
        let index = self.cues.iter().rposition(|cue| cue.number <= *number);
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

pub(crate) fn apply_changes(
    state: &mut HashMap<AttributeAddress, AttributeValue>,
    changes: &[CueChange],
) {
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
