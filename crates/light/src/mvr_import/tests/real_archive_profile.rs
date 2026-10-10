//! Opt-in real archive profiling. No runtime installation or portable transaction is committed.
//! Source tools/artifact-paths.sh and call light_init_artifact_paths before running:
//! LIGHT_MVR_PROFILE_INPUT=/absolute/rig.mvr LIGHT_MVR_PROFILE_REPORT=/canonical/.artifacts/report.json
//! cargo test -p light-application --lib profile_real_native_mvr_archive -- --ignored --nocapture
//! Optional LIGHT_MVR_PROFILE_LIBRARY_JSON is an array of actual immutable profile documents.
//! Optional LIGHT_MVR_PROFILE_LIBRARY_DB is an existing consistent SQLite COPY under LIGHT_TMP_DIR.
//! Opening that copy may migrate it; never pass a live installation database.
use super::super::*;
use crate::{ActionContext, ActionSource};
use light_fixture::{FixtureDefinition, FixtureLibrary, FixtureProfile};
use light_show::ShowStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, time::Instant};
use uuid::Uuid;

fn required_path(name: &str) -> PathBuf {
    let path =
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")));
    assert!(
        path.is_absolute(),
        "{name} must be an explicit absolute path"
    );
    path
}

fn measure<T>(phases: &mut Vec<Value>, name: &str, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let result = work();
    let duration = started.elapsed().as_secs_f64();
    println!("MVR_PROFILE {name}: {duration:.6}s");
    phases.push(json!({"phase":name,"seconds":duration}));
    result
}

fn library_slots(phases: &mut Vec<Value>) -> (MvrProfileSlots, bool) {
    let Some(path) = std::env::var_os("LIGHT_MVR_PROFILE_LIBRARY_JSON") else {
        return (MvrProfileSlots::new(), false);
    };
    let slots = measure(phases, "actual_library_profile_slots", || {
        let values: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut slots = MvrProfileSlots::new();
        for value in values {
            let profile: FixtureProfile = serde_json::from_value(value.clone()).unwrap();
            slots
                .entry((profile.id.0, u64::from(profile.revision)))
                .or_default()
                .insert(mvr_profile_identity(value).unwrap());
        }
        slots
    });
    (slots, true)
}

struct ActualLibraryCatalog {
    library: Option<FixtureLibrary>,
    profiles: Vec<FixtureProfile>,
    definitions: Vec<FixtureDefinition>,
    path: Option<PathBuf>,
}

fn actual_library_catalog(phases: &mut Vec<Value>) -> ActualLibraryCatalog {
    let Some(_) = std::env::var_os("LIGHT_MVR_PROFILE_LIBRARY_DB") else {
        return ActualLibraryCatalog {
            library: None,
            profiles: Vec::new(),
            definitions: Vec::new(),
            path: None,
        };
    };
    let path = required_path("LIGHT_MVR_PROFILE_LIBRARY_DB")
        .canonicalize()
        .unwrap();
    let temporary = required_path("LIGHT_TMP_DIR").canonicalize().unwrap();
    assert!(
        path.is_file() && path.starts_with(&temporary),
        "library must be an existing SQLite COPY inside canonical LIGHT_TMP_DIR (symlinks resolved)"
    );
    let library = measure(phases, "copied_library_open_diagnostic_startup", || {
        FixtureLibrary::open(&path).unwrap()
    });
    let profiles = measure(phases, "actual_catalog_profiles_sql_decode", || {
        library.profiles().unwrap()
    });
    let definitions = measure(
        phases,
        "actual_catalog_legacy_definitions_sql_decode",
        || library.definitions().unwrap(),
    );
    ActualLibraryCatalog {
        library: Some(library),
        profiles,
        definitions,
        path: Some(path),
    }
}

fn bind_actual_catalog(
    phases: &mut Vec<Value>,
    archive: &light_mvr::MvrDocument,
    catalog: &ActualLibraryCatalog,
) -> MvrDefinitions {
    measure(phases, "canonical_source_binding", || {
        bind_mvr_sources(
            archive,
            &catalog.profiles,
            &catalog.definitions,
            |id, revision| {
                let library = catalog
                    .library
                    .as_ref()
                    .expect("native-only diagnostic must not fabricate installed revisions");
                library.profile(id, revision).map_err(|error| {
                    crate::ActionError::new(crate::ActionErrorKind::Invalid, error.to_string())
                })
            },
            |_| panic!("native-only diagnostic must not fabricate canonical attribute mappings"),
        )
        .unwrap()
    })
}

#[test]
#[ignore = "requires explicit real MVR input and canonical artifact destinations"]
fn profile_real_native_mvr_archive() {
    let input = required_path("LIGHT_MVR_PROFILE_INPUT");
    let report = required_path("LIGHT_MVR_PROFILE_REPORT");
    let artifacts = required_path("LIGHT_ARTIFACTS_DIR");
    assert!(
        report.starts_with(&artifacts),
        "report must stay under the canonical artifact root"
    );
    let temporary =
        required_path("LIGHT_TMP_DIR").join(format!("mvr-profile-{}.show", Uuid::new_v4()));
    let mut phases = Vec::new();
    let bytes = measure(&mut phases, "read_input", || std::fs::read(&input).unwrap());
    let input_hash = format!("{:x}", Sha256::digest(&bytes));
    let archive = measure(&mut phases, "mvr_read", || light_mvr::read(&bytes).unwrap());
    let native = measure(&mut phases, "standalone_native_metadata_decode", || {
        crate::mvr_export::tosklight_mvr_fixture_metadata(&archive)
    });
    assert!(
        !archive.fixtures.is_empty(),
        "empty archives do not exercise fixture work"
    );
    assert!(
        archive
            .fixtures
            .iter()
            .all(|fixture| native.contains_key(&fixture.uuid)),
        "this diagnostic requires actual native snapshots for every fixture; external descriptor mapping is a separate scope"
    );
    let profile_keys = native
        .values()
        .filter_map(|entry| entry.fixture.definition.profile_snapshot.as_ref())
        .map(|profile| (profile.id, profile.revision))
        .collect::<std::collections::HashSet<_>>();
    let native_count = native.len();
    drop(native);
    let catalog = actual_library_catalog(&mut phases);
    let mut bindings = bind_actual_catalog(&mut phases, &archive, &catalog);
    let (mut slots, actual_library_supplied) = library_slots(&mut phases);
    let collisions = measure(&mut phases, "native_profile_collision_review", || {
        mvr_profile_conflicts(&bindings, &slots, &HashMap::new()).unwrap()
    });
    let collision_count = collisions.len();
    // This is an isolated diagnostic allocation, never live operator consent or publication.
    measure(&mut phases, "profile_reservation_and_detached_copy", || {
        reserve_mvr_profiles_with_identity_copies(&mut bindings, &mut slots, &HashMap::new(), true)
            .unwrap()
    });
    let (store, _) =
        ShowStore::create(&temporary, "Detached real MVR profiling destination").unwrap();
    let destination = store.portable_document().unwrap();
    let context = ActionContext::system(Uuid::new_v4(), ActionSource::Http);
    let planned = measure(&mut phases, "canonical_new_show_plan", || {
        plan_mvr_document_import(
            &destination,
            context,
            &archive,
            &bindings.definitions,
            &HashMap::new(),
        )
        .unwrap()
    });
    assert_eq!(
        store.portable_document().unwrap().revision(),
        destination.revision()
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&input).unwrap())),
        input_hash
    );
    let output = json!({
        "input":input,"sha256":input_hash,"archive_bytes":bytes.len(),
        "member_bytes":archive.files.values().map(Vec::len).sum::<usize>(),
        "fixtures":archive.fixtures.len(),"native_fixtures":native_count,
        "unique_native_profile_revisions":profile_keys.len(),"bound_fixtures":bindings.definitions.len(),
        "collision_profiles":collision_count,"library_slots_supplied":actual_library_supplied,
        "library_db_copy":catalog.path,"catalog_profiles":catalog.profiles.len(),
        "catalog_legacy_definitions":catalog.definitions.len(),
        "catalog_legacy_embedded_snapshots":catalog.definitions.iter().filter(|definition| definition.profile_snapshot.is_some()).count(),
        "collision_scope":if actual_library_supplied {"actual supplied immutable library documents"} else {"intra-archive only; installed collisions not measured"},
        "planned_fixtures":planned.imported_fixtures,"unresolved_fixtures":planned.unresolved_fixtures,
        "warnings":planned.warnings.len(),"phases":phases,
        "limits":"Detached plan only; no SQLite transaction commit, candidate runtime compilation, activation, event publication, network output or UI timing. Copied-library open and standalone metadata measurements are diagnostic startup/overhead, not extra production phases. Catalog reads mirror the actual MVR runtime path. Only the explicitly supplied temporary database copy may be migrated; no live database is opened."
    });
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    std::fs::write(&report, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
    println!("MVR_PROFILE_REPORT {}", report.display());
    drop(store);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", temporary.display()));
    }
}

/// Bounds the real catalog expansion to two modes, never all modes/all installed profiles.
#[test]
#[ignore = "requires one real immutable profile JSON and canonical artifact destinations"]
fn profile_real_catalog_first_and_last_mode() {
    let input = required_path("LIGHT_MVR_CATALOG_PROFILE_INPUT");
    let report = required_path("LIGHT_MVR_PROFILE_REPORT");
    assert!(report.starts_with(required_path("LIGHT_ARTIFACTS_DIR")));
    let mut phases = Vec::new();
    let bytes = std::fs::read(&input).unwrap();
    let input_hash = format!("{:x}", Sha256::digest(&bytes));
    let mut profile: FixtureProfile = measure(&mut phases, "actual_profile_decode", || {
        serde_json::from_slice(&bytes).unwrap()
    });
    assert!(!profile.modes.is_empty(), "profile must contain real modes");
    let mode_count = profile.modes.len();
    let channel_count: usize = profile.modes.iter().map(|mode| mode.channels.len()).sum();
    // Match patchable_definitions precisely: catalog omits source archives, never modes.
    profile.source_gdtf = None;
    let first = profile.modes[0].id;
    let last = profile.modes[mode_count - 1].id;
    measure(&mut phases, "standalone_full_profile_validation", || {
        profile.validate().unwrap()
    });
    let first_definition = measure(&mut phases, "catalog_first_mode_projection", || {
        profile.resolved_definition(first).unwrap()
    });
    let last_definition = measure(&mut phases, "catalog_last_mode_projection", || {
        profile.resolved_definition(last).unwrap()
    });
    for definition in [&first_definition, &last_definition] {
        let snapshot = definition.profile_snapshot.as_ref().unwrap();
        assert_eq!(
            serde_json::to_value(&snapshot.modes).unwrap(),
            serde_json::to_value(&profile.modes).unwrap(),
            "full ordered modes must remain present"
        );
        assert_eq!(snapshot.id, profile.id);
        assert_eq!(snapshot.revision, profile.revision);
        assert!(
            snapshot.source_gdtf.is_none(),
            "catalog source policy must remain unchanged"
        );
    }
    assert_eq!(first_definition.mode_id, Some(first));
    assert_eq!(last_definition.mode_id, Some(last));
    let snapshots_share_allocation = std::sync::Arc::ptr_eq(
        first_definition.profile_snapshot.as_ref().unwrap(),
        last_definition.profile_snapshot.as_ref().unwrap(),
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&input).unwrap())),
        input_hash
    );
    let output = json!({
        "input":input,"sha256":input_hash,"profile_json_bytes":bytes.len(),
        "profile_id":profile.id,"revision":profile.revision,"name":profile.name,
        "mode_count":mode_count,"channel_count":channel_count,"projected_modes":2,
        "snapshots_share_allocation":snapshots_share_allocation,"phases":phases,
        "limits":"Exactly one actual profile and first/last mode only. Source archive omitted exactly as production catalog does. Full ordered modes preserved. No whole-library projection, SQL writes, runtime installation or UI measurement. Standalone validation is additional diagnostic overhead, not a production phase."
    });
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    std::fs::write(&report, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
    println!("MVR_PROFILE_REPORT {}", report.display());
}
