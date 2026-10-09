use super::*;
use light_core::ReplacementRuntimeMigration;

fn map_addresses<T: Clone>(
    values: &HashMap<AttributeAddress, T>,
    plan: &ReplacementRuntimeMigration,
) -> Result<HashMap<AttributeAddress, T>, String> {
    let mut migrated = HashMap::new();
    for ((owner, attribute), value) in values {
        for (destination, _) in
            crate::replacement_runtime::replacement_destinations(plan, *owner, attribute, None)?
        {
            if migrated
                .insert((destination, attribute.clone()), value.clone())
                .is_some()
            {
                return Err("replacement merges distinct retained evidence addresses; choose distinct destinations".into());
            }
        }
    }
    Ok(migrated)
}
fn cache(
    value: &PlaybackEvidenceCache,
    plan: &ReplacementRuntimeMigration,
) -> Result<Arc<PlaybackEvidenceCache>, String> {
    Ok(Arc::new(PlaybackEvidenceCache {
        generation: value.generation.clone(),
        target_index: value.target_index,
        target_wrap: value.target_wrap,
        targets: map_addresses(&value.targets, plan)?,
        phases: map_addresses(&value.phases, plan)?,
    }))
}
impl PlaybackSourceHistory {
    pub(crate) fn apply_replacement_runtime_migration(
        &mut self,
        plan: &ReplacementRuntimeMigration,
    ) -> Result<(), String> {
        if let Some(prior) = &self.prior {
            self.prior = Some(cache(prior, plan)?);
        }
        if let Some(current) = self.cache.get() {
            self.cache = Arc::new(OnceLock::from(cache(current, plan)?));
        }
        if let Some(endpoints) = &self.endpoint_overrides {
            self.endpoint_overrides = Some(Arc::new(map_addresses(endpoints, plan)?));
        }
        if let Some(leg) = &self.manual {
            let mut migrated = (**leg).clone();
            migrated.from = Arc::new(map_addresses(&leg.from, plan)?);
            if let Some(base) = &mut migrated.base {
                base.apply_replacement_runtime_migration(plan)?;
            }
            self.manual = Some(Arc::new(migrated));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{
        ReplacementHeadTarget, ReplacementProfileContext, ReplacementProgramProjection,
    };
    use std::collections::HashSet;

    #[test]
    fn replacement_runtime_history_keeps_manual_cache_occurrences_and_dormant_decision() {
        let root = FixtureId::new();
        let children = [FixtureId::new(), FixtureId::new()];
        let attribute = AttributeKey("focus".into());
        let mut first = Cue::new(CueNumber::try_from_legacy_f64(1.0).unwrap());
        first.changes.push(CueChange::set(
            root,
            attribute.clone(),
            AttributeValue::Normalized(0.2),
        ));
        let mut second = Cue::new(CueNumber::try_from_legacy_f64(2.0).unwrap());
        second.changes.push(CueChange::set(
            root,
            attribute.clone(),
            AttributeValue::Normalized(0.8),
        ));
        let list = CueList {
            id: CueListId::new(),
            name: "Retained manual leg".into(),
            pool_number: None,
            legacy_pool_aliases: Vec::new(),
            priority: 10,
            mode: CueListMode::Sequence,
            looped: false,
            intensity_priority_mode: IntensityPriorityMode::Htp,
            wrap_mode: Some(WrapMode::Off),
            restart_mode: RestartMode::FirstCue,
            force_cue_timing: false,
            disable_cue_timing: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            chaser_step_millis: 1000,
            chaser_xfade_millis: 0,
            chaser_xfade_percent: Some(0),
            speed_group: None,
            speed_multiplier: 1.0,
            cues: vec![first, second],
        };
        let compiled = Arc::new(CompiledCueList::new(&list));
        let now = Utc::now();
        let source = SequenceMasterSource {
            cue_list_id: list.id,
            playback_number: None,
            playback_identity: None,
            temporary: false,
        };
        let previous =
            PlaybackSourceHistory::cue_leg(None, &compiled, source, now, 7, 0, 0, false, false);
        let mut history = PlaybackSourceHistory::cue_leg(
            Some(&previous),
            &compiled,
            source,
            now,
            8,
            0,
            1,
            false,
            true,
        );
        let original_target =
            history.cache.get().unwrap().targets[&(root, attribute.clone())].clone();
        let original_from =
            history.manual.as_ref().unwrap().from[&(root, attribute.clone())].clone();
        let context = ReplacementProfileContext {
            profile_id: FixtureId::new(),
            profile_revision: 1,
            mode_id: Uuid::new_v4(),
        };
        let target = ReplacementProfileContext {
            profile_revision: 2,
            ..context.clone()
        };
        let projection = ReplacementProgramProjection {
            source_owner: root,
            source_profile: context.clone(),
            source_head_id: Uuid::new_v4(),
            target_profile: target.clone(),
            targets: children
                .iter()
                .map(|fixture_id| ReplacementHeadTarget {
                    fixture_id: *fixture_id,
                    profile_head_id: Uuid::new_v4(),
                })
                .collect(),
        };
        let mut plan = ReplacementRuntimeMigration {
            source_head_owners: HashMap::from([(projection.source_head_id, root)]),
            source_owner: root,
            source_profile: context,
            target_profile: target,
            root_attributes: HashSet::from([attribute.clone()]),
            root_projections: HashMap::from([(attribute.clone(), projection)]),
            head_targets: HashMap::new(),
        };
        history.apply_replacement_runtime_migration(&plan).unwrap();
        assert_eq!(history.action_ordinal(), 8);
        assert_eq!(history.action_changed_at(), now);
        assert_eq!(history.cache.get().unwrap().targets.len(), 2);
        for child in children {
            let destination = (child, attribute.clone());
            let target = &history.cache.get().unwrap().targets[&destination];
            assert_eq!(target.change, original_target.change);
            assert!(Arc::ptr_eq(
                target.evidence.as_ref().unwrap(),
                original_target.evidence.as_ref().unwrap()
            ));
            let from = &history.manual.as_ref().unwrap().from[&destination];
            assert_eq!(from.change, original_from.change);
            assert!(Arc::ptr_eq(
                from.evidence.as_ref().unwrap(),
                original_from.evidence.as_ref().unwrap()
            ));
            assert!(
                history
                    .manual
                    .as_ref()
                    .unwrap()
                    .base
                    .as_ref()
                    .unwrap()
                    .cache
                    .get()
                    .unwrap()
                    .targets
                    .contains_key(&destination)
            );
        }
        let mut dormant = previous;
        plan.root_projections
            .get_mut(&attribute)
            .unwrap()
            .targets
            .clear();
        dormant.apply_replacement_runtime_migration(&plan).unwrap();
        assert!(dormant.cache.get().unwrap().targets.is_empty());
        assert!(dormant.cache.get().unwrap().phases.is_empty());
        assert_eq!(dormant.action_ordinal(), 7);
    }
}
