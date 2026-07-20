use std::collections::{HashMap, HashSet};

use light_playback::{CueChange, CueList, GroupCueChange};

use super::super::incoming::IncomingValue;
use super::super::model::{
    CueSource, CueUpdateMode, UpdateAddress, UpdateIgnoreReason, UpdateItemOutcome,
};

#[derive(Clone, Copy, Debug)]
enum CueEventKind {
    Fixture,
    Group,
}

#[derive(Clone, Debug)]
struct CueEventLocation {
    cue_index: usize,
    change_index: usize,
    kind: CueEventKind,
    has_value: bool,
}

pub(super) struct CueAnalysis {
    all_addresses: HashSet<UpdateAddress>,
    active_sources: HashMap<UpdateAddress, CueEventLocation>,
    current_explicit: HashMap<UpdateAddress, CueEventLocation>,
    current_any: HashMap<UpdateAddress, CueEventLocation>,
}

fn fixture_address(change: &CueChange) -> UpdateAddress {
    UpdateAddress::FixtureAttribute {
        fixture_id: change.fixture_id,
        attribute: change.attribute.clone(),
    }
}

fn group_address(change: &GroupCueChange) -> UpdateAddress {
    UpdateAddress::GroupAttribute {
        group_id: change.group_id.clone(),
        attribute: change.attribute.clone(),
    }
}

pub(super) fn analyse_cue_list(cue_list: &CueList, current_index: usize) -> CueAnalysis {
    let mut analysis = CueAnalysis {
        all_addresses: HashSet::new(),
        active_sources: HashMap::new(),
        current_explicit: HashMap::new(),
        current_any: HashMap::new(),
    };
    for (cue_index, cue) in cue_list.cues.iter().enumerate() {
        for (change_index, change) in cue.changes.iter().enumerate() {
            record_event(
                &mut analysis,
                fixture_address(change),
                CueEventLocation {
                    cue_index,
                    change_index,
                    kind: CueEventKind::Fixture,
                    has_value: change.value.is_some(),
                },
                current_index,
                change.automatic_restore,
            );
        }
        for (change_index, change) in cue.group_changes.iter().enumerate() {
            record_event(
                &mut analysis,
                group_address(change),
                CueEventLocation {
                    cue_index,
                    change_index,
                    kind: CueEventKind::Group,
                    has_value: change.value.is_some(),
                },
                current_index,
                change.automatic_restore,
            );
        }
    }
    analysis
}

fn record_event(
    analysis: &mut CueAnalysis,
    address: UpdateAddress,
    location: CueEventLocation,
    current_index: usize,
    automatic_restore: bool,
) {
    analysis.all_addresses.insert(address.clone());
    if location.cue_index <= current_index {
        analysis
            .active_sources
            .insert(address.clone(), location.clone());
    }
    if location.cue_index == current_index {
        analysis
            .current_any
            .insert(address.clone(), location.clone());
        if !automatic_restore {
            analysis.current_explicit.insert(address, location);
        }
    }
}

pub(super) fn cue_source(cue_list: &CueList, cue_index: usize) -> CueSource {
    let cue = &cue_list.cues[cue_index];
    CueSource {
        cue_id: cue.id,
        cue_number: cue.number,
        cue_index,
    }
}

fn event_matches(
    cue_list: &CueList,
    location: &CueEventLocation,
    incoming: IncomingValue<'_>,
) -> bool {
    match location.kind {
        CueEventKind::Fixture => {
            let change = &cue_list.cues[location.cue_index].changes[location.change_index];
            change.value.as_ref() == Some(incoming.value())
                && !change.automatic_restore
                && change.fade_millis == incoming.fade_millis()
                && change.delay_millis == incoming.delay_millis()
        }
        CueEventKind::Group => {
            let change = &cue_list.cues[location.cue_index].group_changes[location.change_index];
            change.value.as_ref() == Some(incoming.value())
                && !change.automatic_restore
                && change.fade_millis == incoming.fade_millis()
                && change.delay_millis == incoming.delay_millis()
        }
    }
}

pub(super) fn cue_outcome(
    cue_list: &CueList,
    analysis: &CueAnalysis,
    current: &CueSource,
    mode: CueUpdateMode,
    incoming: IncomingValue<'_>,
) -> UpdateItemOutcome {
    let address = incoming.address();
    match mode {
        CueUpdateMode::ExistingOnly => {
            existing_only_outcome(cue_list, analysis, &address, incoming)
        }
        CueUpdateMode::ExistingInCurrentCue => {
            current_only_outcome(cue_list, analysis, current, &address, incoming)
        }
        CueUpdateMode::AddToCurrentCue => {
            add_current_outcome(cue_list, analysis, current, &address, incoming, false)
        }
        CueUpdateMode::AddNew => {
            add_current_outcome(cue_list, analysis, current, &address, incoming, true)
        }
    }
}

fn existing_only_outcome(
    cue_list: &CueList,
    analysis: &CueAnalysis,
    address: &UpdateAddress,
    incoming: IncomingValue<'_>,
) -> UpdateItemOutcome {
    match analysis.active_sources.get(address) {
        Some(location) if location.has_value => {
            let source = cue_source(cue_list, location.cue_index);
            if event_matches(cue_list, location, incoming) {
                UpdateItemOutcome::Unchanged {
                    source: Some(source),
                }
            } else {
                UpdateItemOutcome::ChangeAtSource { source }
            }
        }
        _ if analysis.all_addresses.contains(address) => UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NotInActiveTrackedState,
        },
        _ => UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NewAddress,
        },
    }
}

fn current_only_outcome(
    cue_list: &CueList,
    analysis: &CueAnalysis,
    current: &CueSource,
    address: &UpdateAddress,
    incoming: IncomingValue<'_>,
) -> UpdateItemOutcome {
    match analysis.current_explicit.get(address) {
        Some(location) if event_matches(cue_list, location, incoming) => {
            UpdateItemOutcome::Unchanged {
                source: Some(current.clone()),
            }
        }
        Some(_) => UpdateItemOutcome::ChangeInCurrentCue {
            cue: current.clone(),
        },
        None if analysis.all_addresses.contains(address) => UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NotInCurrentCue,
        },
        None => UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NewAddress,
        },
    }
}

fn add_current_outcome(
    cue_list: &CueList,
    analysis: &CueAnalysis,
    current: &CueSource,
    address: &UpdateAddress,
    incoming: IncomingValue<'_>,
    include_new: bool,
) -> UpdateItemOutcome {
    if !include_new && !analysis.all_addresses.contains(address) {
        return UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NewAddress,
        };
    }
    match analysis.current_any.get(address) {
        Some(location) if event_matches(cue_list, location, incoming) => {
            UpdateItemOutcome::Unchanged {
                source: Some(current.clone()),
            }
        }
        Some(_) => UpdateItemOutcome::ChangeInCurrentCue {
            cue: current.clone(),
        },
        None if analysis.all_addresses.contains(address) => UpdateItemOutcome::AddToCurrentCue {
            cue: current.clone(),
        },
        None => UpdateItemOutcome::AddNewToCurrentCue {
            cue: current.clone(),
        },
    }
}
