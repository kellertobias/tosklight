//! TL-587: activation configuration is prepared once from the exact compiled portable document.
//!
//! The legacy installers (`AttributeConfigurationResource::install_entry` and
//! `psn_http::install_current_show`) reopen the show path and silently fall back to defaults. A
//! prepared activation bundle must instead carry the accepted document (or the document advanced
//! by its own migration commit) and install from memory only.

use super::*;
use attribute_configuration::InstalledAttributeConfiguration;
use psn_http::PsnConfigurationOrigin;

const PSN_KIND: &str = "psn";
const PSN_ID: &str = "main";

struct TempShow {
    data_dir: PathBuf,
    path: PathBuf,
    entry: ShowEntry,
}

impl TempShow {
    fn create(label: &str, name: &str) -> (Self, ShowStore) {
        let data_dir = std::env::temp_dir().join(format!("light-tl587-{label}-{}", Uuid::new_v4()));
        let path = data_dir.join("shows").join(format!("{label}.show"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let (store, _) = ShowStore::create(&path, name).unwrap();
        let id = store.portable_document().unwrap().id();
        let entry = Self::entry(&path, id, name);
        (
            Self {
                data_dir,
                path,
                entry,
            },
            store,
        )
    }

    fn legacy(label: &str, name: &str) -> (Self, ShowStore) {
        let data_dir = std::env::temp_dir().join(format!("light-tl587-{label}-{}", Uuid::new_v4()));
        let path = data_dir.join("shows").join(format!("{label}.show"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let id = default_show::initialise_legacy_test_show(&path).unwrap();
        let store = ShowStore::open(&path).unwrap();
        let entry = Self::entry(&path, id, name);
        (
            Self {
                data_dir,
                path,
                entry,
            },
            store,
        )
    }

    fn entry(path: &FsPath, id: light_core::ShowId, name: &str) -> ShowEntry {
        ShowEntry {
            is_base_show: false,
            id,
            name: name.into(),
            path: path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            created_at: None,
            last_loaded_at: None,
            revision_copy: None,
        }
    }

    fn fresh_document(&self) -> light_show::PortableShowDocument {
        ShowStore::open(&self.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn backups(&self) -> usize {
        std::fs::read_dir(self.data_dir.join("backups"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_str()
                            .is_some_and(|name| name.contains("-migration-"))
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Removes the show file and its SQLite sidecars.
    fn delete_file(&self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}

impl Drop for TempShow {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

fn prepare_activation(
    show: &TempShow,
) -> Result<PreparedShowActivation<EngineSnapshot>, ShowLoadError> {
    let backup = ShowMutationBackupPlan::migration(&show.data_dir, &show.entry, 5);
    prepare_show_load(&show.entry, None)?
        .prepare_runtime(Ok)?
        .commit_activation(&backup)
}

fn intent_configuration() -> light_core::AttributeConfiguration {
    let mut configuration = light_core::AttributeConfiguration::recommended();
    configuration.color_model = light_core::ColorProgrammingModel::Intent;
    configuration
}

fn stored_psn(port: u16) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "enabled": true,
        "group": "236.10.10.10",
        "port": port,
        "staleAfterMillis": 1500,
        "bindings": [{
            "id": Uuid::new_v4(),
            "trackerId": 7,
            "pointFixtureId": Uuid::new_v4(),
            "enabled": true
        }]
    })
}

/// A legacy placement the canonical-attribute migration retires, so the stored Attribute
/// configuration object itself is rewritten by the staged migration.
fn legacy_attribute_configuration() -> serde_json::Value {
    let mut configuration = intent_configuration();
    configuration
        .placements
        .push(light_core::AttributePlacement {
            attribute: light_core::AttributeKey("frost.1".into()),
            encoder: light_core::EncoderPlacement::new(light_core::EncoderGroup::Focus, 1, 3),
            push_turn_of: None,
        });
    configuration
        .activation_groups
        .push(light_core::AttributeActivationGroup {
            id: "frost.1".into(),
            label: "Frost 1".into(),
            members: vec![light_core::AttributeKey("frost.1".into())],
        });
    serde_json::to_value(configuration).unwrap()
}

fn legacy_midi_mapping() -> serde_json::Value {
    serde_json::json!({
        "name": "Legacy MIDI Go",
        "enabled": true,
        "trigger": {"type": "midi", "status": 144, "data1": 7},
        "action": {"type": "cue_go", "cue_list_id": Uuid::nil()}
    })
}

/// Everything preparation must leave alone on the live desk.
#[derive(Debug, PartialEq)]
struct LiveFingerprint {
    attributes: (Option<light_core::ShowId>, u64, u64, Option<String>),
    attribute_configuration: light_core::AttributeConfiguration,
    psn_configuration: light_application::PsnConfiguration,
    psn_source_generation: u64,
    psn_owner: Option<light_core::ShowId>,
    event_sequence: u64,
    tracking_frame: usize,
    color_model: light_core::ColorProgrammingModel,
}

fn live_fingerprint(state: &AppState) -> LiveFingerprint {
    let attributes = state.attributes.snapshot();
    LiveFingerprint {
        attributes: (
            attributes.show_id,
            attributes.show_revision,
            attributes.object_revision,
            attributes.validation_error.clone(),
        ),
        attribute_configuration: attributes.configuration,
        psn_configuration: state.psn.configuration(),
        psn_source_generation: state.psn.generation(),
        psn_owner: state.psn.committed_tracking_frame(0).show_id,
        event_sequence: state.events.latest_sequence(),
        tracking_frame: Arc::as_ptr(&state.output.engine().tracking_frame()) as *const () as usize,
        color_model: state.attributes.color_model(),
    }
}

fn assert_bundle_is_coherent<T>(activation: &PreparedShowActivation<T>) {
    let document = activation.document();
    let configuration = activation.configuration();
    assert_eq!(configuration.show_id(), document.id());
    assert_eq!(configuration.show_revision(), document.revision().value());
    assert_eq!(configuration.attributes().show_id, Some(document.id()));
    assert_eq!(
        configuration.attributes().show_revision,
        document.revision().value()
    );
    assert_eq!(configuration.psn().show_id, document.id());
    assert_eq!(
        configuration.psn().show_revision,
        document.revision().value()
    );
}

#[test]
fn unchanged_show_prepares_configuration_from_the_accepted_document_without_writing() {
    let (show, store) = TempShow::create("unchanged", "Unchanged Tracking");
    let attributes = serde_json::to_value(intent_configuration()).unwrap();
    store
        .put_object("attribute_configuration", "default", &attributes, 0)
        .unwrap();
    store
        .put_object(PSN_KIND, PSN_ID, &stored_psn(7000), 0)
        .unwrap();
    let source = store.portable_document().unwrap();
    drop(store);

    let activation = prepare_activation(&show).unwrap();

    assert_eq!(
        activation.document(),
        &source,
        "no migration keeps the accepted document"
    );
    assert_eq!(activation.runtime().revision, source.revision().value());
    assert_bundle_is_coherent(&activation);
    let prepared = activation.configuration();
    assert_eq!(
        prepared.attributes().object_revision,
        source
            .object("attribute_configuration", "default")
            .unwrap()
            .revision()
    );
    assert_eq!(
        prepared.attributes().configuration.color_model,
        light_core::ColorProgrammingModel::Intent
    );
    assert!(prepared.attributes().validation_error.is_none());
    assert_eq!(prepared.psn().origin, PsnConfigurationOrigin::Stored);
    assert_eq!(prepared.psn().configuration.port, 7000);
    assert!(prepared.psn().configuration.enabled);
    assert_eq!(
        prepared.psn().object_revision,
        source.object(PSN_KIND, PSN_ID).unwrap().revision()
    );
    assert_eq!(
        show.fresh_document(),
        source,
        "unchanged preparation writes nothing"
    );
    assert_eq!(show.backups(), 0);
}

#[test]
fn migrated_show_prepares_configuration_from_the_committed_document() {
    let (state, _state_dir) = test_state();
    let (show, store) = TempShow::create("migrated", "Migrated Tracking");
    store
        .put_object(
            "attribute_configuration",
            "default",
            &legacy_attribute_configuration(),
            0,
        )
        .unwrap();
    store
        .put_object("control_mapping", "midi-go", &legacy_midi_mapping(), 0)
        .unwrap();
    store
        .put_object(PSN_KIND, PSN_ID, &stored_psn(7001), 0)
        .unwrap();
    let source = store.portable_document().unwrap();
    let source_attribute_revision = source
        .object("attribute_configuration", "default")
        .unwrap()
        .revision();
    drop(store);
    let live_before = live_fingerprint(&state);

    let activation = prepare_activation(&show).unwrap();

    assert_eq!(
        live_fingerprint(&state),
        live_before,
        "preparation touches no live state"
    );
    let committed = show.fresh_document();
    assert!(
        committed.revision() > source.revision(),
        "the staged migration was committed"
    );
    assert_eq!(
        activation.document(),
        &committed,
        "the carried document is exactly the committed migration result"
    );
    assert_eq!(activation.runtime().revision, committed.revision().value());
    assert_bundle_is_coherent(&activation);
    let committed_attributes = committed
        .object("attribute_configuration", "default")
        .unwrap();
    assert_ne!(
        committed_attributes.revision(),
        source_attribute_revision,
        "the migration rewrote the Attribute configuration object"
    );
    let prepared = activation.configuration();
    assert_eq!(
        prepared.attributes().object_revision,
        committed_attributes.revision(),
        "configuration derives from the committed, not the source, document"
    );
    assert_eq!(
        prepared.attributes().show_revision,
        committed.revision().value()
    );
    assert!(prepared.attributes().validation_error.is_none());
    assert!(
        prepared
            .attributes()
            .configuration
            .placements
            .iter()
            .all(|placement| placement.attribute.0.as_ref() != "frost.1")
    );
    assert_eq!(prepared.psn().configuration.port, 7001);
    assert_eq!(show.backups(), 1);
}

#[test]
fn file_replacement_and_deletion_after_preparation_do_not_change_installation() {
    let (state, _state_dir) = test_state();
    let (show, store) = TempShow::create("replaced", "Prepared Tracking");
    store
        .put_object(
            "attribute_configuration",
            "default",
            &serde_json::to_value(intent_configuration()).unwrap(),
            0,
        )
        .unwrap();
    store
        .put_object(PSN_KIND, PSN_ID, &stored_psn(7002), 0)
        .unwrap();
    drop(store);
    let activation = prepare_activation(&show).unwrap();
    let prepared = activation.configuration().clone();

    // Replace the file with a different show that has no Attribute or PSN objects.
    show.delete_file();
    let (replacement, _) = ShowStore::create(&show.path, "Replacement").unwrap();
    let replacement_id = replacement.portable_document().unwrap().id();
    drop(replacement);
    let reread = InstalledAttributeConfiguration::for_entry(Some(&show.entry));
    assert_eq!(
        reread.show_id,
        Some(replacement_id),
        "control: the legacy installer rereads the path and adopts another document"
    );

    prepared.install_memory_only(&state);
    let installed = state.attributes.snapshot();
    assert_eq!(installed.show_id, Some(prepared.show_id()));
    assert_eq!(installed.show_revision, prepared.show_revision());
    assert_eq!(installed.configuration, prepared.attributes().configuration);
    assert!(installed.validation_error.is_none());
    assert_eq!(state.psn.configuration(), prepared.psn().configuration);
    assert_eq!(state.psn.configuration().port, 7002);
    assert_eq!(
        state.psn.committed_tracking_frame(0).show_id,
        Some(prepared.show_id())
    );
    assert_eq!(
        state.attributes.color_model(),
        light_core::ColorProgrammingModel::Intent
    );

    // Deleting the file afterwards is equally invisible to the prepared installation.
    show.delete_file();
    prepared.install_memory_only(&state);
    let installed = state.attributes.snapshot();
    assert_eq!(installed.show_id, Some(prepared.show_id()));
    assert!(installed.validation_error.is_none());
    assert_eq!(state.psn.configuration(), prepared.psn().configuration);
}

#[test]
fn invalid_configuration_keeps_the_existing_passive_policies_before_activation() {
    let (state, _state_dir) = test_state();
    let (show, store) = TempShow::create("invalid", "Invalid Configuration");
    let invalid_attributes = serde_json::json!({"version": 999, "future": {"keep": true}});
    store
        .put_object("attribute_configuration", "default", &invalid_attributes, 0)
        .unwrap();
    let undecodable_psn = serde_json::json!({"enabled": "yes", "future": 1});
    store
        .put_object(PSN_KIND, PSN_ID, &undecodable_psn, 0)
        .unwrap();
    let source = store.portable_document().unwrap();
    drop(store);
    let live_before = live_fingerprint(&state);

    let activation = prepare_activation(&show).unwrap();

    assert_eq!(
        live_fingerprint(&state),
        live_before,
        "preparation touches no live state"
    );
    assert_bundle_is_coherent(&activation);
    let prepared = activation.configuration();
    // Same passive result as the existing document installer.
    let legacy = InstalledAttributeConfiguration::for_document(activation.document());
    assert_eq!(prepared.attributes().configuration, legacy.configuration);
    assert_eq!(
        prepared.attributes().validation_error,
        legacy.validation_error
    );
    assert!(
        prepared
            .attributes()
            .validation_error
            .as_deref()
            .is_some_and(|error| error.contains("recommended defaults are active"))
    );
    prepared.attributes().configuration.validate().unwrap();
    assert!(matches!(
        prepared.psn().origin,
        PsnConfigurationOrigin::Undecodable(_)
    ));
    assert_eq!(
        prepared.psn().configuration,
        light_application::PsnConfiguration::default()
    );
    // The existing lossless compatibility migration still runs: it adds the canonical empty
    // arrays but preserves the unsupported version and unknown fields. PSN is never migrated.
    let stored = show.fresh_document();
    assert_eq!(activation.document(), &stored);
    assert!(stored.revision() > source.revision());
    let attributes = stored.object("attribute_configuration", "default").unwrap();
    assert_eq!(attributes.body()["version"], 999);
    assert_eq!(attributes.body()["future"], invalid_attributes["future"]);
    assert_eq!(prepared.attributes().object_revision, attributes.revision());
    assert_eq!(
        stored.object(PSN_KIND, PSN_ID).unwrap().body(),
        &undecodable_psn
    );
    assert_eq!(
        stored.object(PSN_KIND, PSN_ID).unwrap().revision(),
        source.object(PSN_KIND, PSN_ID).unwrap().revision()
    );
}

#[test]
fn decodable_psn_is_installed_as_stored_without_load_time_validation() {
    let (show, store) = TempShow::create("psn-unvalidated", "Unvalidated Tracking");
    let mut body = stored_psn(0);
    body["staleAfterMillis"] = serde_json::json!(1);
    store.put_object(PSN_KIND, PSN_ID, &body, 0).unwrap();
    drop(store);

    let activation = prepare_activation(&show).unwrap();

    let prepared = activation.configuration().psn();
    assert_eq!(prepared.origin, PsnConfigurationOrigin::Stored);
    assert_eq!(prepared.configuration.port, 0);
    assert!(
        prepared.configuration.validate().is_err(),
        "loading keeps the existing edit-only validation policy"
    );
    let absent = TempShow::create("psn-absent", "No Tracking");
    drop(absent.1);
    let absent_activation = prepare_activation(&absent.0).unwrap();
    assert_eq!(
        absent_activation.configuration().psn().origin,
        PsnConfigurationOrigin::Absent
    );
    assert_eq!(absent_activation.configuration().psn().object_revision, 0);
}

#[test]
fn failed_migration_preparation_changes_no_installed_state_or_portable_content() {
    let (state, _state_dir) = test_state();
    let (show, store) = TempShow::legacy("damaged", "Damaged Legacy Show");
    let object = store.objects("patched_fixture").unwrap().remove(0);
    let mut damaged = object.body;
    damaged["fixture_id"] = serde_json::json!("not-a-uuid");
    store
        .put_object("patched_fixture", &object.id, &damaged, object.revision)
        .unwrap();
    store
        .put_object(PSN_KIND, PSN_ID, &stored_psn(7003), 0)
        .unwrap();
    let source = store.portable_document().unwrap();
    drop(store);
    let live_before = live_fingerprint(&state);

    let error = prepare_activation(&show)
        .err()
        .expect("a damaged legacy show must fail preparation");

    assert!(matches!(
        error,
        ShowLoadError::Application(_) | ShowLoadError::Store(_)
    ));
    assert_eq!(live_fingerprint(&state), live_before);
    assert_eq!(show.fresh_document(), source);
    assert_eq!(show.backups(), 0);
}

#[test]
fn prepared_psn_installation_resets_ownership_even_for_equal_configuration() {
    let (state, _state_dir) = test_state();
    let (first, first_store) = TempShow::create("psn-first", "First");
    let (second, second_store) = TempShow::create("psn-second", "Second");
    let body = stored_psn(7004);
    first_store.put_object(PSN_KIND, PSN_ID, &body, 0).unwrap();
    second_store.put_object(PSN_KIND, PSN_ID, &body, 0).unwrap();
    drop((first_store, second_store));
    let first = prepare_activation(&first).unwrap();
    let second = prepare_activation(&second).unwrap();
    assert_eq!(
        first.configuration().psn().configuration,
        second.configuration().psn().configuration,
        "both shows hold an equal tracking configuration"
    );

    psn_http::install_prepared(&state, first.configuration().psn());
    let installed = state.psn.generation();
    assert_eq!(
        state.psn.committed_tracking_frame(0).show_id,
        Some(first.document().id())
    );
    // Control: the same-show document installer skips an equal configuration.
    psn_http::install_document(&state, first.document());
    assert_eq!(state.psn.generation(), installed);

    psn_http::install_prepared(&state, first.configuration().psn());
    let same_show = state.psn.generation();
    assert_ne!(
        same_show, installed,
        "cold activation resets the same show's owner"
    );

    psn_http::install_prepared(&state, second.configuration().psn());
    assert_ne!(
        state.psn.generation(),
        same_show,
        "a different show never reuses ownership"
    );
    assert_eq!(
        state.psn.committed_tracking_frame(0).show_id,
        Some(second.document().id())
    );
    assert_eq!(
        state.psn.configuration(),
        second.configuration().psn().configuration
    );
}

#[test]
fn runtime_activation_helper_prepares_output_and_configuration_without_installing() {
    let (state, _state_dir) = test_state();
    let (show, store) = TempShow::create("helper", "Helper Tracking");
    store
        .put_object(PSN_KIND, PSN_ID, &stored_psn(7005), 0)
        .unwrap();
    drop(store);
    let live_before = live_fingerprint(&state);

    let activation = prepare_show_activation_for_runtime(&state, &show.entry).unwrap();

    assert_eq!(live_fingerprint(&state), live_before);
    assert_bundle_is_coherent(&activation);
    assert_eq!(activation.configuration().psn().configuration.port, 7005);
    let (_runtime, document, configuration) = activation.into_parts();
    assert_eq!(document.id(), show.entry.id);
    assert_eq!(configuration.show_id(), show.entry.id);
}
