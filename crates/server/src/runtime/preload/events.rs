use super::*;
use preparation::{PlaybackIdentity, PlaybackProjection};
use std::collections::{BTreeMap, HashMap};

pub(super) struct PreloadChangeEvents {
    pub(super) drafts: Vec<light_application::EventDraft>,
    exclusion_activations: Vec<(u16, Vec<u16>)>,
}

pub(super) fn preload_change_events(
    state: &AppState,
    context: &light_application::ActionContext,
    identities: &[PlaybackIdentity],
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    actions: &[StagedPreloadPlaybackAction],
) -> Result<PreloadChangeEvents, String> {
    let after = preparation::read_projections(state, context, identities)?;
    let transitions = ordered_transitions(before, after)?;
    let exclusion_activations = final_exclusion_activations(&transitions, actions);
    let drafts = transitions
        .into_iter()
        .filter_map(|(identity, before, projection)| {
            change_event(context, identity, before, projection, actions)
        })
        .collect();
    Ok(PreloadChangeEvents {
        drafts,
        exclusion_activations,
    })
}

pub(super) fn emit_exclusions(state: &AppState, session: &Session, changes: &PreloadChangeEvents) {
    for (activated_playback, released_playbacks) in &changes.exclusion_activations {
        emit(
            state,
            "playback_exclusion_applied",
            serde_json::json!({
                "desk_id":session.desk.id,
                "activated_playback":activated_playback,
                "released_playbacks":released_playbacks,
                "source":"preload",
            }),
        );
    }
}

fn ordered_transitions(
    before: Vec<(PlaybackIdentity, PlaybackProjection)>,
    after: Vec<(PlaybackIdentity, PlaybackProjection)>,
) -> Result<Vec<(PlaybackIdentity, PlaybackProjection, PlaybackProjection)>, String> {
    let mut after = after.into_iter().collect::<HashMap<_, _>>();
    let mut transitions = Vec::with_capacity(before.len());
    for (identity, before) in before {
        let projection = after
            .remove(&identity)
            .ok_or_else(|| "Playback projection batch lost an identity".to_owned())?;
        transitions.push((identity, before, projection));
    }
    if !after.is_empty() {
        return Err("Playback projection batch returned an unexpected identity".into());
    }
    transitions.sort_by_key(|(identity, before, after)| {
        (
            !playback_was_released(before, after),
            identity_sort_key(*identity),
        )
    });
    Ok(transitions)
}

fn change_event(
    context: &light_application::ActionContext,
    identity: PlaybackIdentity,
    before: PlaybackProjection,
    projection: PlaybackProjection,
    actions: &[StagedPreloadPlaybackAction],
) -> Option<light_application::EventDraft> {
    let cause = final_event_cause(actions, identity);
    let action = if playback_was_released(&before, &projection) {
        light_application::PlaybackAction::Off { pressed: true }
    } else {
        cause.action
    };
    light_application::committed_playback_effect_event(
        context,
        action,
        navigation_cause(actions, identity),
        before,
        projection,
        cause.addressed_effect_changed,
    )
}

#[derive(Clone, Copy)]
struct FinalEventCause {
    action: light_application::PlaybackAction,
    addressed_effect_changed: bool,
}

fn final_event_cause(
    actions: &[StagedPreloadPlaybackAction],
    identity: PlaybackIdentity,
) -> FinalEventCause {
    let PlaybackIdentity::Playback(number) = identity else {
        return no_event_cause();
    };
    for action in actions.iter().rev() {
        if action.playback_number == number && action.addressed_effect_changed {
            return FinalEventCause {
                action: application_action(action.action),
                addressed_effect_changed: true,
            };
        }
        if action.released_playbacks.contains(&number) {
            return FinalEventCause {
                action: light_application::PlaybackAction::Off { pressed: true },
                addressed_effect_changed: false,
            };
        }
    }
    no_event_cause()
}

fn no_event_cause() -> FinalEventCause {
    FinalEventCause {
        action: light_application::PlaybackAction::None { pressed: true },
        addressed_effect_changed: false,
    }
}

fn navigation_cause(
    actions: &[StagedPreloadPlaybackAction],
    identity: PlaybackIdentity,
) -> Option<light_application::PlaybackTransitionCause> {
    let PlaybackIdentity::Playback(number) = identity else {
        return None;
    };
    actions.iter().rev().find_map(|action| {
        if action.playback_number != number || !action.addressed_effect_changed {
            return None;
        }
        match action.action {
            light_programmer::PreloadPlaybackQueueAction::Go => {
                Some(light_application::PlaybackTransitionCause::Go)
            }
            light_programmer::PreloadPlaybackQueueAction::Back => {
                Some(light_application::PlaybackTransitionCause::Back)
            }
            _ => None,
        }
    })
}

fn final_exclusion_activations(
    transitions: &[(PlaybackIdentity, PlaybackProjection, PlaybackProjection)],
    actions: &[StagedPreloadPlaybackAction],
) -> Vec<(u16, Vec<u16>)> {
    let mut attributed = BTreeMap::<usize, (u16, Vec<u16>)>::new();
    for number in final_released_numbers(transitions) {
        let Some((index, action)) = final_action_for(number, actions) else {
            continue;
        };
        if action.released_playbacks.contains(&number) {
            attributed
                .entry(index)
                .or_insert_with(|| (action.playback_number, Vec::new()))
                .1
                .push(number);
        }
    }
    attributed.into_values().collect()
}

fn final_released_numbers(
    transitions: &[(PlaybackIdentity, PlaybackProjection, PlaybackProjection)],
) -> impl Iterator<Item = u16> + '_ {
    transitions
        .iter()
        .filter(|(_, before, after)| playback_was_released(before, after))
        .filter_map(|(identity, _, _)| match identity {
            PlaybackIdentity::Playback(number) => Some(*number),
            PlaybackIdentity::CueList(_) => None,
        })
}

fn final_action_for(
    number: u16,
    actions: &[StagedPreloadPlaybackAction],
) -> Option<(usize, &StagedPreloadPlaybackAction)> {
    actions.iter().enumerate().rev().find(|(_, action)| {
        (action.playback_number == number && action.addressed_effect_changed)
            || action.released_playbacks.contains(&number)
    })
}

fn playback_was_released(before: &PlaybackProjection, after: &PlaybackProjection) -> bool {
    before
        .cue_list_runtime()
        .is_some_and(|runtime| runtime.enabled)
        && !after
            .cue_list_runtime()
            .is_some_and(|runtime| runtime.enabled)
}

fn identity_sort_key(identity: PlaybackIdentity) -> (u8, u128) {
    match identity {
        PlaybackIdentity::Playback(number) => (0, u128::from(number)),
        PlaybackIdentity::CueList(id) => (1, id.0.as_u128()),
    }
}

fn application_action(
    action: light_programmer::PreloadPlaybackQueueAction,
) -> light_application::PlaybackAction {
    match action {
        light_programmer::PreloadPlaybackQueueAction::Toggle => {
            light_application::PlaybackAction::Toggle { pressed: true }
        }
        light_programmer::PreloadPlaybackQueueAction::Go => {
            light_application::PlaybackAction::Go { pressed: true }
        }
        light_programmer::PreloadPlaybackQueueAction::Back => {
            light_application::PlaybackAction::Back { pressed: true }
        }
        light_programmer::PreloadPlaybackQueueAction::Off => {
            light_application::PlaybackAction::Off { pressed: true }
        }
        light_programmer::PreloadPlaybackQueueAction::On => {
            light_application::PlaybackAction::On { pressed: true }
        }
        light_programmer::PreloadPlaybackQueueAction::TemporaryOn => {
            light_application::PlaybackAction::Temporary {
                enabled: true,
                pressed: true,
            }
        }
        light_programmer::PreloadPlaybackQueueAction::TemporaryOff => {
            light_application::PlaybackAction::Temporary {
                enabled: false,
                pressed: false,
            }
        }
    }
}
