use super::support::{fixture, profile, source};
use crate::{
    FixtureProfileRevisionResolver, PatchPolicy, PatchedFixtureCompiler, PatchedFixturePatch,
    PatchedFixtureProfileReference, PortablePatchError, PortablePatchedFixtureRecord,
    ResolvedFixtureProfileRevision, fixture_profile_content_digest,
};
use light_core::FixtureId;
use serde_json::json;
use std::{cell::Cell, cell::RefCell, rc::Rc};
use uuid::Uuid;

#[derive(Clone)]
struct CountingResolver {
    source: Option<ResolvedFixtureProfileRevision>,
    calls: usize,
}

impl FixtureProfileRevisionResolver for CountingResolver {
    fn resolve(
        &mut self,
        _reference: PatchedFixtureProfileReference,
    ) -> Option<ResolvedFixtureProfileRevision> {
        self.calls += 1;
        self.source.clone()
    }
}

#[test]
fn one_profile_revision_is_resolved_once_for_many_fixture_records() {
    let profile = profile();
    let first = fixture(&profile);
    let mut second = fixture(&profile);
    second.fixture_number = Some(43);
    let records = [
        PortablePatchedFixtureRecord::from_runtime_fixture(&first).unwrap(),
        PortablePatchedFixtureRecord::from_runtime_fixture(&second).unwrap(),
    ];
    let resolver = CountingResolver {
        source: Some(source(&profile)),
        calls: 0,
    };
    let mut compiler = PatchedFixtureCompiler::new(resolver);
    let compiled = compiler.compile_all(&records).unwrap();

    assert_eq!(compiled.len(), 2);
    assert_eq!(compiler.cached_profile_count(), 1);
    assert_eq!(compiler.into_resolver().calls, 1);
    assert_eq!(json!(compiled[0]), json!(first));
    assert_eq!(json!(compiled[1]), json!(second));
}

#[test]
fn large_profile_batch_resolves_once_and_keeps_only_the_selected_runtime_mode() {
    let profile = profile_with_modes(2_000);
    let selected_mode = profile.modes[1_337].id;
    let template = fixture(&profile);
    let template_patch = PatchedFixturePatch::from_fixture(&template);
    let reference = PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id: selected_mode,
    };
    let records = (0..100)
        .map(|number| {
            let mut patch = template_patch.clone();
            patch.fixture_id = FixtureId::new();
            patch.fixture_number = Some(number + 1);
            PortablePatchedFixtureRecord::from_profile_reference(reference, patch).unwrap()
        })
        .collect::<Vec<_>>();
    let resolver = CountingResolver {
        source: Some(source(&profile)),
        calls: 0,
    };
    let mut compiler = PatchedFixtureCompiler::new(resolver);

    let compiled = compiler.compile_all(&records).unwrap();

    assert_eq!(compiled.len(), 100);
    assert_eq!(compiler.cached_profile_count(), 1);
    assert_eq!(compiler.into_resolver().calls, 1);
    assert!(compiled.iter().all(|fixture| {
        fixture
            .definition
            .profile_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.modes.len() == 1 && snapshot.modes[0].id == selected_mode
            })
    }));
}

#[test]
fn failed_candidate_compile_discards_cached_profile_content() {
    let profile = profile();
    let fixture = fixture(&profile);
    let valid = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let mut missing_mode_body = valid.body().clone();
    missing_mode_body["mode_id"] = json!(Uuid::new_v4());
    let missing_mode = PortablePatchedFixtureRecord::decode(missing_mode_body).unwrap();
    let source = Rc::new(RefCell::new(Some(source(&profile))));
    let calls = Rc::new(Cell::new(0));
    let mut compiler = PatchedFixtureCompiler::new({
        let source = Rc::clone(&source);
        let calls = Rc::clone(&calls);
        move |_| {
            calls.set(calls.get() + 1);
            source.borrow().clone()
        }
    });

    assert!(matches!(
        compiler.compile(&missing_mode),
        Err(PortablePatchError::MissingMode { .. })
    ));
    *source.borrow_mut() = Some(ResolvedFixtureProfileRevision::new(
        profile.id,
        profile.revision.into(),
        "sha256:changed-after-failure",
        serde_json::to_value(&profile).unwrap(),
    ));
    assert!(matches!(
        compiler.compile(&valid),
        Err(PortablePatchError::ProfileDigestMismatch { .. })
    ));
    assert_eq!(calls.get(), 2);
}

#[test]
fn legacy_inline_record_verifies_the_canonical_revision_and_is_equivalent() {
    let profile = profile();
    let fixture = fixture(&profile);
    let record = PortablePatchedFixtureRecord::decode(json!(fixture)).unwrap();
    let resolver = CountingResolver {
        source: Some(source(&profile)),
        calls: 0,
    };
    let mut compiler = PatchedFixtureCompiler::new(resolver);
    let compiled = compiler.compile(&record).unwrap();

    assert_eq!(json!(compiled), json!(fixture));
    assert_eq!(compiler.into_resolver().calls, 1);
}

#[test]
fn legacy_inline_record_rejects_content_that_conflicts_with_canonical_revision() {
    let profile = profile();
    let fixture = fixture(&profile);
    let mut body = json!(fixture);
    body["definition"]["profile_snapshot"]["name"] = json!("Tampered inline copy");
    let record = PortablePatchedFixtureRecord::decode(body).unwrap();
    let error = compile_error(&record, Some(source(&profile)));

    assert!(matches!(
        error,
        PortablePatchError::ProfileDigestMismatch { .. }
    ));
}

#[test]
fn schema_one_legacy_record_remains_loadable_without_a_profile_revision() {
    let profile = profile();
    let mut fixture = fixture(&profile);
    fixture.definition.schema_version = 1;
    fixture.definition.profile_id = None;
    fixture.definition.mode_id = None;
    fixture.definition.profile_snapshot = None;
    fixture.definition.validate().unwrap();
    let record = PortablePatchedFixtureRecord::decode(json!(fixture)).unwrap();
    let mut compiler = PatchedFixtureCompiler::new(CountingResolver {
        source: None,
        calls: 0,
    });

    let compiled = compiler.compile(&record).unwrap();

    assert_eq!(json!(compiled), json!(fixture));
    assert_eq!(compiler.into_resolver().calls, 0);
}

#[test]
fn compiler_reports_missing_and_mismatched_profile_revisions_and_modes() {
    let profile = profile();
    let fixture = fixture(&profile);
    let record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let missing = compile_error(&record, None);
    assert!(matches!(
        missing,
        PortablePatchError::MissingProfileRevision { .. }
    ));

    let wrong_revision = ResolvedFixtureProfileRevision::new(
        profile.id,
        u64::from(profile.revision + 1),
        "sha256:unused",
        serde_json::to_value(&profile).unwrap(),
    );
    let mismatch = compile_error(&record, Some(wrong_revision));
    assert!(matches!(
        mismatch,
        PortablePatchError::ProfileIdentityMismatch { .. }
    ));

    let mut mismatched_content = serde_json::to_value(&profile).unwrap();
    mismatched_content["revision"] = json!(profile.revision + 1);
    let mismatched_digest = fixture_profile_content_digest(&mismatched_content).unwrap();
    let mismatch = compile_error(
        &record,
        Some(ResolvedFixtureProfileRevision::new(
            profile.id,
            profile.revision.into(),
            mismatched_digest,
            mismatched_content,
        )),
    );
    assert!(matches!(
        mismatch,
        PortablePatchError::ProfileIdentityMismatch { .. }
    ));

    let mut missing_mode_body = record.body().clone();
    missing_mode_body["mode_id"] = json!(Uuid::new_v4());
    let missing_mode = PortablePatchedFixtureRecord::decode(missing_mode_body).unwrap();
    let error = compile_error(&missing_mode, Some(source(&profile)));
    assert!(matches!(error, PortablePatchError::MissingMode { .. }));

    // A content digest is verified independently of both the reference and typed profile identity.
    let invalid_digest = ResolvedFixtureProfileRevision::new(
        profile.id,
        profile.revision.into(),
        "sha256:tampered",
        serde_json::to_value(&profile).unwrap(),
    );
    let error = compile_error(&record, Some(invalid_digest));
    assert!(matches!(
        error,
        PortablePatchError::ProfileDigestMismatch { .. }
    ));
}

#[test]
fn compiler_preserves_unpatched_and_virtual_fixture_identity() {
    let physical_profile = profile();
    let mut unpatched = fixture(&physical_profile);
    unpatched.universe = None;
    unpatched.address = None;
    unpatched.split_patches[0].universe = None;
    unpatched.split_patches[0].address = None;
    let unpatched_record = PortablePatchedFixtureRecord::from_runtime_fixture(&unpatched).unwrap();
    let mut compiler = PatchedFixtureCompiler::new(CountingResolver {
        source: Some(source(&physical_profile)),
        calls: 0,
    });
    let compiled = compiler.compile(&unpatched_record).unwrap();
    assert_eq!(compiled.fixture_id, unpatched.fixture_id);
    assert_eq!(compiled.fixture_number, unpatched.fixture_number);
    assert_eq!(compiled.universe, None);
    assert_eq!(compiled.address, None);

    let mut visual_profile = profile();
    visual_profile.id = FixtureId::new();
    visual_profile.patch_policy = PatchPolicy::VisualOnly;
    visual_profile.modes[0].splits[0].footprint = 0;
    visual_profile.modes[0].channels.clear();
    let mut virtual_fixture = fixture(&visual_profile);
    virtual_fixture.fixture_number = None;
    virtual_fixture.virtual_fixture_number = Some(7);
    virtual_fixture.universe = None;
    virtual_fixture.address = None;
    virtual_fixture.split_patches[0].universe = None;
    virtual_fixture.split_patches[0].address = None;
    virtual_fixture.highlight_overrides.clear();
    let virtual_record =
        PortablePatchedFixtureRecord::from_runtime_fixture(&virtual_fixture).unwrap();
    let mut compiler = PatchedFixtureCompiler::new(CountingResolver {
        source: Some(source(&visual_profile)),
        calls: 0,
    });
    let compiled = compiler.compile(&virtual_record).unwrap();
    assert_eq!(compiled.fixture_id, virtual_fixture.fixture_id);
    assert_eq!(compiled.fixture_number, None);
    assert_eq!(compiled.virtual_fixture_number, Some(7));
    assert!(!compiled.definition.is_dmx_patchable());
}

fn compile_error(
    record: &PortablePatchedFixtureRecord,
    source: Option<ResolvedFixtureProfileRevision>,
) -> PortablePatchError {
    PatchedFixtureCompiler::new(CountingResolver { source, calls: 0 })
        .compile(record)
        .unwrap_err()
}

fn profile_with_modes(count: usize) -> crate::FixtureProfile {
    let mut profile = profile();
    let template = profile.modes[0].clone();
    profile.modes = (0..count)
        .map(|index| {
            let mut mode = template.clone();
            mode.id = Uuid::from_u128(index as u128 + 1);
            mode.name = format!("Mode {index}");
            mode
        })
        .collect();
    profile
}
