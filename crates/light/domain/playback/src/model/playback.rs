use crate::*;

pub const MAX_PLAYBACKS: u16 = 1_000;
pub const MAX_PLAYBACK_PAGES: u8 = 127;
pub const MAX_PAGE_SLOTS: u8 = 127;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackTarget {
    CueList {
        cue_list_id: CueListId,
    },
    Dynamic {
        assignment: DynamicPlaybackAssignment,
    },
    Group {
        group_id: String,
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
            PlaybackTarget::Dynamic { .. } => [
                PlaybackButtonAction::Off,
                PlaybackButtonAction::Pause,
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
            PlaybackTarget::Dynamic { .. } => self.fader == PlaybackFaderMode::Master,
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
            ) && assignment.target_scope.is_none()
            {
                return Err(
                    "targetless Dynamic Playback assignments require an explicit target scope"
                        .into(),
                );
            }
            if let Some(DynamicPlaybackTargetScope::FrozenTargets { targets }) =
                &assignment.target_scope
                && targets.is_empty()
            {
                return Err("Dynamic Playback frozen target scope must not be empty".into());
            }
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
