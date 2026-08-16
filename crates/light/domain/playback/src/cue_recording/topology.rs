use super::*;

impl CueList {
    /// Build the backend-canonical first-Cue topology through the same recording content model.
    pub fn new_recording(
        id: CueListId,
        name: impl Into<String>,
        content: CueRecordingContent,
        cue_number: Option<CueNumber>,
        auto_off_at_zero: bool,
        auto_off_flash_release: bool,
    ) -> Result<CueListRecordingPlan, CueRecordingPlanError> {
        validate_content(&content)?;
        require_values(&content)?;
        let cue_number = cue_number.unwrap_or_else(|| CueNumber::from(1_u8));
        let cue = new_cue(content, cue_number.clone());
        let cue_id = cue.id;
        let cue_list = Self {
            id,
            name: name.into(),
            priority: 0,
            mode: CueListMode::Sequence,
            looped: false,
            chaser_step_millis: 1_000,
            speed_group: None,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Off),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            auto_off_at_zero,
            auto_off_flash_release,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_multiplier: 1.0,
            cues: vec![cue],
        };
        Ok(CueListRecordingPlan {
            cue_list,
            changed: true,
            cue_id,
            cue_number: cue_number.clone(),
            deleted: false,
        })
    }
}

impl PlaybackDefinition {
    pub fn new_cue_list(number: u16, name: impl Into<String>, cue_list_id: CueListId) -> Self {
        Self {
            number,
            name: name.into(),
            target: PlaybackTarget::CueList { cue_list_id },
            buttons: [
                PlaybackButtonAction::GoMinus,
                PlaybackButtonAction::Go,
                PlaybackButtonAction::Flash,
            ],
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader: true,
            footprint: crate::PlaybackFootprint::Normal,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }
}
