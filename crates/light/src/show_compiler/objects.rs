use super::invalid_candidate;
use crate::ActionError;
use light_playback::{
    CueList, FlashReleaseMode, PlaybackButtonAction, PlaybackDefinition, PlaybackFaderMode,
    PlaybackPage, PlaybackTarget,
};
use light_programmer::GroupDefinition;
use light_show::PortableShowCandidate;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub(super) fn decode<T: DeserializeOwned>(
    candidate: PortableShowCandidate<'_>,
    kind: &str,
) -> Result<Vec<T>, ActionError> {
    candidate
        .objects_of_kind(kind)
        .map(|object| {
            serde_json::from_value(object.body().clone()).map_err(|error| {
                invalid_candidate(format!("invalid {kind} {}: {error}", object.key().id()))
            })
        })
        .collect()
}

pub(super) fn decode_groups(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<GroupDefinition>, ActionError> {
    candidate
        .objects_of_kind("group")
        .map(|object| {
            let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())
                .map_err(|error| {
                    invalid_candidate(format!("invalid group {}: {error}", object.key().id()))
                })?;
            group.id = object.key().id().to_owned();
            Ok(group)
        })
        .collect()
}

pub(super) fn supply_playback_defaults(
    cue_lists: &[CueList],
    playbacks: &mut Vec<PlaybackDefinition>,
    pages: &mut Vec<PlaybackPage>,
) {
    if playbacks.is_empty() {
        playbacks.extend(
            cue_lists
                .iter()
                .take(1_000)
                .enumerate()
                .map(default_playback),
        );
    }
    if pages.is_empty() {
        pages.push(PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::new(),
        });
    }
}

fn default_playback((index, cue_list): (usize, &CueList)) -> PlaybackDefinition {
    PlaybackDefinition {
        number: index as u16 + 1,
        name: cue_list.name.clone(),
        target: PlaybackTarget::CueList {
            cue_list_id: cue_list.id,
        },
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
