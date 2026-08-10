use crate::*;

pub const MAX_PLAYBACKS: u16 = 1_000;
pub const MAX_PLAYBACK_PAGES: u8 = 127;
pub const MAX_PAGE_SLOTS: u8 = 127;
pub const MIN_VIRTUAL_PLAYBACK: u16 = 1_001;
pub const VIRTUAL_PLAYBACKS_PER_PAGE: u16 = 300;
pub const MAX_VIRTUAL_PLAYBACK: u16 =
    MIN_VIRTUAL_PLAYBACK + (MAX_PLAYBACK_PAGES as u16 * VIRTUAL_PLAYBACKS_PER_PAGE) - 1;

pub const fn virtual_playback_page_start(page: u8) -> Option<u16> {
    if page == 0 || page > MAX_PLAYBACK_PAGES {
        return None;
    }
    Some(MIN_VIRTUAL_PLAYBACK + ((page as u16 - 1) * VIRTUAL_PLAYBACKS_PER_PAGE))
}

pub const fn virtual_playback_page_for_number(number: u16) -> Option<u8> {
    if number < MIN_VIRTUAL_PLAYBACK || number > MAX_VIRTUAL_PLAYBACK {
        return None;
    }
    Some(((number - MIN_VIRTUAL_PLAYBACK) / VIRTUAL_PLAYBACKS_PER_PAGE) as u8 + 1)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhysicalPlaybackNumber(u16);

impl PhysicalPlaybackNumber {
    pub fn new(number: u16) -> Result<Self, String> {
        if (1..=MAX_PLAYBACKS).contains(&number) {
            Ok(Self(number))
        } else {
            Err("physical playback number must be within 1-1000".into())
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VirtualPlaybackNumber(u16);

impl VirtualPlaybackNumber {
    pub fn new(number: u16) -> Result<Self, String> {
        if (MIN_VIRTUAL_PLAYBACK..=MAX_VIRTUAL_PLAYBACK).contains(&number) {
            Ok(Self(number))
        } else {
            Err("virtual playback number must be within 1001-39100".into())
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VirtualPlaybackAddress {
    page: u8,
    number: VirtualPlaybackNumber,
}

impl VirtualPlaybackAddress {
    pub fn new(page: u8, number: u16) -> Result<Self, String> {
        let page_start = virtual_playback_page_start(page)
            .ok_or_else(|| "virtual playback page must be within 1-127".to_owned())?;
        let page_end = page_start + VIRTUAL_PLAYBACKS_PER_PAGE - 1;
        if !(page_start..=page_end).contains(&number) {
            return Err(format!(
                "virtual playback page {page} requires a playback number within {page_start}-{page_end}"
            ));
        }
        Ok(Self {
            page,
            number: VirtualPlaybackNumber::new(number)?,
        })
    }

    pub fn from_number(number: u16) -> Result<Self, String> {
        let page = virtual_playback_page_for_number(number)
            .ok_or_else(|| "virtual playback number must be within 1001-39100".to_owned())?;
        Self::new(page, number)
    }

    pub const fn page(self) -> u8 {
        self.page
    }

    pub const fn number(self) -> VirtualPlaybackNumber {
        self.number
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackIdentity {
    Physical(PhysicalPlaybackNumber),
    Virtual(VirtualPlaybackAddress),
}

impl PlaybackIdentity {
    pub fn physical(number: u16) -> Result<Self, String> {
        Ok(Self::Physical(PhysicalPlaybackNumber::new(number)?))
    }

    pub fn virtual_playback(page: u8, number: u16) -> Result<Self, String> {
        let address = VirtualPlaybackAddress::new(page, number)?;
        Ok(Self::Virtual(address))
    }

    pub const fn number(self) -> u16 {
        match self {
            Self::Physical(number) => number.get(),
            Self::Virtual(address) => address.number.get(),
        }
    }

    pub const fn virtual_address(self) -> Option<VirtualPlaybackAddress> {
        match self {
            Self::Physical(_) => None,
            Self::Virtual(address) => Some(address),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTarget {
    CueList {
        cue_list_id: CueListId,
    },
    Dynamic {
        assignment: DynamicPlaybackAssignment,
    },
    /// One-shot command Macro. Press activation queues the referenced portable Macro through the
    /// desk's authoritative Macro execution service; the Playback owns no parallel runtime state.
    Macro {
        macro_id: uuid::Uuid,
    },
    /// Portable Timecode assignment. Every control surface addresses the same logical runtime
    /// identified by this show-owned Timecode id.
    Timecode {
        timecode_id: TimecodeId,
    },
    Group {
        group_id: String,
        /// Portable compatibility seed for the shared runtime Group Master.
        ///
        /// Every assignment for one Group is normalized to the same value when a legacy show is
        /// opened. The value is not live authority: the runtime owns subsequent changes and the
        /// desk/show output-runtime sidecar overrides this seed when present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial_master: Option<f32>,
    },
    SpeedGroup {
        group: String,
    },
    ProgrammerFade,
    CueFade,
    GrandMaster,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicPlaybackAssignment {
    pub dynamic: light_dynamics::DynamicReference,
    #[serde(default = "default_assignment_revision")]
    pub revision: u64,
    #[serde(default)]
    pub target_scope: Option<DynamicPlaybackTargetScope>,
    #[serde(default)]
    pub fader_mode: DynamicPlaybackFaderMode,
    #[serde(default)]
    pub priority: i16,
    #[serde(default)]
    pub activation_override: Option<light_dynamics::ActivationPolicy>,
    #[serde(default)]
    pub resume_policy: DynamicPlaybackResumePolicy,
    #[serde(default = "default_dynamic_speed_multiplier")]
    pub local_speed_multiplier: light_dynamics::Rational,
    #[serde(default)]
    pub learned_duration_millis: Option<u64>,
    #[serde(default)]
    pub crossfade_non_intensity: bool,
    #[serde(default)]
    pub auto_off_at_zero: bool,
    #[serde(default)]
    pub auto_off_flash_release: bool,
    #[serde(default = "default_true")]
    pub auto_off_full_control: bool,
}

impl DynamicPlaybackAssignment {
    /// Stable identity of the target-bound Dynamic controlled by this assignment.
    pub fn target_id(&self) -> uuid::Uuid {
        self.dynamic
            .dynamic_id
            .unwrap_or(self.dynamic.embedded_fallback.definition.id)
    }
}

const fn default_assignment_revision() -> u64 {
    1
}

const fn default_dynamic_speed_multiplier() -> light_dynamics::Rational {
    light_dynamics::Rational::ONE
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicPlaybackTargetScope {
    LiveGroup { group_id: String },
    FrozenTargets { targets: Vec<FixtureId> },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPlaybackFaderMode {
    None,
    Master,
    Size,
    #[default]
    SizeAndMaster,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicPlaybackResumePolicy {
    #[default]
    FollowDynamic,
    ResumeFrozenPhase,
    RejoinSynchronizedPosition,
    ResumeOnNextBoundary,
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
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
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
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackFootprint {
    #[default]
    Normal,
    Taller {
        upper_button: PlaybackButtonAction,
    },
    Wider {
        right_buttons: [PlaybackButtonAction; 3],
        right_fader: PlaybackFaderMode,
    },
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
    #[serde(default)]
    pub footprint: PlaybackFootprint,
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
            PlaybackTarget::Dynamic { .. } => [
                PlaybackButtonAction::Off,
                PlaybackButtonAction::Pause,
                PlaybackButtonAction::Flash,
            ],
            PlaybackTarget::Macro { .. } => [
                PlaybackButtonAction::Go,
                PlaybackButtonAction::None,
                PlaybackButtonAction::None,
            ],
            PlaybackTarget::Timecode { .. } => [
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Pause,
                PlaybackButtonAction::Off,
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
        self.footprint = if self.number > MAX_PLAYBACKS {
            PlaybackFootprint::Normal
        } else {
            match self.footprint {
                PlaybackFootprint::Normal => PlaybackFootprint::Normal,
                PlaybackFootprint::Taller { .. } => PlaybackFootprint::Taller {
                    upper_button: PlaybackButtonAction::None,
                },
                PlaybackFootprint::Wider { .. } => PlaybackFootprint::Wider {
                    right_buttons: Self::default_buttons(&self.target),
                    right_fader: Self::default_fader(&self.target),
                },
            }
        };
    }

    pub fn layout_is_compatible(&self) -> bool {
        let buttons_compatible = self.buttons.iter().enumerate().all(|(index, action)| {
            if index >= usize::from(self.button_count) {
                return *action == PlaybackButtonAction::None;
            }
            self.button_is_compatible(*action)
        });
        buttons_compatible && self.fader_is_compatible(self.fader) && self.footprint_is_compatible()
    }

    fn button_is_compatible(&self, action: PlaybackButtonAction) -> bool {
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
            PlaybackTarget::Dynamic { .. } => matches!(
                action,
                PlaybackButtonAction::On
                    | PlaybackButtonAction::Off
                    | PlaybackButtonAction::Toggle
                    | PlaybackButtonAction::Pause
                    | PlaybackButtonAction::Flash
                    | PlaybackButtonAction::DynamicRestart
                    | PlaybackButtonAction::DynamicDoubleSpeed
                    | PlaybackButtonAction::DynamicHalfSpeed
                    | PlaybackButtonAction::DynamicLearnSpeed
                    | PlaybackButtonAction::None
            ),
            PlaybackTarget::Macro { .. } => {
                matches!(
                    action,
                    PlaybackButtonAction::Go | PlaybackButtonAction::None
                )
            }
            PlaybackTarget::Timecode { .. } => matches!(
                action,
                PlaybackButtonAction::Off
                    | PlaybackButtonAction::Go
                    | PlaybackButtonAction::Pause
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
    }

    fn fader_is_compatible(&self, fader: PlaybackFaderMode) -> bool {
        match &self.target {
            PlaybackTarget::CueList { .. } => matches!(
                fader,
                PlaybackFaderMode::Master | PlaybackFaderMode::Temp | PlaybackFaderMode::XFade
            ),
            PlaybackTarget::Dynamic { .. } => fader == PlaybackFaderMode::Master,
            PlaybackTarget::Macro { .. } | PlaybackTarget::Timecode { .. } => {
                fader == PlaybackFaderMode::Master
            }
            PlaybackTarget::SpeedGroup { .. } => matches!(
                fader,
                PlaybackFaderMode::DirectBpm
                    | PlaybackFaderMode::CenteredRelative
                    | PlaybackFaderMode::LearnedPercentage
            ),
            PlaybackTarget::Group { .. }
            | PlaybackTarget::ProgrammerFade
            | PlaybackTarget::CueFade
            | PlaybackTarget::GrandMaster => fader == PlaybackFaderMode::Master,
        }
    }

    fn footprint_is_compatible(&self) -> bool {
        match self.footprint {
            PlaybackFootprint::Normal => true,
            PlaybackFootprint::Taller { upper_button } => {
                self.number <= MAX_PLAYBACKS && self.button_is_compatible(upper_button)
            }
            PlaybackFootprint::Wider {
                right_buttons,
                right_fader,
            } => {
                self.number <= MAX_PLAYBACKS
                    && right_buttons
                        .into_iter()
                        .all(|action| self.button_is_compatible(action))
                    && self.fader_is_compatible(right_fader)
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_VIRTUAL_PLAYBACK).contains(&self.number) {
            return Err("playback number must be physical 1-1000 or virtual 1001-39100".into());
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
        if let PlaybackTarget::Dynamic { assignment } = &self.target {
            if assignment.revision == 0
                || assignment.local_speed_multiplier.numerator == 0
                || assignment.local_speed_multiplier.denominator == 0
                || assignment.learned_duration_millis == Some(0)
            {
                return Err("Dynamic Playback assignment is invalid".into());
            }
            let definition = &assignment.dynamic.embedded_fallback.definition;
            if matches!(
                definition.target_binding,
                light_dynamics::DynamicTargetBinding::Targetless
            ) {
                return Err("targetless Dynamics cannot be assigned directly to a Playback".into());
            }
            if assignment.target_scope.is_some() {
                return Err(
                    "target-bound Dynamic Playback assignments must not override target scope"
                        .into(),
                );
            }
        }
        if let PlaybackTarget::Macro { macro_id } = &self.target
            && macro_id.is_nil()
        {
            return Err("Macro Playback target id must not be nil".into());
        }
        if let PlaybackTarget::Timecode { timecode_id } = &self.target
            && timecode_id.0.is_nil()
        {
            return Err("Timecode Playback target id must not be nil".into());
        }
        if let PlaybackTarget::Group {
            initial_master: Some(initial_master),
            ..
        } = &self.target
            && (!initial_master.is_finite() || !(0.0..=1.0).contains(initial_master))
        {
            return Err("Group Master initial level must be within 0-1".into());
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
        if self
            .presentation_icon
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 1_024)
        {
            return Err("playback presentation icon must contain 1-1024 characters".into());
        }
        if self
            .presentation_image
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.len() > 600_000)
        {
            return Err("playback presentation image must contain 1-600000 characters".into());
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
            #[serde(default)]
            footprint: PlaybackFootprint,
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
            footprint: stored.footprint,
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
    /// Sparse dedicated Virtual Playback assignments for this page.
    ///
    /// Keys are the operator-visible numbers in this page's 300-number Virtual Playback bank.
    /// They are intentionally separate from physical page slots.
    pub virtual_playbacks: HashMap<u16, PlaybackDefinition>,
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
        for (number, playback) in &self.virtual_playbacks {
            VirtualPlaybackAddress::new(self.number, *number)?;
            if playback.number != *number {
                return Err(
                    "virtual playback assignment key must match its playback number".into(),
                );
            }
            playback.validate()?;
        }
        Ok(())
    }
}
