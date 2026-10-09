//! Resolve live Preset identities against one portable candidate before installing Cue tracking.
use light_core::{AttributeValue, PresetValueOwner, PresetValueReference, programming::*};
use light_engine::NativeColorSourceCatalog;
use light_programmer::Preset;
use std::collections::HashMap;

pub(super) fn resolve(
    reference: &PresetValueReference,
    presets: &HashMap<uuid::Uuid, Preset>,
    native: &NativeColorSourceCatalog,
) -> Option<AttributeValue> {
    let preset = presets.get(&reference.preset_instance_id)?;
    let source = match &reference.source_owner {
        PresetValueOwner::Universal => &preset.universal_values,
        PresetValueOwner::Fixture { fixture_id } => preset.values.get(fixture_id)?,
        PresetValueOwner::Group { group_id } => preset.group_values.get(group_id)?,
    };
    let mut value = source.get(&reference.source_attribute)?.clone();
    if let Some(fixture) = reference.member_fixture
        && let AttributeValue::GroupFamily(group) = &value
    {
        value = group
            .members
            .get(&fixture.0)
            .unwrap_or(&group.template)
            .clone();
    }
    let Some((rank, count)) = reference.sample_rank else {
        return Some(value);
    };
    if count == 0 || rank >= count {
        return None;
    }
    let native_model = match &value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Direct { recipe, .. } if value.spread_control_points() > 0 => {
                Some(native.resolve(&recipe.source).ok()?)
            }
            _ => None,
        },
        _ => None,
    };
    let context = FamilyEditContext {
        color_model: Some(&VirtualColorAuthoringV1),
        native_model: native_model
            .as_deref()
            .map(|model| model as &dyn NativeColorEditModel),
        ..Default::default()
    };
    compile_programming_spread(&value, count, &context)
        .ok()?
        .at_rank(rank)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{AttributeKey, FixtureId};

    #[test]
    fn identity_owner_and_rank_survive_edit_move_but_never_rebind_recreated_number() {
        let fixture = FixtureId::new();
        let id = uuid::Uuid::new_v4();
        let intensity = AttributeKey("intensity".into());
        let reference = PresetValueReference {
            preset_instance_id: id,
            source_owner: PresetValueOwner::Fixture {
                fixture_id: fixture,
            },
            source_attribute: intensity.clone(),
            sample_rank: None,
            member_fixture: Some(fixture),
        };
        let mut preset = Preset {
            instance_id: Some(id),
            number: 12,
            ..Default::default()
        };
        preset.values.insert(
            fixture,
            HashMap::from([(intensity.clone(), AttributeValue::Normalized(0.5))]),
        );
        let native = NativeColorSourceCatalog::default();
        let mut catalog = HashMap::from([(id, preset.clone())]);
        assert_eq!(
            resolve(&reference, &catalog, &native),
            Some(AttributeValue::Normalized(0.5))
        );
        preset.number = 99;
        preset
            .values
            .get_mut(&fixture)
            .unwrap()
            .insert(intensity.clone(), AttributeValue::Normalized(0.25));
        catalog.insert(id, preset.clone());
        assert_eq!(
            resolve(&reference, &catalog, &native),
            Some(AttributeValue::Normalized(0.25))
        );
        catalog.remove(&id);
        let replacement = uuid::Uuid::new_v4();
        preset.instance_id = Some(replacement);
        catalog.insert(replacement, preset);
        assert_eq!(resolve(&reference, &catalog, &native), None);
    }
    #[test]
    fn live_group_templates_and_frozen_rank_sources_remain_distinct() {
        let id = uuid::Uuid::new_v4();
        let attribute = AttributeKey::intensity();
        let value = AttributeValue::Spread(vec![0.2, 0.8]);
        let mut preset = Preset {
            instance_id: Some(id),
            ..Default::default()
        };
        preset.group_values.insert(
            "odd".into(),
            HashMap::from([(attribute.clone(), value.clone())]),
        );
        let reference = PresetValueReference {
            preset_instance_id: id,
            source_owner: PresetValueOwner::Group {
                group_id: "odd".into(),
            },
            source_attribute: attribute,
            sample_rank: None,
            member_fixture: None,
        };
        let catalog = HashMap::from([(id, preset)]);
        let native = NativeColorSourceCatalog::default();
        assert_eq!(resolve(&reference, &catalog, &native), Some(value));
        let frozen = PresetValueReference {
            sample_rank: Some((1, 3)),
            member_fixture: Some(FixtureId::new()),
            ..reference
        };
        let sampled = resolve(&frozen, &catalog, &native)
            .unwrap()
            .normalized()
            .unwrap();
        assert!((sampled - 0.5).abs() < 0.0001);
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::prepare_show_candidate;
    use light_core::{AttributeKey, FixtureId};
    use light_show::{ProgrammingContractMarker, ShowStore};
    use serde_json::json;

    #[test]
    fn legacy_identity_migration_and_live_link_survive_actual_sqlite_reopen() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("LIGHT_TMP_DIR").expect("initialize canonical artifact paths"),
        );
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("live-preset-{}.sqlite", uuid::Uuid::new_v4()));
        let (store, _) = ShowStore::create(&path, "Live source reopen").unwrap();
        let fixture = FixtureId::new();
        let attribute = AttributeKey::intensity();
        let legacy = Preset {
            values: HashMap::from([(
                fixture,
                HashMap::from([(attribute.clone(), AttributeValue::Normalized(0.7))]),
            )]),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            ..Default::default()
        };
        store
            .put_object("preset", "1.1", &serde_json::to_value(&legacy).unwrap(), 0)
            .unwrap();
        let document = store.portable_document().unwrap();
        let (mut migration, _) = prepare_show_candidate(&document, document.transaction())
            .unwrap()
            .into_parts();
        migration.stamp_programming_contract(2);
        store.apply_portable_transaction(migration).unwrap();
        let document = store.portable_document().unwrap();
        let migrated: Preset =
            serde_json::from_value(document.object("preset", "1.1").unwrap().body().clone())
                .unwrap();
        let identity = migrated.instance_id.unwrap();
        assert_eq!(migrated.values, legacy.values);
        assert_eq!(
            document.programming_contract_marker(),
            ProgrammingContractMarker::Declared(1)
        );
        let (again, _) = prepare_show_candidate(&document, document.transaction())
            .unwrap()
            .into_parts();
        assert!(again.is_empty(), "identity migration is applied once");
        let reference = PresetValueReference {
            preset_instance_id: identity,
            source_owner: PresetValueOwner::Fixture {
                fixture_id: fixture,
            },
            source_attribute: attribute.clone(),
            sample_rank: None,
            member_fixture: Some(fixture),
        };
        let id = uuid::Uuid::new_v4();
        let cue = json!({"id":id,"name":"Linked","priority":0,"mode":"sequence","looped":false,"cues":[{"id":uuid::Uuid::new_v4(),"number":"1","name":"Linked","changes":[{"fixture_id":fixture,"attribute":"intensity","value":AttributeValue::Normalized(0.7),"preset_reference":reference}],"dynamic_changes":[],"fade_millis":0,"delay_millis":0,"trigger":{"type":"manual"}}]});
        let mut transaction = document.transaction();
        transaction.put("cue_list", id.to_string(), cue);
        let (mut transaction, _) = prepare_show_candidate(&document, transaction)
            .unwrap()
            .into_parts();
        transaction.check_programming_contract(2).unwrap();
        transaction.stamp_programming_contract(2);
        store.apply_portable_transaction(transaction).unwrap();
        drop(store);
        let reopened = ShowStore::open(&path).unwrap();
        let document = reopened.portable_document().unwrap();
        assert_eq!(
            document.programming_contract_marker(),
            ProgrammingContractMarker::Declared(2)
        );
        let source: Preset =
            serde_json::from_value(document.object("preset", "1.1").unwrap().body().clone())
                .unwrap();
        assert_eq!(source.instance_id, Some(identity));
        let (transaction, snapshot) = prepare_show_candidate(&document, document.transaction())
            .unwrap()
            .into_parts();
        assert!(transaction.is_empty());
        assert_eq!(
            snapshot.cue_lists[0].cues[0].changes[0].value,
            Some(AttributeValue::Normalized(0.7))
        );
        assert_eq!(
            snapshot.cue_lists[0].cues[0].changes[0]
                .preset_reference
                .as_ref()
                .unwrap()
                .preset_instance_id,
            identity
        );
        drop(reopened);
        std::fs::remove_file(&path).unwrap();
    }
}
