//! Read-only compatibility diagnostics for show-local raw Highlight overrides.
//!
//! Plan 28 moves the normal operator look to installation settings. This scanner deliberately
//! does not migrate or normalize portable show objects: until the operator or a later migration
//! explicitly resolves a legacy look, the original show remains the compatibility authority.

use light_show::PortableShowDocument;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct HighlightCompatibilityReport {
    pub(super) show_id: light_core::ShowId,
    pub(super) show_revision: light_core::Revision,
    pub(super) patch_revision: light_core::Revision,
    pub(super) status: HighlightCompatibilityStatus,
    pub(super) fixtures: Vec<LegacyHighlightFixture>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum HighlightCompatibilityStatus {
    Compatible,
    NeedsReview,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct LegacyHighlightFixture {
    pub(super) object_id: String,
    pub(super) fixture_number: Option<u32>,
    pub(super) name: Option<String>,
    pub(super) override_count: usize,
    pub(super) issue: LegacyHighlightIssue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LegacyHighlightIssue {
    RawOverrides,
    InvalidOverrideShape,
}

fn inspect_document(document: &PortableShowDocument) -> HighlightCompatibilityReport {
    let fixtures = document
        .objects_of_kind("patched_fixture")
        .filter_map(|object| {
            let overrides = object.body().get("highlight_overrides")?;
            let (override_count, issue) = match overrides {
                serde_json::Value::Object(values) if values.is_empty() => return None,
                serde_json::Value::Object(values) => {
                    (values.len(), LegacyHighlightIssue::RawOverrides)
                }
                // Some transport projections use an array. Treat a stored non-empty array as
                // legacy state too, while leaving validation and repair to the explicit migration.
                serde_json::Value::Array(values) if values.is_empty() => return None,
                serde_json::Value::Array(values) => {
                    (values.len(), LegacyHighlightIssue::InvalidOverrideShape)
                }
                serde_json::Value::Null => return None,
                _ => (0, LegacyHighlightIssue::InvalidOverrideShape),
            };
            Some(LegacyHighlightFixture {
                object_id: object.key().id().to_owned(),
                fixture_number: object
                    .body()
                    .get("fixture_number")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|value| u32::try_from(value).ok()),
                name: object
                    .body()
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                override_count,
                issue,
            })
        })
        .collect::<Vec<_>>();
    HighlightCompatibilityReport {
        show_id: document.id(),
        show_revision: document.revision().value(),
        patch_revision: document.patch_revision().value(),
        status: if fixtures.is_empty() {
            HighlightCompatibilityStatus::Compatible
        } else {
            HighlightCompatibilityStatus::NeedsReview
        },
        fixtures,
    }
}

/// If an unreviewed installation opens legacy raw Highlight state, keep exact compatibility
/// evaluation active and persist a visible review requirement without modifying the show.
pub(super) fn require_review_for_show(
    state: &super::AppState,
    entry: &super::ShowEntry,
) -> Result<(), super::ApiError> {
    let configuration = state.installation.configuration();
    if configuration.highlight_legacy_overrides_acknowledged
        || configuration.highlight_look.compatibility
            != light_fixture::HighlightLookCompatibility::Semantic
    {
        return Ok(());
    }
    let store = super::ActiveShowRepository::open(&entry.path).map_err(super::ApiError::store)?;
    let report = inspect_document(&store.portable_document().map_err(super::ApiError::store)?);
    if report.status != HighlightCompatibilityStatus::NeedsReview {
        return Ok(());
    }
    let mut configuration = configuration;
    configuration.highlight_look.compatibility =
        light_fixture::HighlightLookCompatibility::NeedsReview;
    state
        .output
        .set_highlight_look(configuration.highlight_look.clone())
        .map_err(|error| super::ApiError::internal(error.to_string()))?;
    state.installation.replace_configuration(configuration);
    super::persist_server_configuration(state)?;
    super::emit(
        state,
        "highlight_compatibility_review_required",
        serde_json::json!({
            "show_id": report.show_id,
            "show_revision": report.show_revision,
            "patch_revision": report.patch_revision,
            "fixtures": report.fixtures,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_fixture::{
        PatchedFixturePatch, PatchedFixtureProfileReference, PortablePatchedFixtureRecord,
    };
    use light_show::ShowStore;
    use serde_json::json;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn inspect_highlight_compatibility(
        store: &ShowStore,
    ) -> Result<HighlightCompatibilityReport, light_show::StoreError> {
        Ok(inspect_document(&store.portable_document()?))
    }

    fn temporary_show(name: &str) -> (std::path::PathBuf, ShowStore) {
        let path = std::env::temp_dir().join(format!("light-{name}-{}.show", Uuid::new_v4()));
        let (store, _) = ShowStore::create(&path, name).unwrap();
        (path, store)
    }

    fn fixture_body(
        fixture_number: u32,
        overrides: BTreeMap<Uuid, u32>,
    ) -> (String, serde_json::Value) {
        let fixture_id = light_core::FixtureId::new();
        let record = PortablePatchedFixtureRecord::from_profile_reference(
            PatchedFixtureProfileReference {
                profile_id: light_core::FixtureId::new(),
                profile_revision: 1,
                mode_id: Uuid::new_v4(),
            },
            PatchedFixturePatch {
                fixture_id,
                fixture_number: Some(fixture_number),
                virtual_fixture_number: None,
                name: format!("Fixture {fixture_number}"),
                universe: Some(1),
                address: Some(fixture_number as u16),
                split_patches: Vec::new(),
                layer_id: "default".into(),
                direct_control: None,
                internal_bindings: Default::default(),
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: Vec::new(),
                multipatch: Vec::new(),
                group_masters_enabled: true,
                grand_master_enabled: true,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 0.0,
                shaper_angle: None,
                installed_appearance: Default::default(),
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
                highlight_overrides: overrides,
                freeze: Default::default(),
            },
        )
        .unwrap();
        (fixture_id.0.to_string(), record.into_body())
    }

    #[test]
    fn scanner_reports_needs_review_without_mutating_show_or_patch_revisions() {
        let (path, store) = temporary_show("highlight-compatibility-scan");
        let channel_id = Uuid::new_v4();
        let (fixture_id, mut body) = fixture_body(41, BTreeMap::from([(channel_id, 211)]));
        body["future_extension"] = json!({"preserved": [3, 1, 2]});
        store
            .put_object("patched_fixture", &fixture_id, &body, 0)
            .unwrap();
        let before = store.portable_document().unwrap();

        let report = inspect_highlight_compatibility(&store).unwrap();

        assert_eq!(report.show_id, before.id());
        assert_eq!(report.show_revision, before.revision().value());
        assert_eq!(report.patch_revision, before.patch_revision().value());
        assert_eq!(report.status, HighlightCompatibilityStatus::NeedsReview);
        assert_eq!(
            report.fixtures,
            vec![LegacyHighlightFixture {
                object_id: fixture_id.clone(),
                fixture_number: Some(41),
                name: Some("Fixture 41".into()),
                override_count: 1,
                issue: LegacyHighlightIssue::RawOverrides,
            }]
        );
        let after = store.portable_document().unwrap();
        assert_eq!(after, before, "diagnostics must be a byte-lossless read");
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn typed_patch_load_and_save_preserves_hidden_overrides_and_extensions() {
        let (path, store) = temporary_show("highlight-compatibility-save");
        let channel_id = Uuid::new_v4();
        let (fixture_id, mut original) = fixture_body(7, BTreeMap::from([(channel_id, 65_535)]));
        original["future_extension"] = json!({"preserved": true});
        let first_revision = store
            .put_object("patched_fixture", &fixture_id, &original, 0)
            .unwrap();

        let stored = store.objects("patched_fixture").unwrap().remove(0);
        let mut record = PortablePatchedFixtureRecord::decode(stored.body).unwrap();
        let mut patch = record.patch().unwrap();
        patch.name = "Renamed fixture".into();
        record.update_patch(&patch).unwrap();
        store
            .put_object(
                "patched_fixture",
                &fixture_id,
                record.body(),
                first_revision,
            )
            .unwrap();

        drop(store);
        let reopened = ShowStore::open(&path).unwrap();
        let saved = reopened.objects("patched_fixture").unwrap().remove(0).body;
        assert_eq!(saved["name"], "Renamed fixture");
        assert_eq!(saved["highlight_overrides"][channel_id.to_string()], 65_535);
        assert_eq!(saved["future_extension"], json!({"preserved": true}));
        assert_eq!(
            inspect_highlight_compatibility(&reopened).unwrap().status,
            HighlightCompatibilityStatus::NeedsReview
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn empty_or_absent_override_maps_are_compatible_but_malformed_state_needs_review() {
        let (path, store) = temporary_show("highlight-compatibility-shapes");
        let (empty_id, empty) = fixture_body(1, BTreeMap::new());
        store
            .put_object("patched_fixture", &empty_id, &empty, 0)
            .unwrap();
        assert_eq!(
            inspect_highlight_compatibility(&store).unwrap().status,
            HighlightCompatibilityStatus::Compatible
        );

        let absent_id = light_core::FixtureId::new().0.to_string();
        store
            .put_object(
                "patched_fixture",
                &absent_id,
                &json!({
                    "fixture_id": absent_id,
                    "fixture_number": 2,
                    "name": "No legacy Highlight field"
                }),
                0,
            )
            .unwrap();
        assert_eq!(
            inspect_highlight_compatibility(&store).unwrap().status,
            HighlightCompatibilityStatus::Compatible
        );

        let malformed_id = light_core::FixtureId::new().0.to_string();
        store
            .put_object(
                "patched_fixture",
                &malformed_id,
                &json!({
                    "fixture_id": malformed_id,
                    "fixture_number": 3,
                    "name": "Malformed legacy Highlight",
                    "highlight_overrides": "raw legacy data"
                }),
                0,
            )
            .unwrap();
        let report = inspect_highlight_compatibility(&store).unwrap();
        assert_eq!(report.status, HighlightCompatibilityStatus::NeedsReview);
        assert_eq!(report.fixtures.len(), 1);
        assert_eq!(
            report.fixtures[0].issue,
            LegacyHighlightIssue::InvalidOverrideShape
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
