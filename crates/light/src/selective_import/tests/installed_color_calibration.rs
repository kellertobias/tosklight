//! TL-608: installed Color calibration is passive observational metadata of each physical
//! instance. Profile Duplicate rebases only a proof that was current against the original pinned
//! profile; stale, foreign, absent-source and Keep observations stay exactly as authored.
use super::support::*;
use crate::selective_import::*;
use light_core::{AttributeKey, NativeColorIdentity, NativeColorValue, PhysicalDataQuality, Xyz};
use light_fixture::{
    ChannelFunction, ColorPhysicalModel, ColorRecipeMeasurement, FixtureProfile, GeometryGraph,
    GeometryTemplate, HeadOpticalPath, InstalledColorCalibration, InstalledColorCalibrationStatus,
    InstalledColorPathCalibration, InstalledEmitterCalibration, NativeColorBinding, OpticalEmitter,
    OpticalEmitterBand, OpticalProvenance, OpticalSource, PatchedFixture,
    forward::{ColorForwardFlags, CompiledColorForward},
};
use light_show::{FixtureProfileRevision, PortableShowDocument};
use serde_json::{Value, json};
use uuid::Uuid;

/// Mutates one raw JSON value of a test case.
type Edit = fn(&mut Value);

const ROOT: &str = "/color_calibration";
const COPY: &str = "/multipatch/0/color_calibration";
/// Full-width native words of the measured complete-path recipe (U32, U8, U16).
const RECIPE: [u32; 3] = [u32::MAX - 1, 200, 65_534];
const EMITTERS: [Xyz; 3] = [
    Xyz {
        x: 0.4124,
        y: 0.2126,
        z: 0.0193,
    },
    Xyz {
        x: 0.3576,
        y: 0.7152,
        z: 0.1192,
    },
    Xyz {
        x: 0.1805,
        y: 0.0722,
        z: 0.9505,
    },
];

struct Calibrated {
    record: PortableFixtureTestRecord,
    profile: FixtureProfile,
    identity: NativeColorIdentity,
}

impl Calibrated {
    fn key(&self) -> ImportProfileKey {
        ImportProfileKey {
            profile_id: self.record.profile.id().profile_id(),
            revision: self.record.profile.id().revision(),
        }
    }

    fn id(&self) -> String {
        self.record.fixture_id.0.to_string()
    }
}

/// An RGB additive path whose three controls have different full widths.
fn calibrated_fixture(base: u128, number: u32) -> Calibrated {
    let mut record = portable_fixture_record(base, number);
    let mut raw_profile = record.profile.profile().clone();
    let mut profile: FixtureProfile = serde_json::from_value(raw_profile.clone()).unwrap();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    let widths = [("u32", vec![2, 3, 4]), ("u8", vec![]), ("u16", vec![7])];
    mode.channels = ["color.red", "color.green", "color.blue"]
        .iter()
        .zip(widths)
        .enumerate()
        .map(|(index, (attribute, (resolution, secondary)))| {
            let max = match resolution {
                "u32" => u32::MAX,
                "u16" => 65_535,
                _ => 255,
            };
            let mut function =
                ChannelFunction::continuous(*attribute, AttributeKey((*attribute).into()), max);
            function.id = Uuid::from_u128(base + 40 + index as u128);
            serde_json::from_value(json!({
                "id": Uuid::from_u128(base + 30 + index as u128), "head_id": head, "split": 1,
                "fixture_attribute": attribute, "attribute": attribute,
                "resolution": resolution, "secondary_slots": secondary,
                "default_raw": 0, "highlight_raw": max, "functions": [function]
            }))
            .unwrap()
        })
        .collect();
    mode.splits[0].footprint = 7;
    let emitters = mode
        .channels
        .iter()
        .zip(EMITTERS)
        .enumerate()
        .map(|(index, (channel, xyz))| OpticalEmitter {
            id: Uuid::from_u128(base + 50 + index as u128),
            name: channel.attribute.0.to_string(),
            binding: NativeColorBinding {
                channel_id: channel.id,
                function_id: channel.functions[0].id,
            },
            xyz: Some(xyz),
            spectrum: vec![],
            band: OpticalEmitterBand::Visible,
            native_reversed: false,
            maximum_level: 1.0,
            response_exponent: 1.0,
            provenance: OpticalProvenance {
                quality: PhysicalDataQuality::Manufacturer,
                source: Some("datasheet".into()),
                revision: 1,
            },
        })
        .collect();
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 3,
        paths: vec![HeadOpticalPath {
            id: Uuid::from_u128(base + 60),
            head_id: head,
            controls: mode.channels.iter().map(|channel| channel.id).collect(),
            source: OpticalSource::Additive { emitters },
            filters: vec![],
            measurements: vec![],
        }],
    });
    profile.validate().unwrap();
    let identity = profile
        .native_color_identity(profile.modes[0].id, head)
        .unwrap();
    let typed = serde_json::to_value(&profile).unwrap();
    // Keep the raw extension field portable_fixture_record stores outside the typed profile.
    raw_profile
        .as_object_mut()
        .unwrap()
        .extend(typed.as_object().unwrap().clone());
    record.profile = FixtureProfileRevision::from_profile(raw_profile).unwrap();
    Calibrated {
        record,
        profile,
        identity,
    }
}

/// Distinct observations per physical instance, including provenance and full-width recipe words.
fn calibration(fixture: &Calibrated, gains: [f32; 3], meter: &str, measured: Xyz) -> Value {
    let mode = &fixture.profile.modes[0];
    let path = &mode.color_physical.as_ref().unwrap().paths[0];
    let OpticalSource::Additive { emitters } = &path.source else {
        unreachable!()
    };
    let provenance = |revision| OpticalProvenance {
        quality: PhysicalDataQuality::Measured,
        source: Some(meter.into()),
        revision,
    };
    let typed = InstalledColorCalibration {
        version: 1,
        revision: 9,
        paths: vec![InstalledColorPathCalibration {
            source_identity: fixture.identity.clone(),
            emitters: emitters
                .iter()
                .zip(gains)
                .map(|(emitter, output_gain)| InstalledEmitterCalibration {
                    emitter_id: emitter.id,
                    output_gain,
                    provenance: provenance(4),
                })
                .collect(),
            measurements: vec![ColorRecipeMeasurement {
                recipe: mode
                    .channels
                    .iter()
                    .zip(RECIPE)
                    .map(|(channel, raw)| NativeColorValue {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                        raw,
                    })
                    .collect(),
                xyz: measured,
                provenance: provenance(5),
            }],
        }],
    };
    typed
        .validate_for_profile(&fixture.profile, mode.id)
        .unwrap();
    let mut value = serde_json::to_value(typed).unwrap();
    value["future_calibration"] = json!({"profile_id": "must-not-change"});
    value["paths"][0]["future_path"] = json!({"retained": meter});
    value["paths"][0]["source_identity"]["future_identity"] = json!({"retained": true});
    value
}

fn install(rig: &TestRig, fixture: &Calibrated, root: Value, copy: Value) -> Value {
    let mut body = fixture.record.body.clone();
    body[&ROOT[1..]] = root;
    body["multipatch"][0]["color_calibration"] = copy;
    rig.source_profile(&fixture.record.profile);
    rig.source_object("patched_fixture", &fixture.id(), body.clone());
    body
}

fn occupied_fixture(rig: &TestRig, fixture: &Calibrated) {
    let mut occupied = fixture.record.body.clone();
    occupied["name"] = json!("Destination fixture");
    rig.target_object("patched_fixture", &fixture.id(), occupied);
}

fn conflicting_profile(
    fixture: &Calibrated,
    edit: impl FnOnce(&mut Value),
) -> FixtureProfileRevision {
    let mut body = fixture.record.profile.profile().clone();
    edit(&mut body);
    FixtureProfileRevision::from_profile(body).unwrap()
}

struct Imported {
    document: PortableShowDocument,
    body: Value,
    profile: FixtureProfile,
    fixture_id: String,
    preview: SelectiveShowImportPreview,
}

fn import(
    rig: &TestRig,
    fixture: &Calibrated,
    resolution: ImportProfileConflictResolution,
) -> Imported {
    let preview = rig.preview(
        rig.request("patched_fixture", &fixture.id())
            .with_mode(ImportLoadMode::AddToEnd)
            .resolve_profile(fixture.key(), resolution),
    );
    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert_eq!(
        preview.profiles.len(),
        1,
        "observations must never add profile dependencies: {:?}",
        preview.profiles
    );
    rig.apply(&preview).unwrap();
    // A fresh ShowStore open, not the committed in-memory candidate.
    let document = rig.target_document();
    let fixture_id = preview
        .objects
        .iter()
        .find(|entry| entry.source.kind() == "patched_fixture")
        .unwrap()
        .destination
        .id()
        .to_owned();
    let body = document
        .object("patched_fixture", &fixture_id)
        .unwrap()
        .body()
        .clone();
    let destination = preview.profiles[0].destination;
    let profile = serde_json::from_value(
        document
            .fixture_profile_revision(destination.profile_id, destination.revision)
            .unwrap()
            .profile()
            .clone(),
    )
    .unwrap();
    Imported {
        document,
        body,
        profile,
        fixture_id,
        preview,
    }
}

fn installed_fixture(rig: &TestRig, id: &str) -> PatchedFixture {
    rig.ports
        .installed
        .lock()
        .as_ref()
        .expect("actual runtime preparation installed the candidate")
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id.0.to_string() == id)
        .unwrap()
        .clone()
}

/// The compiled runtime calibration of the root (`None`) or a multipatch copy.
fn compiled_calibration(fixture: &PatchedFixture, copy: bool) -> InstalledColorCalibration {
    if copy {
        fixture.multipatch[0].color_calibration.clone()
    } else {
        fixture.color_calibration.clone()
    }
    .expect("installed calibration survived compilation")
}

/// Evaluate with the existing pure fixture forward API and the compiled runtime Color context.
fn evaluate(
    profile: &FixtureProfile,
    calibration: Option<&InstalledColorCalibration>,
    context: Option<&light_fixture::ColorCalibrationContext>,
    raw: [u32; 3],
) -> light_fixture::forward::ColorForwardResult {
    let forward = CompiledColorForward::compile_with_context(
        profile,
        profile.modes[0].id,
        calibration,
        context,
    )
    .unwrap()
    .unwrap();
    let mut output = forward.create_output();
    forward.evaluate(&raw, &mut output).unwrap();
    output.remove(0)
}

fn rebased(original: &Value, identity: &NativeColorIdentity) -> Value {
    let mut expected = original.clone();
    let source = &mut expected["paths"][0]["source_identity"];
    source["profile_id"] = json!(identity.profile_id);
    source["profile_digest"] = json!(identity.profile_digest);
    expected
}

fn red_full() -> [u32; 3] {
    [u32::MAX, 0, 0]
}

#[test]
fn duplicate_rebases_root_and_copy_calibrations_under_fixture_and_profile_collisions() {
    let rig = TestRig::new();
    let fixture = calibrated_fixture(1_100_000, 1);
    let measured = [
        Xyz {
            x: 0.61,
            y: 0.52,
            z: 0.43,
        },
        Xyz {
            x: 0.31,
            y: 0.22,
            z: 0.13,
        },
    ];
    let calibrations = [
        calibration(&fixture, [0.5, 1.0, 0.0], "meter-root", measured[0]),
        calibration(&fixture, [0.75, 0.25, 2.0], "meter-copy", measured[1]),
    ];
    install(
        &rig,
        &fixture,
        calibrations[0].clone(),
        calibrations[1].clone(),
    );
    occupied_fixture(&rig, &fixture);
    rig.target_profile(&conflicting_profile(&fixture, |body| {
        body["manufacturer"] = json!("Occupied immutable revision")
    }));
    let imported = import(&rig, &fixture, ImportProfileConflictResolution::Duplicate);
    assert!(matches!(
        imported.preview.profiles[0].action,
        ImportProfileAction::Duplicate { .. }
    ));
    assert_ne!(
        imported.fixture_id,
        fixture.id(),
        "fixture ID collision is duplicated"
    );
    assert_ne!(
        imported.profile.id, fixture.profile.id,
        "profile ID collision is duplicated"
    );
    let identity = imported
        .profile
        .native_color_identity(fixture.identity.mode_id, fixture.identity.head_id)
        .unwrap();
    assert_ne!(identity.profile_digest, fixture.identity.profile_digest);
    assert_eq!(
        NativeColorIdentity {
            profile_id: fixture.identity.profile_id,
            profile_digest: fixture.identity.profile_digest.clone(),
            ..identity.clone()
        },
        fixture.identity,
        "only the duplicated profile identity and digest may change"
    );
    let runtime = installed_fixture(&rig, &imported.fixture_id);
    let context = runtime
        .definition
        .runtime_color_context
        .clone()
        .expect("compiled from the imported immutable profile");
    for (index, pointer) in [ROOT, COPY].into_iter().enumerate() {
        // Every observation byte, provenance, recipe word and future field is unchanged.
        assert_eq!(
            imported.body.pointer(pointer).unwrap(),
            &rebased(&calibrations[index], &identity),
            "{pointer}"
        );
        let stored: InstalledColorCalibration =
            serde_json::from_value(imported.body.pointer(pointer).unwrap().clone()).unwrap();
        assert_eq!(
            stored.status(&imported.profile, imported.profile.modes[0].id),
            InstalledColorCalibrationStatus::Current
        );
        let compiled = compiled_calibration(&runtime, index == 1);
        assert_eq!(compiled, stored);
        compiled.validate_for_context(&context).unwrap();
        let original: InstalledColorCalibration =
            serde_json::from_value(calibrations[index].clone()).unwrap();
        // Output gains and the measured full-width recipe apply exactly as before the import.
        for raw in [red_full(), [0, 255, 0], [0, 0, 65_535], RECIPE] {
            let before = evaluate(&fixture.profile, Some(&original), None, raw);
            let after = evaluate(&imported.profile, Some(&compiled), Some(&context), raw);
            assert_eq!(after.known_xyz, before.known_xyz, "{pointer} {raw:?}");
            assert!(!after.flags.contains(ColorForwardFlags::STALE_CALIBRATION));
        }
        let gain = original.paths[0].emitters[0].output_gain;
        let red = evaluate(
            &imported.profile,
            Some(&compiled),
            Some(&context),
            red_full(),
        );
        let open = evaluate(&imported.profile, None, Some(&context), red_full());
        assert_eq!(red.known_xyz.y, open.known_xyz.y * gain, "{pointer}");
        assert_eq!(
            evaluate(&imported.profile, Some(&compiled), Some(&context), RECIPE).known_xyz,
            measured[index]
        );
    }
    // The occupied destination fixture keeps its own (absent) observations.
    assert!(
        imported
            .document
            .object("patched_fixture", &fixture.id())
            .unwrap()
            .body()
            .get("color_calibration")
            .is_none()
    );
}

#[test]
fn stale_foreign_and_absent_source_calibrations_stay_passive_under_duplicate() {
    // Each case leaves the root unchanged; the independent copy remains a valid current proof.
    let cases: [(&str, Edit); 5] = [
        ("stale digest", |id| {
            id["profile_digest"] = json!("b".repeat(64))
        }),
        ("stale native layout", |id| {
            id["native_layout_signature"] = json!("c".repeat(64))
        }),
        ("foreign profile", |id| {
            id["profile_id"] = json!(Uuid::from_u128(1_299_999))
        }),
        ("absent source revision", |id| {
            id["profile_revision"] = json!(3)
        }),
        // An exact identity whose observation no longer fits the original model is not current.
        ("dangling emitter observation", |_| {}),
    ];
    for (case, (label, edit)) in cases.into_iter().enumerate() {
        let rig = TestRig::new();
        let fixture = calibrated_fixture(1_200_000 + case as u128 * 1_000, 1);
        let mut root = calibration(&fixture, [0.5, 0.5, 0.5], "old meter", EMITTERS[0]);
        edit(&mut root["paths"][0]["source_identity"]);
        if label == "dangling emitter observation" {
            root["paths"][0]["emitters"][0]["emitter_id"] = json!(Uuid::from_u128(1_299_998));
        }
        let copy = calibration(&fixture, [1.5, 1.0, 1.0], "copy meter", EMITTERS[1]);
        install(&rig, &fixture, root.clone(), copy.clone());
        rig.target_profile(&conflicting_profile(&fixture, |body| {
            body["manufacturer"] = json!("Occupied immutable revision")
        }));
        let imported = import(&rig, &fixture, ImportProfileConflictResolution::Duplicate);
        let identity = imported
            .profile
            .native_color_identity(fixture.identity.mode_id, fixture.identity.head_id)
            .unwrap();
        assert_eq!(imported.body.pointer(ROOT).unwrap(), &root, "{label}");
        assert_eq!(
            imported.body.pointer(COPY).unwrap(),
            &rebased(&copy, &identity),
            "{label}"
        );
        let runtime = installed_fixture(&rig, &imported.fixture_id);
        let context = runtime.definition.runtime_color_context.clone().unwrap();
        let stale = compiled_calibration(&runtime, false);
        assert!(stale.validate_for_context(&context).is_err(), "{label}");
        let output = evaluate(&imported.profile, Some(&stale), Some(&context), red_full());
        assert!(
            output.flags.contains(ColorForwardFlags::STALE_CALIBRATION),
            "{label}"
        );
        assert_eq!(
            output.known_xyz,
            evaluate(&imported.profile, None, Some(&context), red_full()).known_xyz,
            "{label}: a stale gain is disabled, never applied"
        );
        let current = compiled_calibration(&runtime, true);
        current.validate_for_context(&context).unwrap();
        assert_eq!(
            evaluate(
                &imported.profile,
                Some(&current),
                Some(&context),
                red_full()
            )
            .known_xyz
            .y,
            evaluate(&imported.profile, None, Some(&context), red_full())
                .known_xyz
                .y
                * 1.5
        );
    }
}

#[test]
fn keep_retains_compatible_calibration_and_never_certifies_changed_optics_or_geometry() {
    let cases: [(&str, bool, Edit); 4] = [
        // Raw extension data changes the immutable digest but not the typed Color identity.
        ("compatible", true, |body| {
            body["future_profile"] = json!({"retained": "destination"})
        }),
        ("changed optics", false, |body| {
            body["modes"][0]["color_physical"]["paths"][0]["source"]["emitters"][0]["xyz"]["y"] =
                json!(0.3)
        }),
        ("changed geometry", false, |body| {
            let head = body["modes"][0]["heads"][0]["id"].clone();
            let head = serde_json::from_value(head).unwrap();
            body["geometry"] = serde_json::to_value(GeometryGraph::template(
                GeometryTemplate::MovingHead,
                &[head],
            ))
            .unwrap();
        }),
        ("changed control", false, |body| {
            body["modes"][0]["channels"][2]["default_raw"] = json!(17)
        }),
    ];
    for (case, (label, compatible, edit)) in cases.into_iter().enumerate() {
        let rig = TestRig::new();
        let fixture = calibrated_fixture(1_300_000 + case as u128 * 1_000, 1);
        let calibrations = [
            calibration(&fixture, [0.5, 1.0, 1.0], "root meter", EMITTERS[2]),
            calibration(&fixture, [1.0, 0.25, 1.0], "copy meter", EMITTERS[0]),
        ];
        install(
            &rig,
            &fixture,
            calibrations[0].clone(),
            calibrations[1].clone(),
        );
        let destination = conflicting_profile(&fixture, edit);
        assert_ne!(
            destination.digest(),
            fixture.record.profile.digest(),
            "{label}"
        );
        rig.target_profile(&destination);
        let imported = import(
            &rig,
            &fixture,
            ImportProfileConflictResolution::KeepDestination,
        );
        assert_eq!(
            imported.preview.profiles[0].action,
            ImportProfileAction::KeepDestination
        );
        let runtime = installed_fixture(&rig, &imported.fixture_id);
        let context = runtime.definition.runtime_color_context.clone().unwrap();
        for (index, pointer) in [ROOT, COPY].into_iter().enumerate() {
            assert_eq!(
                imported.body.pointer(pointer).unwrap(),
                &calibrations[index],
                "{label}: Keep never re-stamps an observation"
            );
            let compiled = compiled_calibration(&runtime, index == 1);
            assert_eq!(
                compiled.validate_for_context(&context).is_ok(),
                compatible,
                "{label} {pointer}"
            );
            let output = evaluate(
                &imported.profile,
                Some(&compiled),
                Some(&context),
                red_full(),
            );
            let open = evaluate(&imported.profile, None, Some(&context), red_full());
            assert_eq!(
                output.flags.contains(ColorForwardFlags::STALE_CALIBRATION),
                !compatible,
                "{label}"
            );
            let gain = if compatible {
                compiled.paths[0].emitters[0].output_gain
            } else {
                1.0
            };
            assert_eq!(
                output.known_xyz.y,
                open.known_xyz.y * gain,
                "{label} {pointer}"
            );
        }
    }
}

#[test]
fn failed_import_and_direct_keep_refusal_leave_the_destination_unchanged() {
    let rig = TestRig::new();
    let fixture = calibrated_fixture(1_400_000, 1);
    let root = calibration(&fixture, [0.5, 1.0, 1.0], "root meter", EMITTERS[0]);
    let copy = calibration(&fixture, [1.0, 0.5, 1.0], "copy meter", EMITTERS[1]);
    let channels = json!(
        fixture.profile.modes[0]
            .channels
            .iter()
            .zip(RECIPE)
            .map(|(channel, raw)| json!({
                "channel_id": channel.id, "function_id": channel.functions[0].id, "raw": raw
            }))
            .collect::<Vec<_>>()
    );
    let mut body = install(&rig, &fixture, root.clone(), copy.clone());
    body["freeze"] = json!({"targets":{fixture.id():{
        "full":false,"families":["color"],"values":{"color":{
            "kind":"color_program","value":{"kind":"direct",
                "recipe":{"source":fixture.identity,"channels":channels,"spreads":[]},
                "portable":{"model_revision":3,"visible":null,
                    "uv":{"amount":0.0,"quality":"estimated"},
                    "quality":"estimated","limitations":[]}
            }
        }}
    }}});
    rig.update_source_object("patched_fixture", &fixture.id(), body);
    occupied_fixture(&rig, &fixture);
    let destination = conflicting_profile(&fixture, |body| {
        body["manufacturer"] = json!("Incompatible destination")
    });
    rig.target_profile(&destination);
    let unchanged = |rig: &TestRig| {
        let document = rig.target_document();
        assert_eq!(document.objects_of_kind("patched_fixture").count(), 1);
        assert_eq!(document.fixture_profile_revisions().len(), 1);
        assert_eq!(
            document.fixture_profile_revisions()[0].digest(),
            destination.digest()
        );
        assert_eq!(
            document
                .object("patched_fixture", &fixture.id())
                .unwrap()
                .body()["name"],
            "Destination fixture"
        );
    };
    let request = |resolution| {
        rig.request("patched_fixture", &fixture.id())
            .with_mode(ImportLoadMode::AddToEnd)
            .resolve_profile(fixture.key(), resolution)
    };

    // Strict Direct recipe policy is unchanged, even though the observations themselves are passive.
    let keep = rig.preview(request(ImportProfileConflictResolution::KeepDestination));
    assert!(!keep.can_apply());
    assert!(
        keep.blockers.iter().any(|blocker| matches!(blocker,
            ImportBlocker::ReferenceRewrite { message, .. }
                if message == "destination profile changes the pinned native Color model; duplicate the source profile to preserve its recipe")),
        "{:?}",
        keep.blockers
    );
    assert!(rig.apply(&keep).is_err());
    unchanged(&rig);

    // Runtime preparation failure after a successful Duplicate rewrite leaves nothing behind.
    let duplicate = rig.preview(request(ImportProfileConflictResolution::Duplicate));
    assert!(duplicate.can_apply(), "{:?}", duplicate.blockers);
    rig.ports
        .fail_prepare
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(rig.apply(&duplicate).is_err());
    assert!(rig.ports.installed.lock().is_none());
    unchanged(&rig);

    // The same Duplicate commits once preparation succeeds; Direct and installed proofs agree.
    rig.ports
        .fail_prepare
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let imported = import(&rig, &fixture, ImportProfileConflictResolution::Duplicate);
    let identity = imported
        .profile
        .native_color_identity(fixture.identity.mode_id, fixture.identity.head_id)
        .unwrap();
    assert_eq!(
        imported.body["freeze"]["targets"][&imported.fixture_id]["values"]["color"]["value"]["recipe"]
            ["source"],
        serde_json::to_value(&identity).unwrap()
    );
    assert_eq!(
        imported.body.pointer(ROOT).unwrap(),
        &rebased(&root, &identity)
    );
    assert_eq!(
        imported.body.pointer(COPY).unwrap(),
        &rebased(&copy, &identity)
    );
}
