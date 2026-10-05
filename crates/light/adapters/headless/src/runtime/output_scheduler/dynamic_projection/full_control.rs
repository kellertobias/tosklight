//! Detects playbacks whose every Dynamic output is fully overridden by a persistent source, so
//! auto-off can release them.

use super::*;

pub(in crate::runtime) struct DynamicPlaybackControl {
    pub(in crate::runtime) identity: PlaybackIdentity,
    pub(in crate::runtime) master: f32,
    pub(in crate::runtime) crossfade_non_intensity: bool,
    pub(in crate::runtime) auto_off_full_control: bool,
    pub(in crate::runtime) temporary_only: bool,
}

pub(in crate::runtime) fn fully_controlled_dynamic_playbacks(
    engine: &Engine,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<PlaybackIdentity> {
    if !controls
        .values()
        .any(|control| control.auto_off_full_control)
    {
        return Vec::new();
    }
    let persistent = engine.playback_contributions_at(now);
    fully_controlled_dynamic_playbacks_from(
        persistent
            .iter()
            .map(|candidate| (candidate.source, &candidate.value)),
        samples,
        controls,
        runtime,
        programmer_values,
        cue_values,
    )
}

pub(super) fn fully_controlled_dynamic_playbacks_from<'a>(
    playback: impl IntoIterator<Item = (light_playback::SequenceMasterSource, &'a TimedValue)>,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> Vec<PlaybackIdentity> {
    if !controls
        .values()
        .any(|control| control.auto_off_full_control)
    {
        return Vec::new();
    }
    let persistent = playback
        .into_iter()
        .filter(|(source, _)| !source.temporary)
        .collect::<Vec<_>>();
    let mut addresses = HashMap::<
        PlaybackIdentity,
        Vec<(
            light_dynamics::LegacyDynamicSample<'_>,
            &DynamicPlaybackControl,
        )>,
    >::new();
    let controller_sources = runtime
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .controllers
                .iter()
                .map(|controller| (controller.id, controller.source.clone()))
        })
        .collect::<HashMap<_, _>>();
    let persistent_fat = persistent_fat_values(programmer_values, cue_values);
    let temporary_cue_controllers = cue_values
        .iter()
        .filter(|row| row.source.temporary)
        .filter_map(|row| {
            row.value
                .track_key()
                .instance_link
                .map(|link| row.source_key.controller_id(link))
        })
        .collect::<HashSet<_>>();
    for sample in samples
        .iter()
        .filter_map(light_dynamics::DynamicRuntimeSample::legacy)
    {
        if let Some(control) = controls.get(&sample.controller_id)
            && control.auto_off_full_control
        {
            addresses
                .entry(control.identity)
                .or_default()
                .push((sample, control));
        }
    }
    addresses
        .into_iter()
        .filter_map(|(identity, target_samples)| {
            (!target_samples.is_empty()
                && target_samples.iter().all(|(sample, control)| {
                    let dynamic_value = if sample.attribute.is_level() {
                        sample.value * control.master
                    } else {
                        sample.value
                    };
                    persistent.iter().any(|(source, candidate)| {
                        candidate_playback_identity(*source).is_some_and(|other| other != identity)
                            && candidate.fixture_id == sample.target
                            && candidate.attribute == *sample.attribute
                            && persistent_playback_wins_dynamic(candidate, sample, dynamic_value)
                    }) || samples
                        .iter()
                        .filter_map(light_dynamics::DynamicRuntimeSample::legacy)
                        .any(|candidate| {
                            candidate.controller_id != sample.controller_id
                                && candidate.target == sample.target
                                && candidate.attribute == sample.attribute
                                && candidate.activation_mix >= 1.0
                                && controller_sources
                                    .get(&candidate.controller_id)
                                    .is_some_and(|source| match source {
                                        light_dynamics::DynamicControllerSource::Playback {
                                            ..
                                        } => controls.get(&candidate.controller_id).is_some_and(
                                            |control| {
                                                control.identity != identity
                                                    && !control.temporary_only
                                            },
                                        ),
                                        light_dynamics::DynamicControllerSource::Programmer {
                                            ..
                                        } => true,
                                        light_dynamics::DynamicControllerSource::Cue { .. } => {
                                            !temporary_cue_controllers
                                                .contains(&candidate.controller_id)
                                        }
                                    })
                                && persistent_dynamic_wins_dynamic(&candidate, sample)
                        })
                        || persistent_fat.iter().any(|candidate| {
                            candidate.fixture_id == sample.target
                                && candidate.attribute == *sample.attribute
                                && persistent_semantic_wins_dynamic(
                                    candidate.priority,
                                    candidate.changed_at_millis,
                                    sample,
                                )
                        })
                }))
            .then_some(identity)
        })
        .collect()
}

#[derive(Clone)]
pub(super) struct PersistentFatValue {
    fixture_id: FixtureId,
    attribute: AttributeKey,
    priority: i16,
    changed_at_millis: u64,
}

pub(super) fn persistent_fat_values(
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> Vec<PersistentFatValue> {
    let programmer = programmer_values
        .iter()
        .filter_map(|(_, priority, stored)| {
            matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
                    | light_dynamics::DynamicSemanticValue::Static { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute.clone(),
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    let cues = cue_values
        .iter()
        .filter(|row| row.output_enabled && !row.source.temporary)
        .filter_map(|stored| {
            matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
                    | light_dynamics::DynamicSemanticValue::Static { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute.clone(),
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    programmer.chain(cues).collect()
}

fn candidate_playback_identity(
    source: light_playback::SequenceMasterSource,
) -> Option<PlaybackIdentity> {
    source.playback_identity.or_else(|| {
        source
            .playback_number
            .and_then(|number| PlaybackIdentity::physical(number).ok())
    })
}

fn persistent_dynamic_wins_dynamic(
    candidate: &light_dynamics::DynamicRuntimeSample,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (
        candidate.priority,
        candidate.activated_at_millis,
        candidate.controller_id,
    ) > (
        dynamic.priority,
        dynamic.activated_at_millis,
        dynamic.controller_id,
    )
}

fn persistent_semantic_wins_dynamic(
    priority: i16,
    changed_at_millis: u64,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (priority, changed_at_millis) > (dynamic.priority, dynamic.activated_at_millis)
}

fn persistent_playback_wins_dynamic(
    candidate: &TimedValue,
    dynamic: &light_dynamics::DynamicRuntimeSample,
    dynamic_value: f32,
) -> bool {
    if candidate.priority != dynamic.priority {
        return candidate.priority > dynamic.priority;
    }
    if candidate.merge_mode == MergeMode::Htp {
        return candidate.value.normalized().unwrap_or(0.0) > dynamic_value;
    }
    u64::try_from(candidate.changed_at.timestamp_millis()).unwrap_or_default()
        > dynamic.activated_at_millis
}
