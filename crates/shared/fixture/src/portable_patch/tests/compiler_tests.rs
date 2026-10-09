use super::support::{fixture, profile, source};
use crate::{
    ChannelFunction, FixtureProfileRevisionResolver, PatchPolicy, PatchedFixtureCompiler,
    PatchedFixturePatch, PatchedFixtureProfileReference, PortablePatchError,
    PortablePatchedFixtureRecord, ResolvedFixtureProfileRevision, fixture_profile_content_digest,
};
use light_core::{AttributeKey, FixtureId};
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
fn embedded_jbled_a7_revision_one_uses_safe_open_shutter_runtime_compatibility() {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library/jb-lighting--jbled-a7.toskfixture");
    let mut profile = crate::read_fixture_package(&std::fs::read(package).unwrap()).unwrap();
    profile.revision = 1;
    let mode = profile
        .modes
        .iter_mut()
        .find(|mode| mode.name == "Standard RGB 16 Bit (S16)")
        .unwrap();
    let shutter = mode
        .channels
        .iter_mut()
        .find(|channel| *channel.attribute.0 == *"shutter")
        .unwrap();
    shutter.default_raw = 0;
    shutter.functions = vec![ChannelFunction::continuous(
        "Shutter / Strobe",
        AttributeKey("shutter".into()),
        255,
    )];
    let mode_id = mode.id;
    let fixture = fixture(&profile);
    let reference = PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id,
    };
    let record = PortablePatchedFixtureRecord::from_profile_reference(
        reference,
        PatchedFixturePatch::from_fixture(&fixture),
    )
    .unwrap();
    let compiled = PatchedFixtureCompiler::new(CountingResolver {
        source: Some(source(&profile)),
        calls: 0,
    })
    .compile(&record)
    .unwrap();

    let snapshot = compiled.definition.profile_snapshot.unwrap();
    let shutter = snapshot.modes[0]
        .channels
        .iter()
        .find(|channel| *channel.attribute.0 == *"shutter")
        .unwrap();
    assert_eq!(shutter.default_raw, 16);
    assert_eq!(shutter.functions.len(), 23);
    assert_eq!(shutter.functions[0].name, "Shutter closed");
    assert_eq!(shutter.functions[1].name, "Shutter open");
    assert_eq!(
        (shutter.functions[1].dmx_from, shutter.functions[1].dmx_to),
        (16, 95)
    );
}

#[test]
fn a_referenced_generic_led_compiles_with_the_derived_uncalibrated_color_model() {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("assets/fixture-library/generic--rgb-led.toskfixture");
    let profile = crate::read_fixture_package(&std::fs::read(package).unwrap()).unwrap();
    let mode_id = profile
        .modes
        .iter()
        .find(|mode| mode.name == "DRGB 8-bit dimmer first")
        .unwrap()
        .id;
    assert!(
        profile
            .modes
            .iter()
            .all(|mode| mode.color_physical.is_none())
    );
    let digest = fixture_profile_content_digest(&json!(profile)).unwrap();
    let source = source(&profile);
    let reference = PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: profile.revision.into(),
        mode_id,
    };
    let record = PortablePatchedFixtureRecord::from_profile_reference(
        reference,
        PatchedFixturePatch::from_fixture(&fixture(&profile)),
    )
    .unwrap();
    let compiled = PatchedFixtureCompiler::new(CountingResolver {
        source: Some(source),
        calls: 0,
    })
    .compile(&record)
    .unwrap();

    let snapshot = compiled.definition.profile_snapshot.as_deref().unwrap();
    let model = snapshot.modes[0]
        .color_physical
        .as_ref()
        .expect("the runtime projection derives a Color model");
    let crate::OpticalSource::Additive { emitters } = &model.paths[0].source else {
        panic!("additive emitters")
    };
    assert_eq!(emitters.len(), 3);
    assert!(
        emitters
            .iter()
            .all(|e| e.provenance.quality == crate::PhysicalDataQuality::Unknown)
    );
    // The derived model is the mode's native Color source (G7a): its context names the same
    // identities the stored profile derives, so Direct capture and replay work on it. The stored
    // revision is untouched.
    let context = compiled
        .definition
        .runtime_color_context
        .as_deref()
        .expect("a derived Color model has a native Color context");
    assert_eq!(
        context.identities(),
        profile.native_color_identities(mode_id).unwrap()
    );
    assert!(context.mode().color_physical.is_some());
    assert_eq!(
        fixture_profile_content_digest(&json!(profile)).unwrap(),
        digest
    );
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

/// Exercise the same retained legacy optical model that the runtime importer projects.
fn assert_projected_color_context_keeps_original_recipe(profile: crate::FixtureProfile) {
    use light_core::NativeColorValue;
    use light_core::programming::{
        ColorProgram, DirectDestination, DirectReplay, NativeColorEditModel, NativeColorRecipe,
        plan_direct_replay,
    };
    let mode = &profile.modes[0];
    let mode_id = mode.id;
    let original = crate::CompiledNativeColorEditModel::compile_mode(&profile, mode_id)
        .unwrap()
        .remove(0)
        .1
        .unwrap();
    let original_identity = original.source().clone();
    let mut placed = fixture(&profile);
    placed.logical_heads[0].head_index = 0;
    placed.logical_heads[0].profile_head_id = Some(mode.heads[0].id);
    let record = PortablePatchedFixtureRecord::from_profile_reference(
        PatchedFixtureProfileReference {
            profile_id: profile.id,
            profile_revision: profile.revision.into(),
            mode_id,
        },
        PatchedFixturePatch::from_fixture(&placed),
    )
    .unwrap();
    let compiled = PatchedFixtureCompiler::new(CountingResolver {
        source: Some(source(&profile)),
        calls: 0,
    })
    .compile(&record)
    .unwrap();
    let runtime = compiled.definition.profile_snapshot.as_ref().unwrap();
    let context = compiled.definition.runtime_color_context.as_ref().unwrap();
    assert_eq!(
        context.identities(),
        std::slice::from_ref(&original_identity)
    );
    assert_ne!(
        serde_json::to_value(&mode.color_physical).unwrap(),
        serde_json::to_value(&runtime.mode(mode_id).unwrap().color_physical).unwrap(),
        "fixture must exercise a real compatibility projection"
    );
    context.validate_runtime_profile(runtime, mode_id).unwrap();
    let forward = crate::forward::CompiledColorForward::compile_with_context(
        runtime,
        mode_id,
        None,
        Some(context),
    )
    .unwrap()
    .unwrap();
    assert!(crate::forward::CompiledColorFitting::from_forward(runtime, mode_id, forward).is_ok());
    let destination = crate::CompiledNativeColorEditModel::compile_mode(runtime, mode_id)
        .unwrap()
        .remove(0)
        .1
        .unwrap();
    for (family, raw) in [("color.red", 32), ("color.blue", 64)] {
        let path = &mode.color_physical.as_ref().unwrap().paths[0];
        if !mode
            .channels
            .iter()
            .any(|channel| &*channel.attribute.0 == family)
        {
            continue;
        }
        let channels = path
            .controls
            .iter()
            .map(|id| {
                let channel = mode
                    .channels
                    .iter()
                    .find(|channel| channel.id == *id)
                    .unwrap();
                let function = &channel.functions[0];
                NativeColorValue {
                    channel_id: channel.id,
                    function_id: function.id,
                    raw: if &*channel.attribute.0 == family {
                        raw
                    } else {
                        0
                    },
                }
            })
            .collect();
        let recipe = NativeColorRecipe {
            source: original_identity.clone(),
            channels,
            spreads: vec![],
        };
        let portable = original.predict(&recipe).unwrap();
        let program = ColorProgram::Direct {
            recipe: recipe.clone(),
            portable,
        };
        assert!(
            matches!(plan_direct_replay(&program, &DirectDestination::Verified(&destination)).unwrap(),
            DirectReplay::Exact { recipe: exact } if exact == recipe)
        );
    }
    let mut forged = runtime.as_ref().clone();
    forged
        .modes
        .iter_mut()
        .find(|mode| mode.id == mode_id)
        .unwrap()
        .color_physical
        .as_mut()
        .unwrap()
        .revision += 1;
    assert!(
        context.validate_runtime_profile(&forged, mode_id).is_err(),
        "generic context validation must continue rejecting an unapproved optical model"
    );
    let mut forged_physical = runtime.as_ref().clone();
    forged_physical
        .modes
        .iter_mut()
        .find(|mode| mode.id == mode_id)
        .unwrap()
        .channels[0]
        .physical_min = Some(0.5);
    assert_eq!(
        forged_physical.native_color_identities(mode_id).unwrap()[0].native_layout_signature,
        runtime.native_color_identities(mode_id).unwrap()[0].native_layout_signature,
        "physical fields are deliberately excluded from the compact native layout signature"
    );
    let mut physical_rebound = context.as_ref().clone();
    let before_physical_mode = serde_json::to_value(physical_rebound.mode()).unwrap();
    assert!(
        physical_rebound
            .rebind_verified_runtime_projection(&forged_physical, mode_id)
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(physical_rebound.mode()).unwrap(),
        before_physical_mode
    );
    let mut forged_layout = runtime.as_ref().clone();
    let channel = &mut forged_layout
        .modes
        .iter_mut()
        .find(|mode| mode.id == mode_id)
        .unwrap()
        .channels[0];
    channel.default_raw = 1;
    let mut rebound = context.as_ref().clone();
    let before_mode = serde_json::to_value(rebound.mode()).unwrap();
    assert!(
        rebound
            .rebind_verified_runtime_projection(&forged_layout, mode_id)
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(rebound.mode()).unwrap(),
        before_mode,
        "rejected native layout must not partially replace the context"
    );
}

#[test]
fn projected_legacy_gdtf_color_context_preserves_exact_native_recipe() {
    use std::io::Write;
    let xml = r#"<GDTF DataVersion="1.2"><FixtureType Name="Optical context" Manufacturer="Contract" FixtureTypeID="684af0b8-5e84-4e28-a8a2-687647b2b515"><AttributeDefinitions><Attributes><Attribute Name="ColorAdd_R"/><Attribute Name="ColorAdd_B"/></Attributes></AttributeDefinitions><PhysicalDescriptions><Emitters><Emitter Name="Red" Color="0.64,0.33,21.26729"/><Emitter Name="Blue" Color="0.15,0.06,7.2175"/></Emitters></PhysicalDescriptions><DMXModes><DMXMode Name="RB"><DMXChannels><DMXChannel Offset="1" Geometry="Head"><LogicalChannel Attribute="ColorAdd_R"><ChannelFunction Name="Red" Attribute="ColorAdd_R" DMXFrom="0/1" PhysicalFrom="0" PhysicalTo="1" Emitter="Red"/></LogicalChannel></DMXChannel><DMXChannel Offset="2" Geometry="Head"><LogicalChannel Attribute="ColorAdd_B"><ChannelFunction Name="Blue" Attribute="ColorAdd_B" DMXFrom="0/1" PhysicalFrom="0" PhysicalTo="1" Emitter="Blue"/></LogicalChannel></DMXChannel></DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"#;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("description.xml", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(xml.as_bytes()).unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let mut profile = crate::gdtf::read::import_legacy_optical_profile(&bytes).unwrap();
    profile.source_gdtf = Some(crate::ProfileGdtfSource::associate(&profile, &bytes).unwrap());
    assert_projected_color_context_keeps_original_recipe(profile);
}

#[test]
#[ignore = "explicit local retained manufacturer acceptance profile; not shipped fixture data"]
fn projected_retained_brighter_color_context_preserves_exact_native_recipe() {
    let path = std::env::var("LIGHT_FIXTURE_ACCEPTANCE_PROFILE").unwrap();
    let profile = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_projected_color_context_keeps_original_recipe(profile);
}
