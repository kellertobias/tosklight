use super::*;
use light_programmer::PresetFamily;
use light_show::{PortableShowDocument, ShowStore};

fn tl784_document(objects: &[(&str, Preset)]) -> PortableShowDocument {
    let path = std::env::temp_dir().join(format!("tl784-alias-{}.show", Uuid::new_v4()));
    let (store, _) = ShowStore::create(&path, "TL784 aliases").unwrap();
    for (id, preset) in objects {
        store
            .put_object("preset", id, &serde_json::to_value(preset).unwrap(), 0)
            .unwrap();
    }
    let document = store.portable_document().unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
    document
}

fn mixed() -> Preset {
    Preset {
        number: 150,
        family: PresetFamily::Mixed,
        instance_id: Some(Uuid::from_u128(150)),
        ..Default::default()
    }
}

#[test]
fn tl784_preset_alias_resolves_actual_identity_without_rewriting_document() {
    let document = tl784_document(&[("150", mixed())]);
    let (object, preset) =
        super::super::resolution::find_recalled_preset(&document, "0.150").unwrap();
    assert_eq!(object.key().id(), "150");
    assert_eq!(object.revision(), 1);
    assert_eq!(preset.instance_id, Some(Uuid::from_u128(150)));
    assert!(document.object("preset", "0.150").is_none());
}

#[test]
fn tl784_preset_alias_refuses_ambiguity_even_when_exact_key_exists() {
    let mut second = mixed();
    second.instance_id = Some(Uuid::from_u128(151));
    let document = tl784_document(&[("150", mixed()), ("0.150", second)]);
    assert!(super::super::resolution::find_recalled_preset(&document, "0.150").is_err());
    assert!(super::super::resolution::find_recalled_preset(&document, "150").is_err());
}

#[test]
fn tl784_preset_missing_and_distinct_family_are_not_aliases() {
    let document = tl784_document(&[("150", mixed())]);
    assert!(super::super::resolution::find_recalled_preset(&document, "0.151").is_err());
    assert!(super::super::resolution::find_recalled_preset(&document, "2.150").is_err());
    let color = Preset {
        family: PresetFamily::Color,
        number: 150,
        ..Default::default()
    };
    let document = tl784_document(&[("150", mixed()), ("2.150", color)]);
    assert_eq!(
        super::super::resolution::find_recalled_preset(&document, "2.150")
            .unwrap()
            .0
            .key()
            .id(),
        "2.150"
    );
}

#[test]
fn tl784_resolution_does_not_normalize_legacy_body_or_match_opaque_aliases() {
    let mut legacy = mixed();
    legacy.number = 0;
    let document = tl784_document(&[("150", legacy), ("named-look", mixed())]);
    let (object, preset) =
        super::super::resolution::find_recalled_preset(&document, "0.150").unwrap();
    assert_eq!(preset.number, 0);
    assert_eq!(object.body()["number"], 0);
    assert_eq!(
        super::super::resolution::find_recalled_preset(&document, "named-look")
            .unwrap()
            .0
            .key()
            .id(),
        "named-look"
    );
}

#[test]
fn tl784_malformed_requested_identity_is_refused() {
    let mut bad = mixed();
    bad.number = 151;
    let document = tl784_document(&[("150", bad)]);
    assert!(super::super::resolution::find_recalled_preset(&document, "150").is_err());
    assert!(super::super::resolution::find_recalled_preset(&document, "0.150").is_err());
}

#[test]
fn tl784_menu_keeps_actual_key_and_omits_missing_or_ambiguous_sources() {
    let first = fixture(784);
    let mut preset = mixed();
    preset.values.insert(
        first,
        HashMap::from([(AttributeKey::intensity(), normalized(0.5))]),
    );
    let document = tl784_document(&[("150", preset.clone())]);
    let mut input = super::super::ProgrammingUpdateMenuInput {
        values: content(vec![fixture_update(first, "intensity", 0.8, 1)]),
        selection: Default::default(),
        values_fingerprint: "values".into(),
        selection_fingerprint: "selection".into(),
        active_preset_id: Some("0.150".into()),
        referenced_group_ids: vec![],
    };
    let menu = |document: &PortableShowDocument,
                input: &super::super::ProgrammingUpdateMenuInput| {
        super::super::menu::preview_update_menu(
            document,
            &[],
            input,
            UpdateTargetFilter::ShowAllActive,
        )
    };
    let entries = menu(&document, &input);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].object.object_id, "150");
    assert_eq!(
        entries[0].target,
        ProgrammingUpdateTargetRequest::Preset {
            object_id: "150".into()
        }
    );
    assert_eq!(entries[0].existing_preview.target.object_id, "150");
    let ambiguous = tl784_document(&[("150", preset.clone()), ("0.150", preset)]);
    assert!(menu(&ambiguous, &input).is_empty());
    input.active_preset_id = Some("0.151".into());
    assert!(menu(&document, &input).is_empty());
    input.active_preset_id = None;
    assert!(menu(&document, &input).is_empty());
}

#[test]
fn tl784_explicit_preview_and_mutation_ids_remain_exact() {
    let document = tl784_document(&[("150", mixed())]);
    assert!(super::super::resolution::find_preset(&document, "0.150").is_err());
    assert_eq!(
        super::super::resolution::find_preset(&document, "150")
            .unwrap()
            .0
            .key()
            .id(),
        "150"
    );
}
