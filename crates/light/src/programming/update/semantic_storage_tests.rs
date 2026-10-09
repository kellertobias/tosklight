//! TL-627: requested semantic intent through actual Update commits, candidate preparation and
//! SQLite reopen.
//!
//! Every commit here runs the real `ProgrammingService::preview_update` / `handle_update` path:
//! `ProgrammerRegistry::capture_update_values` captures the Normal Programmer, the planner runs
//! inside `ActiveShowService::commit_programming_update`, `prepare_show_candidate` compiles the
//! candidate, and the parent rig's `TestPorts` validates, backs up and commits it to SQLite. The
//! `RetainingPorts` wrapper forwards to those ports and keeps the exact `EngineSnapshot` the
//! application installed, so installed-candidate claims compare retained install data, not a
//! recompiled stand-in. Every storage assertion reopens the file with `ShowStore::open`.
//!
//! Scope: Update captures only the Normal Programmer (no Preload lane exists for Update). Payload
//! equality proves retained authoring only; nothing here runs the Color solver, the live output
//! scheduler, physical output or the programming-contract startup gate.
use super::*;
use crate::prepare_show_candidate;
use crate::programming::semantic_intent_cases::{
    angles, assert_semantic_value, color, focus, group_family, magenta, magenta_with_uv,
    point_target, uv_only_black, warm_white_3200, warm_white_zero_output, zoom,
};
use light_core::OpeningConvention;
use light_core::programming::{
    ColorProgram, NativeColorRecipe, PortableColorEstimate, PortableUv, PortableVisibleColor,
    PositionIntent, ProgrammingOwner, ScalarIntent, TargetReference, ZoomIntent,
};
use light_core::{NativeColorIdentity, NativeColorValue, PhysicalDataQuality, Xyz};
use light_playback::GroupCueChange;

const GROUP: &str = "tl627-front";
const POINT: Uuid = Uuid::from_u128(0x0627_0f0f);
const PLAYBACK: u16 = 7;

fn fixture_a() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00a1))
}
fn fixture_b() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00b1))
}
fn fixture_c() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00c1))
}
/// Outside the Group: an authored fixture UUID and stored member exception.
/// No fixture entity/profile/patch is seeded here; this is storage preservation coverage.
fn dormant() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00d1))
}
/// Present only as a new address in the Programmer.
fn fixture_e() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00e1))
}
/// Stored but never in the Programmer: must survive every Update untouched.
fn unrelated() -> FixtureId {
    FixtureId(Uuid::from_u128(0x0627_00f1))
}

fn key(owner: ProgrammingOwner) -> AttributeKey {
    owner.key()
}

/// Angles beyond one turn. Storage must not wrap them into a canonical range.
fn unwrapped_angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(540.0, -200.5)))
}

fn narrow_zoom() -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(10.0),
        convention: OpeningConvention::Beam,
    }))
}

/// A tagged Direct Color: pinned native identity, exact full raw words and a portable estimate.
fn direct(raw: u32) -> AttributeValue {
    let source = NativeColorIdentity {
        profile_id: Uuid::from_u128(0x0006_2701),
        profile_revision: 4,
        profile_digest: "tl627-profile-digest".into(),
        mode_id: Uuid::from_u128(0x0006_2702),
        head_id: Uuid::from_u128(0x0006_2703),
        path_id: Uuid::from_u128(0x0006_2704),
        model_revision: 2,
        native_layout_signature: "tl627-layout".into(),
    };
    let program = ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source,
            channels: (0..3u128)
                .map(|index| NativeColorValue {
                    channel_id: Uuid::from_u128(0x0006_2710 + index),
                    function_id: Uuid::from_u128(0x0006_2720 + index),
                    // Full 16-bit native words, not 8-bit levels.
                    raw: raw + index as u32 * 0x1111,
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 2,
            visible: Some(PortableVisibleColor {
                xyz: Xyz {
                    x: 0.11,
                    y: 0.07,
                    z: 0.02,
                },
                relative_output: 1.0,
            }),
            uv: Some(PortableUv {
                amount: 0.4,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["recorded".into()],
        },
    };
    program.validate().unwrap();
    AttributeValue::ColorProgram(Arc::new(program))
}

type FixtureRow = (FixtureId, AttributeKey, AttributeValue);
type GroupRow = (String, AttributeKey, AttributeValue);

/// Requested intent at exact addresses, in Programmer order.
#[derive(Clone, Default)]
struct Intent {
    fixtures: Vec<FixtureRow>,
    groups: Vec<GroupRow>,
}

impl Intent {
    fn chain(mut self, other: Intent) -> Self {
        self.fixtures.extend(other.fixtures);
        self.groups.extend(other.groups);
        self
    }

    fn filter(&self, keep: impl Fn(&AttributeKey) -> bool) -> Self {
        Self {
            fixtures: self
                .fixtures
                .iter()
                .filter(|row| keep(&row.1))
                .cloned()
                .collect(),
            groups: self
                .groups
                .iter()
                .filter(|row| keep(&row.1))
                .cloned()
                .collect(),
        }
    }
}

/// The complete new requested intent the operator holds in the Programmer.
fn new_intent() -> Intent {
    Intent {
        fixtures: vec![
            (
                fixture_a(),
                key(ProgrammingOwner::Color),
                color(magenta_with_uv()),
            ),
            (
                fixture_a(),
                key(ProgrammingOwner::Position),
                unwrapped_angles(),
            ),
            (fixture_a(), key(ProgrammingOwner::Focus), focus()),
            (fixture_a(), key(ProgrammingOwner::Zoom), zoom()),
            (fixture_b(), key(ProgrammingOwner::Color), direct(0x1000)),
            (
                fixture_b(),
                key(ProgrammingOwner::Position),
                point_target(POINT),
            ),
            (
                fixture_c(),
                key(ProgrammingOwner::Color),
                color(warm_white_3200()),
            ),
            (
                dormant(),
                key(ProgrammingOwner::Color),
                color(uv_only_black()),
            ),
        ],
        groups: vec![
            (
                GROUP.into(),
                key(ProgrammingOwner::Color),
                group_family(
                    ProgrammingOwner::Color,
                    color(uv_only_black()),
                    [
                        (fixture_a().0, color(magenta_with_uv())),
                        (dormant().0, color(warm_white_zero_output())),
                    ],
                ),
            ),
            (
                GROUP.into(),
                key(ProgrammingOwner::Position),
                group_family(
                    ProgrammingOwner::Position,
                    unwrapped_angles(),
                    [(fixture_b().0, point_target(POINT))],
                ),
            ),
            (GROUP.into(), key(ProgrammingOwner::Focus), focus()),
            (
                GROUP.into(),
                key(ProgrammingOwner::Zoom),
                group_family(ProgrammingOwner::Zoom, zoom(), []),
            ),
        ],
    }
}

/// The previously stored intent at exactly the addresses of [`new_intent`], every value different.
fn old_intent() -> Intent {
    Intent {
        fixtures: vec![
            (fixture_a(), key(ProgrammingOwner::Color), color(magenta())),
            (fixture_a(), key(ProgrammingOwner::Position), angles()),
            (
                fixture_a(),
                key(ProgrammingOwner::Focus),
                AttributeValue::Normalized(0.1),
            ),
            (fixture_a(), key(ProgrammingOwner::Zoom), narrow_zoom()),
            (fixture_b(), key(ProgrammingOwner::Color), direct(0x2000)),
            (fixture_b(), key(ProgrammingOwner::Position), angles()),
            (
                fixture_c(),
                key(ProgrammingOwner::Color),
                color(warm_white_zero_output()),
            ),
            (dormant(), key(ProgrammingOwner::Color), color(magenta())),
        ],
        groups: vec![
            (
                GROUP.into(),
                key(ProgrammingOwner::Color),
                group_family(
                    ProgrammingOwner::Color,
                    color(magenta()),
                    [(fixture_a().0, color(warm_white_3200()))],
                ),
            ),
            (
                GROUP.into(),
                key(ProgrammingOwner::Position),
                group_family(ProgrammingOwner::Position, angles(), []),
            ),
            (
                GROUP.into(),
                key(ProgrammingOwner::Focus),
                AttributeValue::Normalized(0.2),
            ),
            (
                GROUP.into(),
                key(ProgrammingOwner::Zoom),
                group_family(ProgrammingOwner::Zoom, narrow_zoom(), []),
            ),
        ],
    }
}

/// Programmer addresses with no stored event or Preset value: one per Preset family.
fn new_addresses() -> Intent {
    Intent {
        fixtures: vec![
            (
                fixture_e(),
                key(ProgrammingOwner::Color),
                color(warm_white_zero_output()),
            ),
            (fixture_c(), key(ProgrammingOwner::Position), angles()),
            (fixture_b(), key(ProgrammingOwner::Zoom), zoom()),
        ],
        groups: Vec::new(),
    }
}

fn programmer_intent() -> Intent {
    new_intent().chain(new_addresses())
}

fn seed_group(rig: &TestRig) {
    rig.seed(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "TL-627 front".into(),
            fixtures: vec![fixture_a(), fixture_b()],
            ..GroupDefinition::default()
        })
        .unwrap(),
    );
}

/// Puts `intent` into the desk's Normal Programmer through the operator setters.
fn program(rig: &TestRig, intent: &Intent) {
    for (fixture_id, attribute, value) in &intent.fixtures {
        rig.registry
            .set(rig.session, *fixture_id, attribute.clone(), value.clone());
    }
    for (group_id, attribute, value) in &intent.groups {
        assert!(rig.registry.set_group(
            rig.session,
            group_id.clone(),
            attribute.clone(),
            value.clone()
        ));
    }
}

// --- Ports that retain the actually installed runtime -------------------------------------------

/// Forwards every port to the parent rig's `TestPorts` and retains each installed snapshot.
struct RetainingPorts<'a> {
    inner: &'a TestPorts,
    installed: Mutex<Vec<EngineSnapshot>>,
    fail_prepare: bool,
}

impl<'a> RetainingPorts<'a> {
    fn new(rig: &'a TestRig) -> Self {
        Self {
            inner: &rig.ports,
            installed: Mutex::default(),
            fail_prepare: false,
        }
    }

    fn failing(rig: &'a TestRig) -> Self {
        Self {
            fail_prepare: true,
            ..Self::new(rig)
        }
    }

    fn installs(&self) -> usize {
        self.installed.lock().len()
    }
}

impl ActiveShowPorts for RetainingPorts<'_> {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.inner.begin_active_show(context, show_id)
    }

    fn prepare_object_undo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        self.inner
            .prepare_object_undo(unit, kind, object_id, expected_object_revision)
    }

    fn prepare_runtime(&self, snapshot: EngineSnapshot) -> Result<EngineSnapshot, ActionError> {
        if self.fail_prepare {
            self.inner.steps.lock().push("prepare");
            return Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "runtime preparation rejected the candidate",
            ));
        }
        self.inner.prepare_runtime(snapshot)
    }

    fn install_runtime(&self, context: &ActionContext, prepared: EngineSnapshot) {
        self.installed.lock().push(prepared.clone());
        self.inner.install_runtime(context, prepared);
    }
}

impl ProgrammingUpdatePorts for RetainingPorts<'_> {
    fn authorize_programming_update(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.inner.authorize_programming_update(context)
    }

    fn active_update_cue_contexts(
        &self,
        context: &ActionContext,
    ) -> Result<Vec<ActiveCueContext>, ActionError> {
        self.inner.active_update_cue_contexts(context)
    }

    fn reconcile_programming_update(&self, projection: &ProgrammingUpdateProjection) {
        self.inner.reconcile_programming_update(projection);
    }
}

fn context(rig: &TestRig, request_id: &str) -> ActionContext {
    ActionContext::operator(rig.desk, rig.session.0, ActionSource::Http).with_request_id(request_id)
}

fn preview(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
    target: ProgrammingUpdateTargetRequest,
    mode: UpdateMode,
    request_id: &str,
) -> Result<ProgrammingUpdatePreviewResult, ActionError> {
    rig.service.preview_update(
        ActionEnvelope {
            context: context(rig, request_id),
            command: ProgrammingUpdatePreviewRequest {
                show_id: rig.show_id,
                target,
                mode,
            },
        },
        &rig.active_show,
        ports,
    )
}

fn apply(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
    command: ProgrammingUpdateCommand,
    request_id: &str,
) -> Result<ProgrammingUpdateResult, ActionError> {
    rig.service.handle_update(
        ActionEnvelope {
            context: context(rig, request_id),
            command,
        },
        &rig.active_show,
        ports,
    )
}

/// Preview, then confirm exactly that preview, as the desk's Update dialog does.
fn preview_and_command(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
    target: ProgrammingUpdateTargetRequest,
    mode: UpdateMode,
    request_id: &str,
) -> ProgrammingUpdateCommand {
    let preview = preview(rig, ports, target.clone(), mode, request_id).unwrap();
    command_from_preview(rig, target, preview)
}

// --- Reopened storage, revision, event and runtime observation -----------------------------------

fn document(rig: &TestRig) -> PortableShowDocument {
    ShowStore::open(&rig.ports.path)
        .unwrap()
        .portable_document()
        .unwrap()
}

/// Everything a rejected Update must leave untouched: every stored object with its revision and
/// body, the show revision, the raw SQLite main and WAL bytes, the event sequence and the installs.
#[derive(Debug, PartialEq)]
struct Observed {
    revision: u64,
    objects: Vec<(String, String, u64, Value)>,
    files: Vec<Option<Vec<u8>>>,
    events: u64,
    installs: usize,
}

fn observe(rig: &TestRig, ports: &RetainingPorts<'_>) -> Observed {
    let document = document(rig);
    let mut objects = document
        .objects()
        .map(|object| {
            (
                object.key().kind().to_owned(),
                object.key().id().to_owned(),
                object.revision(),
                object.body().clone(),
            )
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    Observed {
        revision: document.revision().value(),
        objects,
        files: ["", "-wal"]
            .into_iter()
            .map(|suffix| std::fs::read(format!("{}{suffix}", rig.ports.path.display())).ok())
            .collect(),
        events: rig.active_show.events().latest_sequence(),
        installs: ports.installs(),
    }
}

fn assert_no_side_effects(rig: &TestRig, label: &str) {
    let steps = rig.steps();
    for step in ["prepare", "backup", "commit", "install", "reconcile"] {
        assert!(
            !steps.contains(&step),
            "{label}: unexpected {step} in {steps:?}"
        );
    }
}

fn assert_one_commit_lifecycle(rig: &TestRig, label: &str) {
    let steps = rig.steps();
    for step in ["prepare", "backup", "commit", "install", "reconcile"] {
        assert_eq!(
            steps.iter().filter(|seen| **seen == step).count(),
            1,
            "{label}: {step} in {steps:?}"
        );
    }
}

/// The snapshot the application actually installed equals a fresh show-open compile of the
/// reopened SQLite document, and carries the committed revision.
fn assert_installed_is_reopened_compile(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
) -> EngineSnapshot {
    let document = document(rig);
    let installed = ports.installed.lock().last().cloned().expect("installed");
    assert_eq!(installed.revision, document.revision().value());
    let (_, reopened) = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts();
    assert_eq!(
        serde_json::to_value(&installed).unwrap(),
        serde_json::to_value(&reopened).unwrap(),
        "the whole installed snapshot is the reopened compile"
    );
    assert_eq!(installed.cue_lists, reopened.cue_lists);
    assert_eq!(
        serde_json::to_value(&installed.groups).unwrap(),
        serde_json::to_value(&reopened.groups).unwrap()
    );
    assert_eq!(
        installed.required_programming_contract,
        reopened.required_programming_contract
    );
    installed
}

/// One stored row with an exact target and attribute.
fn stored_row<'a>(rows: &'a Value, target_field: &str, target: &str, attribute: &str) -> &'a Value {
    let matches = rows
        .as_array()
        .expect("stored rows")
        .iter()
        .filter(|row| row[target_field] == target && row["attribute"] == attribute)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{target}/{attribute} in {rows}");
    matches[0]
}

/// A stored value is the exact JSON of the requested value. Semantic rows carry no native
/// recipe, estimate or wheel substitution; a Direct row is the exact tagged program.
fn assert_exact_value(stored: &Value, requested: &AttributeValue) {
    assert_eq!(stored, &serde_json::to_value(requested).unwrap());
    if stored["value"]["kind"] == "direct" {
        assert!(matches!(
            requested,
            AttributeValue::ColorProgram(program) if matches!(**program, ColorProgram::Direct { .. })
        ));
    } else {
        assert_semantic_value(stored);
    }
}

/// Literal persisted facts the acceptance criteria name, read back from one reopened row set.
fn assert_named_facts(fixture_rows: impl Fn(FixtureId, &AttributeKey) -> Value) {
    let a_color = fixture_rows(fixture_a(), &key(ProgrammingOwner::Color));
    let intent = &a_color["value"]["intent"];
    assert_eq!(intent["uv"]["amount"].as_f64().unwrap() as f32, 0.45);
    assert_eq!(intent["white_blend"].as_f64().unwrap() as f32, 0.25);
    assert_eq!(
        intent["white_target"]["kelvin"].as_f64().unwrap() as f32,
        5600.0
    );
    assert_eq!(
        intent["white_target"]["duv"].as_f64().unwrap() as f32,
        -0.004
    );
    assert_eq!(intent["relative_output"].as_f64().unwrap() as f32, 0.8);
    let uv_only = fixture_rows(dormant(), &key(ProgrammingOwner::Color));
    let uv_only = &uv_only["value"]["intent"];
    assert_eq!(uv_only["uv"]["amount"].as_f64().unwrap() as f32, 0.9);
    assert_eq!(uv_only["base_xyz"]["y"], json!(0.0), "UV-only zero-Y kept");
    assert_eq!(uv_only["relative_output"], json!(0.0), "zero output kept");
    let warm = fixture_rows(fixture_c(), &key(ProgrammingOwner::Color));
    assert_eq!(
        warm["value"]["intent"]["white_target"]["kelvin"]
            .as_f64()
            .unwrap() as f32,
        3200.0
    );
    let direct_row = fixture_rows(fixture_b(), &key(ProgrammingOwner::Color));
    assert_eq!(direct_row["value"]["kind"], "direct");
    assert_eq!(
        direct_row["value"]["recipe"]["source"]["profile_digest"],
        "tl627-profile-digest"
    );
    let raws = direct_row["value"]["recipe"]["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|channel| channel["raw"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(raws, [0x1000, 0x2111, 0x3222], "full native words kept");

    let decode = |value: Value| serde_json::from_value::<AttributeValue>(value).unwrap();
    assert_eq!(
        decode(fixture_rows(fixture_a(), &key(ProgrammingOwner::Position))),
        unwrapped_angles(),
        "unwrapped Angles are not wrapped"
    );
    let AttributeValue::Position(target) =
        decode(fixture_rows(fixture_b(), &key(ProgrammingOwner::Position)))
    else {
        panic!("Target row")
    };
    assert_eq!(
        *target,
        PositionIntent::Target {
            reference: TargetReference::Point { point_id: POINT },
            offset_metres: [0.5, -1.25, 2.0].map(ScalarIntent::Value),
        }
    );
    assert_eq!(
        decode(fixture_rows(fixture_a(), &key(ProgrammingOwner::Focus))),
        focus()
    );
    assert_eq!(
        decode(fixture_rows(fixture_a(), &key(ProgrammingOwner::Zoom))),
        zoom()
    );
}

// --- Preset Update ---------------------------------------------------------------------------------

fn preset_object_id(family: PresetFamily) -> &'static str {
    match family {
        PresetFamily::Color => "2.1",
        PresetFamily::Position => "3.1",
        PresetFamily::Beam => "4.1",
        other => panic!("unused family {other:?}"),
    }
}

fn preset_from(family: PresetFamily, intent: &Intent) -> Preset {
    let mut preset = Preset {
        instance_id: None,
        name: format!("TL-627 {family:?}"),
        family,
        number: 1,
        values: HashMap::new(),
        group_values: HashMap::new(),
        aim_at_fixture_number: None,
        universal_values: HashMap::new(),
    };
    for (fixture_id, attribute, value) in &intent.fixtures {
        preset
            .values
            .entry(*fixture_id)
            .or_default()
            .insert(attribute.clone(), value.clone());
    }
    for (group_id, attribute, value) in &intent.groups {
        preset
            .group_values
            .entry(group_id.clone())
            .or_default()
            .insert(attribute.clone(), value.clone());
    }
    preset
}

fn unrelated_preset_row(family: PresetFamily) -> Intent {
    let (owner, value) = match family {
        PresetFamily::Color => (ProgrammingOwner::Color, color(warm_white_3200())),
        PresetFamily::Position => (ProgrammingOwner::Position, point_target(POINT)),
        PresetFamily::Beam => (ProgrammingOwner::Focus, AttributeValue::Normalized(0.66)),
        other => panic!("unused family {other:?}"),
    };
    Intent {
        fixtures: vec![(unrelated(), key(owner), value)],
        groups: Vec::new(),
    }
}

/// Seeds one Preset holding the old intent of its family beside an unrelated value and an unknown
/// future field, and loads the full Programmer (every family plus an Intensity value).
fn preset_rig(family: PresetFamily) -> TestRig {
    let rig = TestRig::new();
    seed_group(&rig);
    let stored = old_intent()
        .filter(|attribute| family.accepts(attribute))
        .chain(unrelated_preset_row(family));
    let mut body = serde_json::to_value(preset_from(family, &stored)).unwrap();
    body["future_preset"] = json!({"keep": [1, "two"]});
    rig.seed("preset", preset_object_id(family), body);
    program(
        &rig,
        &programmer_intent().chain(Intent {
            fixtures: vec![(
                fixture_a(),
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.7),
            )],
            groups: Vec::new(),
        }),
    );
    rig
}

fn preset_target_of(family: PresetFamily) -> ProgrammingUpdateTargetRequest {
    ProgrammingUpdateTargetRequest::Preset {
        object_id: preset_object_id(family).into(),
    }
}

#[test]
fn preset_update_commits_exact_semantic_and_direct_payloads_after_reopen() {
    for family in [
        PresetFamily::Color,
        PresetFamily::Position,
        PresetFamily::Beam,
    ] {
        for mode in [
            ExistingContentMode::UpdateExisting,
            ExistingContentMode::AddNew,
        ] {
            let label = format!("{family:?}/{mode:?}");
            let rig = preset_rig(family);
            let ports = RetainingPorts::new(&rig);
            let object_id = preset_object_id(family);
            let before = document(&rig);
            let revision_before = before.object("preset", object_id).unwrap().revision();

            let command = preview_and_command(
                &rig,
                &ports,
                preset_target_of(family),
                UpdateMode::ExistingContent(mode),
                &format!("{label}-preview"),
            );
            rig.clear_steps();
            let result = apply(&rig, &ports, command.clone(), &label).unwrap();
            assert!(!result.replayed, "{label}");
            assert_one_commit_lifecycle(&rig, &label);

            let accepted = |attribute: &AttributeKey| family.accepts(attribute);
            let updated = new_intent().filter(accepted);
            let added = new_addresses().filter(accepted);
            let mut expected = updated.clone().chain(unrelated_preset_row(family));
            if mode == ExistingContentMode::AddNew {
                expected = expected.chain(added.clone());
            }
            let summary = &result.outcome.summary;
            assert_eq!(
                summary.changed_count,
                updated.fixtures.len()
                    + updated.groups.len()
                    + if mode == ExistingContentMode::AddNew {
                        added.fixtures.len()
                    } else {
                        0
                    },
                "{label}"
            );
            assert_eq!(summary.revision_before, revision_before, "{label}");

            // Reopened storage holds exactly the expected Preset and nothing from other families.
            let reopened = document(&rig);
            let object = reopened.object("preset", object_id).unwrap();
            assert_eq!(object.revision(), summary.revision_after, "{label}");
            assert_eq!(
                object.body(),
                result.outcome.projection.raw_body.as_ref(),
                "{label}: the committed projection is the reopened body"
            );
            assert_eq!(object.body()["future_preset"], json!({"keep": [1, "two"]}));
            assert_eq!(object.body()["future"], json!({"keep": true}));
            let typed: Preset = serde_json::from_value(object.body().clone()).unwrap();
            let expected_preset = preset_from(family, &expected);
            assert_eq!(typed.values, expected_preset.values, "{label}");
            assert_eq!(typed.group_values, expected_preset.group_values, "{label}");
            assert!(typed.universal_values.is_empty(), "{label}");
            for (fixture_id, attribute, value) in &expected.fixtures {
                assert_exact_value(
                    &object.body()["values"][fixture_id.0.to_string()][attribute.0.as_ref()],
                    value,
                );
            }
            for (group_id, attribute, value) in &expected.groups {
                assert_exact_value(
                    &object.body()["group_values"][group_id][attribute.0.as_ref()],
                    value,
                );
            }
            if family == PresetFamily::Color {
                assert_eq!(
                    object.body()["group_values"][GROUP]["color"]["value"]["members"]
                        [dormant().0.to_string()],
                    serde_json::to_value(color(warm_white_zero_output())).unwrap(),
                    "dormant Group member exception kept"
                );
            }

            // The installed candidate is the actual install and equals a reopened compile.
            assert_eq!(ports.installs(), 1, "{label}");
            assert_installed_is_reopened_compile(&rig, &ports);
            assert_eq!(rig.active_show.events().latest_sequence(), 1, "{label}");

            // Replaying the same request returns the retained result: no commit, event or install.
            let committed = observe(&rig, &ports);
            rig.clear_steps();
            let replay = apply(&rig, &ports, command, &label).unwrap();
            assert!(replay.replayed, "{label}");
            assert_eq!(replay.outcome, result.outcome, "{label}");
            assert_no_side_effects(&rig, &label);
            assert_eq!(observe(&rig, &ports), committed, "{label}: replay");

            // Identical intent after reopen: the documented Update no-op is Invalid, no commit.
            let identical = preview_and_command(
                &rig,
                &ports,
                preset_target_of(family),
                UpdateMode::ExistingContent(mode),
                &format!("{label}-identical-preview"),
            );
            rig.clear_steps();
            let error = apply(&rig, &ports, identical, &format!("{label}-identical")).unwrap_err();
            assert_eq!(error.kind, ActionErrorKind::Invalid, "{label}: {error:?}");
            assert_no_side_effects(&rig, &label);
            assert_eq!(observe(&rig, &ports), committed, "{label}: identical");
        }
    }
}

#[test]
fn preset_update_reopens_with_the_named_semantic_and_direct_facts() {
    let rig = preset_rig(PresetFamily::Color);
    let ports = RetainingPorts::new(&rig);
    let command = preview_and_command(
        &rig,
        &ports,
        preset_target_of(PresetFamily::Color),
        UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
        "facts-preview",
    );
    apply(&rig, &ports, command, "facts").unwrap();
    let color_body = document(&rig)
        .object("preset", "2.1")
        .unwrap()
        .body()
        .clone();

    let rig = preset_rig(PresetFamily::Position);
    let ports = RetainingPorts::new(&rig);
    let command = preview_and_command(
        &rig,
        &ports,
        preset_target_of(PresetFamily::Position),
        UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
        "facts-preview",
    );
    apply(&rig, &ports, command, "facts").unwrap();
    let position_body = document(&rig)
        .object("preset", "3.1")
        .unwrap()
        .body()
        .clone();

    let rig = preset_rig(PresetFamily::Beam);
    let ports = RetainingPorts::new(&rig);
    let command = preview_and_command(
        &rig,
        &ports,
        preset_target_of(PresetFamily::Beam),
        UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
        "facts-preview",
    );
    apply(&rig, &ports, command, "facts").unwrap();
    let beam_body = document(&rig)
        .object("preset", "4.1")
        .unwrap()
        .body()
        .clone();

    assert_named_facts(|fixture, attribute| {
        [&color_body, &position_body, &beam_body]
            .into_iter()
            .find_map(|body| {
                body["values"]
                    .get(fixture.0.to_string())
                    .and_then(|values| values.get(attribute.0.as_ref()))
                    .cloned()
            })
            .unwrap_or_else(|| panic!("{fixture:?}/{attribute:?}"))
    });
}

// --- Source-aware Cue Update ---------------------------------------------------------------------

struct CueLayout {
    cue_list_id: CueListId,
    cues: Vec<Cue>,
}

impl CueLayout {
    fn context(&self, index: usize) -> ActiveCueContext {
        ActiveCueContext {
            playback_number: PLAYBACK,
            cue_list_id: self.cue_list_id,
            cue_id: self.cues[index].id,
            cue_number: self.cues[index].number.clone(),
        }
    }

    fn target(&self, index: usize) -> ProgrammingUpdateTargetRequest {
        ProgrammingUpdateTargetRequest::Cue {
            cue_list_id: self.cue_list_id,
            playback_number: Some(PLAYBACK),
            cue_id: Some(self.cues[index].id),
            cue_number: Some(self.cues[index].number.clone()),
            validate_active_context: true,
        }
    }
}

fn cue_with(number: f64, intent: &Intent) -> Cue {
    let mut cue = Cue::new(crate::CueNumber::try_from_legacy_f64(number).unwrap());
    cue.changes = intent
        .fixtures
        .iter()
        .map(|(fixture_id, attribute, value)| {
            CueChange::set(*fixture_id, attribute.clone(), value.clone())
        })
        .collect();
    cue.group_changes = intent
        .groups
        .iter()
        .map(|(group_id, attribute, value)| GroupCueChange {
            preset_reference: None,
            group_id: group_id.clone(),
            attribute: attribute.clone(),
            value: Some(value.clone()),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        })
        .collect();
    cue
}

fn unrelated_cue_row() -> Intent {
    Intent {
        fixtures: vec![(
            unrelated(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        )],
        groups: Vec::new(),
    }
}

/// Later Cue values at addresses the Programmer also holds. No Update mode may overwrite them.
fn later_intent() -> Intent {
    Intent {
        fixtures: vec![
            (
                fixture_a(),
                key(ProgrammingOwner::Color),
                color(warm_white_3200()),
            ),
            (
                fixture_a(),
                key(ProgrammingOwner::Position),
                point_target(POINT),
            ),
        ],
        groups: vec![(
            GROUP.into(),
            key(ProgrammingOwner::Color),
            group_family(ProgrammingOwner::Color, color(magenta_with_uv()), []),
        )],
    }
}

/// Cue 1 tracks the old intent, Cue 2 (the later active Cue) holds only an unrelated value and
/// Cue 3 holds later values at Programmer addresses. Rows carry unknown future fields.
fn cue_rig() -> (TestRig, CueLayout) {
    let rig = TestRig::new();
    seed_group(&rig);
    let cue_list_id = CueListId(Uuid::from_u128(0x0627_c11e));
    let cues = vec![
        cue_with(1.0, &old_intent()),
        cue_with(2.0, &unrelated_cue_row()),
        cue_with(3.0, &later_intent()),
    ];
    let mut body = cue_list_body(cue_list_id, cues[0].clone());
    for cue in &cues[1..] {
        body["cues"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(cue).unwrap());
    }
    body["cues"][0]["future_cue"] = json!({"keep": "nested"});
    body["cues"][0]["changes"][0]["future_change"] = json!({"keep": 42});
    body["cues"][0]["group_changes"][0]["future_group_change"] = json!({"keep": [true]});
    body["cues"][1]["changes"][0]["future_change"] = json!({"keep": "unrelated"});
    rig.seed("cue_list", "tl627-cuelist", body);
    program(&rig, &programmer_intent());
    (rig, CueLayout { cue_list_id, cues })
}

fn stored_cues(rig: &TestRig) -> Vec<Value> {
    document(rig)
        .object("cue_list", "tl627-cuelist")
        .unwrap()
        .body()["cues"]
        .as_array()
        .unwrap()
        .clone()
}

fn sorted_changes(changes: &[CueChange]) -> Vec<CueChange> {
    let mut changes = changes.to_vec();
    changes.sort_by_key(|change| (change.fixture_id.0, change.attribute.0.to_string()));
    changes
}

fn sorted_group_changes(changes: &[GroupCueChange]) -> Vec<GroupCueChange> {
    let mut changes = changes.to_vec();
    changes.sort_by_key(|change| (change.group_id.clone(), change.attribute.0.to_string()));
    changes
}

/// A reopened stored Cue holds exactly `intent`: every row present once with its exact payload,
/// and the typed decode equals the requested changes.
fn assert_cue_holds(cue: &Value, intent: &Intent, label: &str) {
    assert_eq!(
        cue["changes"].as_array().unwrap().len(),
        intent.fixtures.len(),
        "{label}: {}",
        cue["changes"]
    );
    assert_eq!(
        cue["group_changes"].as_array().unwrap().len(),
        intent.groups.len(),
        "{label}: {}",
        cue["group_changes"]
    );
    for (fixture_id, attribute, value) in &intent.fixtures {
        let row = stored_row(
            &cue["changes"],
            "fixture_id",
            &fixture_id.0.to_string(),
            &attribute.0,
        );
        assert_exact_value(&row["value"], value);
        assert_eq!(row["automatic_restore"], json!(false), "{label}");
    }
    for (group_id, attribute, value) in &intent.groups {
        let row = stored_row(&cue["group_changes"], "group_id", group_id, &attribute.0);
        assert_exact_value(&row["value"], value);
    }
    let typed: Cue = serde_json::from_value(cue.clone()).unwrap();
    let expected = cue_with(1.0, intent);
    assert_eq!(
        sorted_changes(&typed.changes),
        sorted_changes(&expected.changes),
        "{label}"
    );
    assert_eq!(
        sorted_group_changes(&typed.group_changes),
        sorted_group_changes(&expected.group_changes),
        "{label}"
    );
}

/// What each Cue must hold after one mode's Update, or `None` when that Cue's stored JSON must be
/// byte-for-byte what it was before.
struct CueExpectation {
    mode: CueUpdateMode,
    active: usize,
    cues: [Option<Intent>; 3],
    changed_cue: usize,
}

#[test]
fn cue_update_modes_write_exact_intent_to_their_actual_sources_only() {
    let cases = [
        // Existing Only rewrites the original tracked source; the later active Cue gains nothing.
        CueExpectation {
            mode: CueUpdateMode::ExistingOnly,
            active: 1,
            cues: [Some(new_intent()), None, None],
            changed_cue: 0,
        },
        CueExpectation {
            mode: CueUpdateMode::ExistingInCurrentCue,
            active: 0,
            cues: [Some(new_intent()), None, None],
            changed_cue: 0,
        },
        CueExpectation {
            mode: CueUpdateMode::AddToCurrentCue,
            active: 1,
            cues: [None, Some(unrelated_cue_row().chain(new_intent())), None],
            changed_cue: 1,
        },
        CueExpectation {
            mode: CueUpdateMode::AddNew,
            active: 1,
            cues: [
                None,
                Some(
                    unrelated_cue_row()
                        .chain(new_intent())
                        .chain(new_addresses()),
                ),
                None,
            ],
            changed_cue: 1,
        },
    ];
    for case in cases {
        let label = format!("{:?}@{}", case.mode, case.active + 1);
        let (rig, layout) = cue_rig();
        let ports = RetainingPorts::new(&rig);
        let before = stored_cues(&rig);
        rig.set_active_contexts(vec![layout.context(case.active)]);

        let command = preview_and_command(
            &rig,
            &ports,
            layout.target(case.active),
            UpdateMode::Cue(case.mode),
            &format!("{label}-preview"),
        );
        rig.clear_steps();
        let result = apply(&rig, &ports, command.clone(), &label).unwrap();
        assert_one_commit_lifecycle(&rig, &label);
        let summary = &result.outcome.summary;
        assert_eq!(
            summary
                .changed_cues
                .iter()
                .map(|cue| cue.cue_id)
                .collect::<Vec<_>>(),
            [layout.cues[case.changed_cue].id],
            "{label}"
        );
        let expected_changed = case.cues[case.changed_cue].as_ref().unwrap();
        let written = expected_changed.fixtures.len() + expected_changed.groups.len()
            - if case.changed_cue == 1 { 1 } else { 0 };
        assert_eq!(summary.changed_count, written, "{label}");

        let after = stored_cues(&rig);
        assert_eq!(after.len(), 3, "{label}");
        for (index, expected) in case.cues.iter().enumerate() {
            match expected {
                Some(intent) => assert_cue_holds(&after[index], intent, &label),
                None => assert_eq!(after[index], before[index], "{label}: Cue {}", index + 1),
            }
        }
        // Unknown future fields survive on both the rewritten and the untouched rows.
        assert_eq!(after[0]["future_cue"], json!({"keep": "nested"}), "{label}");
        assert_eq!(
            stored_row(
                &after[0]["changes"],
                "fixture_id",
                &fixture_a().0.to_string(),
                "color"
            )["future_change"],
            json!({"keep": 42}),
            "{label}"
        );
        assert_eq!(
            after[0]["group_changes"][0]["future_group_change"],
            json!({"keep": [true]}),
            "{label}"
        );
        assert_eq!(
            after[1]["changes"][0]["future_change"],
            json!({"keep": "unrelated"}),
            "{label}: the unrelated row stays first and keeps its unknown field"
        );
        let reopened = document(&rig);
        let object = reopened.object("cue_list", "tl627-cuelist").unwrap();
        assert_eq!(object.body(), result.outcome.projection.raw_body.as_ref());
        assert_eq!(object.body()["future"], json!({"keep": true}));
        assert_named_facts(|fixture, attribute| {
            stored_row(
                &after[case.changed_cue]["changes"],
                "fixture_id",
                &fixture.0.to_string(),
                &attribute.0,
            )["value"]
                .clone()
        });

        // The actually installed runtime holds the reopened Cuelist exactly.
        assert_eq!(ports.installs(), 1, "{label}");
        let installed = assert_installed_is_reopened_compile(&rig, &ports);
        let typed: CueList = serde_json::from_value(object.body().clone()).unwrap();
        let installed_list = installed
            .cue_lists
            .iter()
            .find(|list| list.id == layout.cue_list_id)
            .unwrap();
        for (installed_cue, typed_cue) in installed_list.cues.iter().zip(&typed.cues) {
            assert_eq!(installed_cue.changes, typed_cue.changes, "{label}");
            assert_eq!(
                installed_cue.group_changes, typed_cue.group_changes,
                "{label}"
            );
        }
        assert_eq!(rig.active_show.events().latest_sequence(), 1, "{label}");

        // Replay: the retained result, no second commit, event or install.
        let committed = observe(&rig, &ports);
        rig.clear_steps();
        let replay = apply(&rig, &ports, command, &label).unwrap();
        assert!(replay.replayed, "{label}");
        assert_eq!(replay.outcome, result.outcome, "{label}");
        assert_no_side_effects(&rig, &label);
        assert_eq!(observe(&rig, &ports), committed, "{label}: replay");

        // Identical intent after reopen is the documented Invalid no-op, not a recorded no-change.
        let identical = preview_and_command(
            &rig,
            &ports,
            layout.target(case.active),
            UpdateMode::Cue(case.mode),
            &format!("{label}-identical-preview"),
        );
        rig.clear_steps();
        let error = apply(&rig, &ports, identical, &format!("{label}-identical")).unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid, "{label}: {error:?}");
        assert_no_side_effects(&rig, &label);
        assert_eq!(observe(&rig, &ports), committed, "{label}: identical");
    }
}

#[test]
fn existing_in_current_cue_on_a_cue_without_those_events_is_the_documented_invalid_no_op() {
    let (rig, layout) = cue_rig();
    let ports = RetainingPorts::new(&rig);
    rig.set_active_contexts(vec![layout.context(1)]);
    let preview = preview(
        &rig,
        &ports,
        layout.target(1),
        UpdateMode::Cue(CueUpdateMode::ExistingInCurrentCue),
        "current-preview",
    )
    .unwrap();
    assert_eq!(preview.preview.changed_count(), 0);
    assert!(preview.preview.items.iter().all(|item| matches!(
        item.outcome,
        UpdateItemOutcome::Ignored {
            reason: UpdateIgnoreReason::NotInCurrentCue | UpdateIgnoreReason::NewAddress
        }
    )));
    let before = observe(&rig, &ports);
    rig.clear_steps();
    let error = apply(
        &rig,
        &ports,
        command_from_preview(&rig, layout.target(1), preview),
        "current",
    )
    .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_no_side_effects(&rig, "current");
    assert_eq!(observe(&rig, &ports), before);
}

// --- Stale context and rejected preparation -------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Stale {
    Object,
    Programmer,
    Show,
    LiveContext,
}

#[test]
fn stale_object_programmer_show_and_live_context_change_nothing() {
    for stale in [
        Stale::Object,
        Stale::Programmer,
        Stale::Show,
        Stale::LiveContext,
    ] {
        let (rig, layout) = cue_rig();
        let ports = RetainingPorts::new(&rig);
        rig.set_active_contexts(vec![layout.context(1)]);
        let mut command = preview_and_command(
            &rig,
            &ports,
            layout.target(1),
            UpdateMode::Cue(CueUpdateMode::ExistingOnly),
            "stale-preview",
        );
        match stale {
            Stale::Object => {
                command.expected_object_revision =
                    Some(command.expected_object_revision.unwrap() + 1);
            }
            Stale::Programmer => rig.registry.set(
                rig.session,
                fixture_a(),
                key(ProgrammingOwner::Color),
                color(warm_white_3200()),
            ),
            Stale::Show => {
                // Another object commits: the show moves while the Cuelist revision does not.
                ShowStore::open(&rig.ports.path)
                    .unwrap()
                    .put_object(
                        "group",
                        "tl627-other",
                        &group_body_with_id("tl627-other"),
                        0,
                    )
                    .unwrap();
            }
            Stale::LiveContext => rig.set_active_contexts(vec![layout.context(2)]),
        }
        let before = observe(&rig, &ports);
        rig.clear_steps();
        let error = apply(&rig, &ports, command, "stale").unwrap_err();
        assert_eq!(
            error.kind,
            ActionErrorKind::Conflict,
            "{stale:?}: {error:?}"
        );
        assert_no_side_effects(&rig, &format!("{stale:?}"));
        assert_eq!(observe(&rig, &ports), before, "{stale:?}");
    }

    for stale in [Stale::Object, Stale::Programmer, Stale::Show] {
        let rig = preset_rig(PresetFamily::Color);
        let ports = RetainingPorts::new(&rig);
        let mut command = preview_and_command(
            &rig,
            &ports,
            preset_target_of(PresetFamily::Color),
            UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
            "stale-preview",
        );
        match stale {
            Stale::Object => {
                command.expected_object_revision =
                    Some(command.expected_object_revision.unwrap() + 1);
            }
            Stale::Programmer => rig.registry.set(
                rig.session,
                dormant(),
                key(ProgrammingOwner::Color),
                color(magenta()),
            ),
            Stale::Show => {
                ShowStore::open(&rig.ports.path)
                    .unwrap()
                    .put_object(
                        "group",
                        "tl627-other",
                        &group_body_with_id("tl627-other"),
                        0,
                    )
                    .unwrap();
            }
            Stale::LiveContext => unreachable!(),
        }
        let before = observe(&rig, &ports);
        rig.clear_steps();
        let error = apply(&rig, &ports, command, "stale").unwrap_err();
        assert_eq!(
            error.kind,
            ActionErrorKind::Conflict,
            "Preset {stale:?}: {error:?}"
        );
        assert_no_side_effects(&rig, &format!("Preset {stale:?}"));
        assert_eq!(observe(&rig, &ports), before, "Preset {stale:?}");
    }
}

#[test]
fn rejected_candidate_preparation_changes_nothing() {
    // A stored independent Color component: adding a complete Color for the same fixture in the
    // same Cue passes the planner and is rejected by show-candidate compilation.
    let rig = TestRig::new();
    seed_group(&rig);
    let cue_list_id = CueListId(Uuid::from_u128(0x0627_c12e));
    let cue = cue_with(
        1.0,
        &Intent {
            fixtures: vec![(
                fixture_b(),
                AttributeKey(Arc::from("color.red")),
                AttributeValue::Normalized(0.25),
            )],
            groups: Vec::new(),
        },
    );
    let layout = CueLayout {
        cue_list_id,
        cues: vec![cue.clone()],
    };
    rig.seed(
        "cue_list",
        "tl627-component",
        cue_list_body(cue_list_id, cue),
    );
    program(
        &rig,
        &Intent {
            fixtures: vec![(fixture_b(), key(ProgrammingOwner::Color), direct(0x1000))],
            groups: Vec::new(),
        },
    );
    let ports = RetainingPorts::new(&rig);
    rig.set_active_contexts(vec![layout.context(0)]);
    let command = preview_and_command(
        &rig,
        &ports,
        layout.target(0),
        UpdateMode::Cue(CueUpdateMode::AddNew),
        "candidate-preview",
    );
    let before = observe(&rig, &ports);
    rig.clear_steps();
    let error = apply(&rig, &ports, command.clone(), "candidate").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid, "{error:?}");
    assert!(
        error.message.contains("invalid cue list"),
        "candidate compilation must reject it: {error:?}"
    );
    assert_no_side_effects(&rig, "candidate");
    assert_eq!(observe(&rig, &ports), before);
    // A rejected request is not retained as a replay.
    let error = apply(&rig, &ports, command, "candidate").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(observe(&rig, &ports), before);
}

#[test]
fn rejected_runtime_preparation_changes_nothing_and_a_retry_commits_once() {
    // Cue
    let (rig, layout) = cue_rig();
    let failing = RetainingPorts::failing(&rig);
    rig.set_active_contexts(vec![layout.context(1)]);
    let command = preview_and_command(
        &rig,
        &failing,
        layout.target(1),
        UpdateMode::Cue(CueUpdateMode::ExistingOnly),
        "runtime-preview",
    );
    let before = observe(&rig, &failing);
    rig.clear_steps();
    let error = apply(&rig, &failing, command.clone(), "runtime").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Unavailable, "{error:?}");
    assert!(rig.steps().contains(&"prepare"));
    for step in ["backup", "commit", "install", "reconcile"] {
        assert!(!rig.steps().contains(&step), "{step}");
    }
    assert_eq!(observe(&rig, &failing), before);

    // The failure was not retained: the same request now commits exactly once.
    let healthy = RetainingPorts::new(&rig);
    rig.clear_steps();
    let result = apply(&rig, &healthy, command, "runtime").unwrap();
    assert!(!result.replayed);
    assert_one_commit_lifecycle(&rig, "retry");
    assert_cue_holds(&stored_cues(&rig)[0], &new_intent(), "retry");
    assert_eq!(rig.active_show.events().latest_sequence(), 1);
    assert_installed_is_reopened_compile(&rig, &healthy);

    // Preset
    let rig = preset_rig(PresetFamily::Color);
    let failing = RetainingPorts::failing(&rig);
    let command = preview_and_command(
        &rig,
        &failing,
        preset_target_of(PresetFamily::Color),
        UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
        "runtime-preview",
    );
    let before = observe(&rig, &failing);
    rig.clear_steps();
    let error = apply(&rig, &failing, command, "runtime").unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Unavailable, "{error:?}");
    for step in ["backup", "commit", "install", "reconcile"] {
        assert!(!rig.steps().contains(&step), "Preset {step}");
    }
    assert_eq!(observe(&rig, &failing), before);
}

#[path = "universal_preset_update_tests.rs"]
mod universal_preset_update;
