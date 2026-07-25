use crate::*;

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
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn contribution_level(engine: &PlaybackEngine, at: DateTime<Utc>, fixture: FixtureId) -> f32 {
    engine
        .contributions_at(at)
        .into_iter()
        .find(|value| value.fixture_id == fixture && value.attribute.is_intensity())
        .and_then(|value| value.value.normalized())
        .unwrap_or(-1.0)
}

mod automatic;
mod contribution;
mod controls;
mod cue_recording;
mod cue_tracking;
mod master;
mod mutation;
mod runtime;
mod scheduling;
