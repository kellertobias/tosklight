use crate::*;

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

    pub(crate) fn address(&self) -> AttributeAddress {
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

pub(crate) fn cue_completion_millis(
    cue_list: &CueList,
    cue: &Cue,
    sequence_master_fade_millis: u64,
) -> u64 {
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
