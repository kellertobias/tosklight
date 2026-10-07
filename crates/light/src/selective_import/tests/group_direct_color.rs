//! TL-607: tagged Direct Color stored in an actual `GroupDefinition::programming`, through the
//! real selective-import preview/apply, the committed SQLite target and the show compiler.
//!
//! `programming_native` keeps its Direct members in `Preset.group_values` and the TL-570 Group
//! matrix in `semantic_intent` stores only Semantic Color. Here the live Group itself owns one
//! `GroupFamilyAssignment` whose Direct template, Direct member exceptions (one with a full-width
//! u32 spread), a Semantic exception with a discrete wheel-slot constraint and a dormant member
//! (an exception for a patched fixture outside the current membership) must survive profile,
//! fixture and Group collisions. Only identities may change: every recipe, raw word, spread,
//! portable estimate (explicit zero UV versus unknown UV) and limitation stays byte-exact, and
//! no destination native recipe is substituted. Nothing here claims fitted output.
use super::programming_native::{NativeFixture, conflicting_revision, native_fixture, profile_key};
use super::support::*;
use crate::prepare_show_candidate;
use crate::selective_import::*;
use light_core::programming::{
    ColorProgram, DirectFallback, DirectReplay, GroupFamilyAssignment, UvFallback,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, NativeColorIdentity};
use light_engine::EngineSnapshot;
use light_fixture::{ChannelFunction, ChannelFunctionBehavior, FixtureProfile};
use light_programmer::{FrozenGroup, GroupDefinition, GroupFixtureSource, resolve_group};
use light_show::FixtureProfileRevision;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

const GROUP: &str = "direct-front";
const PARENT: &str = "direct-parent";
const UNRELATED: &str = "direct-unrelated";
const KEEP_BLOCKER: &str = "destination profile changes the pinned native Color model; duplicate the source profile to preserve its recipe";

/// Discrete colour-wheel slots on the 8-bit wheel channel: (label, from, to).
const SLOTS: [(&str, u32, u32); 4] = [
    ("open", 0, 63),
    ("red", 64, 127),
    ("blue", 128, 191),
    ("green", 192, 255),
];

/// `native_fixture` with its wheel turned into discrete Indexed slots. The UV control stays a
/// continuous 32-bit function, so the same head carries both discrete and full-width values.
struct WheelFixture {
    native: NativeFixture,
    wheel_channel: Uuid,
    /// Slot function ids in `SLOTS` order.
    slots: [Uuid; 4],
    uv_channel: Uuid,
    uv_function: Uuid,
}

impl WheelFixture {
    fn new() -> Self {
        let base = native_fixture();
        let mut profile: FixtureProfile =
            serde_json::from_value(base.revision.profile().clone()).unwrap();
        let mode = &mut profile.modes[0];
        let (mode_id, head_id) = (mode.id, mode.heads[0].id);
        let wheel = mode
            .channels
            .iter_mut()
            .find(|channel| channel.fixture_attribute.0.as_ref() == "color.wheel.1")
            .unwrap();
        wheel.functions = SLOTS
            .iter()
            .map(|(label, from, to)| ChannelFunction {
                id: Uuid::new_v4(),
                name: (*label).into(),
                dmx_from: *from,
                dmx_to: *to,
                attribute: AttributeKey("color.wheel.1".into()),
                priority: 0,
                angular_motion: None,
                physical_mapping: None,
                behavior: ChannelFunctionBehavior::Indexed {
                    semantic_id: format!("wheel.{label}"),
                    label: (*label).into(),
                    raw_value: *from,
                },
            })
            .collect();
        let wheel_channel = wheel.id;
        let slots = std::array::from_fn(|index| wheel.functions[index].id);
        let uv = mode
            .channels
            .iter()
            .find(|channel| channel.fixture_attribute.0.as_ref() == "color.uv")
            .unwrap();
        let (uv_channel, uv_function) = (uv.id, uv.functions[0].id);
        let identity = profile.native_color_identity(mode_id, head_id).unwrap();
        let mut body = serde_json::to_value(&profile).unwrap();
        // A raw extension outside the typed profile: changing it alone changes the immutable
        // revision digest but not the typed model, which is what makes a compatible Keep.
        body["future_profile"] = json!({"retained": true, "variant": "source"});
        let revision = FixtureProfileRevision::from_profile(body).unwrap();
        let mut fixture = Self {
            native: NativeFixture {
                revision,
                identity,
                fixture_id: base.fixture_id,
                fixture_body: base.fixture_body,
                channels: Value::Null,
            },
            wheel_channel,
            slots,
            uv_channel,
            uv_function,
        };
        fixture.native.channels = fixture.recipe(132, u32::MAX - 1);
        fixture
    }

    /// Complete recipe for the single optical path: one wheel slot plus the 32-bit UV word.
    fn recipe(&self, wheel_raw: u32, uv_raw: u32) -> Value {
        let slot = SLOTS
            .iter()
            .position(|(_, from, to)| (*from..=*to).contains(&wheel_raw))
            .unwrap();
        json!([
            {"channel_id": self.wheel_channel, "function_id": self.slots[slot], "raw": wheel_raw},
            {"channel_id": self.uv_channel, "function_id": self.uv_function, "raw": uv_raw}
        ])
    }

    fn wheel_value(&self, raw: u32) -> Value {
        self.recipe(raw, 0)[0].clone()
    }

    /// The same native model at the same key with only a raw extension changed.
    fn compatible_revision(&self) -> FixtureProfileRevision {
        let mut body = self.native.revision.profile().clone();
        body["future_profile"] = json!({"retained": true, "variant": "destination"});
        FixtureProfileRevision::from_profile(body).unwrap()
    }

    /// Another patched fixture of the same profile, mode and head layout.
    fn fixture_body(&self, base: u128, number: u32) -> (FixtureId, Value) {
        let id = FixtureId(Uuid::from_u128(base));
        let mut body = self.native.fixture_body.clone();
        body["fixture_id"] = json!(id.0);
        body["fixture_number"] = json!(number);
        body["name"] = json!(format!("Direct {number}"));
        body["logical_heads"][0]["fixture_id"] = json!(Uuid::from_u128(base + 1));
        body["multipatch"][0]["id"] = json!(Uuid::from_u128(base + 2));
        (id, body)
    }
}

fn direct(source: &NativeColorIdentity, channels: Value, spreads: Value, portable: Value) -> Value {
    json!({"kind": "color_program", "value": {"kind": "direct",
        "recipe": {"source": source, "channels": channels, "spreads": spreads},
        "portable": portable}})
}

/// The authored Group: B is governed by the template, A and D carry exceptions, C is dormant.
struct GroupShow {
    wheel: WheelFixture,
    /// A, B, C, D in that order.
    fixtures: [(FixtureId, Value); 4],
    group: Value,
}

impl GroupShow {
    fn a(&self) -> FixtureId {
        self.fixtures[0].0
    }
    fn b(&self) -> FixtureId {
        self.fixtures[1].0
    }
    fn c(&self) -> FixtureId {
        self.fixtures[2].0
    }
    fn d(&self) -> FixtureId {
        self.fixtures[3].0
    }
    fn membership(&self) -> Vec<FixtureId> {
        // Deliberately not identity order, so a reordering rewrite is visible.
        vec![self.b(), self.a(), self.d()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Membership {
    /// Legacy `fixtures` authority without a canonical source.
    Live,
    /// Frozen Group with an explicit canonical source beside its frozen provenance.
    FrozenCanonical,
}

fn group_programming(wheel: &WheelFixture, a: FixtureId, c: FixtureId, d: FixtureId) -> Value {
    let source = &wheel.native.identity;
    let uv = json!({"channel_id": wheel.uv_channel, "function_id": wheel.uv_function});
    // Template: blue slot, 0xFFFF_FFFE UV, appearance and UV both unknown.
    let template = direct(
        source,
        wheel.recipe(132, u32::MAX - 1),
        json!([]),
        json!({"model_revision": 1, "visible": null, "uv": null, "quality": "unknown",
            "limitations": ["tl607 template: UV response unknown"]}),
    );
    // Exception A: red slot, raw-zero UV with a full-width spread, known black, KNOWN zero UV.
    let member_a = direct(
        source,
        wheel.recipe(70, 0),
        json!([{"binding": uv, "points": [0, u32::MAX, 0x8000_0001_u32]}]),
        json!({"model_revision": 1,
            "visible": {"xyz": {"x": 0.0, "y": 0.0, "z": 0.0}, "relative_output": 1.0},
            "uv": {"amount": 0.0, "quality": "measured"}, "quality": "measured",
            "limitations": []}),
    );
    // Dormant C: green slot top edge and the maximum 32-bit word, reduced visible output.
    let member_c = direct(
        source,
        wheel.recipe(255, u32::MAX),
        json!([]),
        json!({"model_revision": 1,
            "visible": {"xyz": {"x": 0.2, "y": 0.1, "z": 0.05}, "relative_output": 0.35},
            "uv": {"amount": 0.7, "quality": "estimated"}, "quality": "estimated",
            "limitations": ["tl607 dormant estimate"]}),
    );
    // Exception D: Semantic Color pinned to a discrete wheel slot of the same source path.
    let mut intent = serde_json::to_value(light_core::programming::ColorIntent::default()).unwrap();
    intent["wheel_constraints"] = json!([{"source": source, "value": wheel.wheel_value(191)}]);
    let member_d =
        json!({"kind": "color_program", "value": {"kind": "semantic", "intent": intent}});
    let mut members = serde_json::Map::new();
    members.insert(a.0.to_string(), member_a);
    members.insert(c.0.to_string(), member_c);
    members.insert(d.0.to_string(), member_d);
    json!({"color": {"kind": "group_family",
        "value": {"owner": "color", "template": template, "members": members}}})
}

/// Seeds the source and, for every source identity, a colliding destination object.
fn seed(
    rig: &TestRig,
    membership: Membership,
    destination_profile: impl Fn(&WheelFixture) -> FixtureProfileRevision,
) -> GroupShow {
    let wheel = WheelFixture::new();
    let fixtures = [
        (wheel.native.fixture_id, wheel.native.fixture_body.clone()),
        wheel.fixture_body(0x607_0b00, 802),
        wheel.fixture_body(0x607_0c00, 803),
        wheel.fixture_body(0x607_0d00, 804),
    ];
    rig.source_profile(&wheel.native.revision);
    rig.target_profile(&destination_profile(&wheel));
    for (id, body) in &fixtures {
        rig.source_object("patched_fixture", &id.0.to_string(), body.clone());
        rig.target_object("patched_fixture", &id.0.to_string(), body.clone());
    }
    let mut show = GroupShow {
        wheel,
        fixtures,
        group: Value::Null,
    };
    let mut group = GroupDefinition {
        id: GROUP.into(),
        name: "Direct front".into(),
        fixtures: show.membership(),
        ..Default::default()
    };
    if membership == Membership::FrozenCanonical {
        group.source = Some(GroupFixtureSource::Explicit {
            fixture_ids: show.membership(),
        });
        group.frozen_from = Some(FrozenGroup {
            source_group_id: PARENT.into(),
            source_revision: 4,
            captured_at: chrono::DateTime::from_timestamp(1_760_000_000, 0).unwrap(),
        });
        let parent = GroupDefinition {
            id: PARENT.into(),
            name: "Direct parent".into(),
            fixtures: vec![show.a(), show.b(), show.c(), show.d()],
            ..Default::default()
        };
        rig.source_object("group", PARENT, serde_json::to_value(parent).unwrap());
    }
    let mut body = serde_json::to_value(group).unwrap();
    body["programming"] = group_programming(&show.wheel, show.a(), show.c(), show.d());
    // The authored body must already be a valid typed Group before import.
    let typed: GroupDefinition = serde_json::from_value(body.clone()).unwrap();
    let AttributeValue::GroupFamily(family) = &typed.programming[&AttributeKey("color".into())]
    else {
        panic!("Group family expected")
    };
    family.validate().unwrap();
    rig.source_object("group", GROUP, body.clone());
    show.group = body;
    for (id, name) in [(GROUP, "Destination front"), (PARENT, "Destination parent")] {
        rig.target_object("group", id, json!({"id": id, "name": name, "fixtures": []}));
    }
    show
}

#[derive(Clone, Copy, Debug)]
enum Collision {
    /// Explicit Duplicate resolution for every colliding Group and fixture.
    Duplicate,
    /// AddToEnd allocates every object in the closure under a new identity.
    AddToEnd,
}

fn request(
    rig: &TestRig,
    show: &GroupShow,
    membership: Membership,
    collision: Collision,
    profile: ImportProfileConflictResolution,
) -> SelectiveShowImportRequest {
    let mut request = rig
        .request("group", GROUP)
        .resolve_profile(profile_key(&show.wheel.native), profile);
    match collision {
        Collision::AddToEnd => request = request.with_mode(ImportLoadMode::AddToEnd),
        Collision::Duplicate => {
            let mut keys = vec![key("group", GROUP)];
            if membership == Membership::FrozenCanonical {
                keys.push(key("group", PARENT));
            }
            keys.extend(
                show.fixtures
                    .iter()
                    .map(|(id, _)| key("patched_fixture", &id.0.to_string())),
            );
            for key in keys {
                request = request.resolve(key, ImportConflictResolution::Duplicate);
            }
        }
    }
    request
}

fn destination(preview: &SelectiveShowImportPreview, kind: &str, id: &str) -> String {
    preview
        .objects
        .iter()
        .find(|entry| entry.source == key(kind, id))
        .unwrap_or_else(|| panic!("{kind}/{id} is not planned: {:?}", preview.objects))
        .destination
        .id()
        .to_owned()
}

/// The show compiler's patch migration (`show_compiler::migrations::patch`) rides any active
/// show transaction and materializes default split patches on existing fixtures. That rider is
/// existing compatibility behavior, not part of the import, so it is excluded from comparison.
fn without_split_patches(body: &Value) -> Value {
    let mut body = body.clone();
    body.as_object_mut().unwrap().remove("split_patches");
    for instance in body["multipatch"].as_array_mut().into_iter().flatten() {
        instance.as_object_mut().unwrap().remove("split_patches");
    }
    body
}

/// Every pinned native identity object below `value`, by JSON pointer.
fn native_sources(value: &Value, pointer: String, found: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("profile_digest") {
                found.insert(pointer, value.clone());
                return;
            }
            for (key, child) in map {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                native_sources(child, format!("{pointer}/{escaped}"), found);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                native_sources(child, format!("{pointer}/{index}"), found);
            }
        }
        _ => {}
    }
}

/// The source programming with ONLY identities rewritten: member keys through `fixtures`, and
/// every pinned native source replaced by `identity`. Everything else is the authored JSON.
fn expected_programming(
    source: &Value,
    fixtures: &BTreeMap<Uuid, Uuid>,
    identity: &NativeColorIdentity,
) -> Value {
    let mut expected = source.clone();
    let mut found = BTreeMap::new();
    native_sources(source, String::new(), &mut found);
    assert_eq!(
        found.len(),
        4,
        "template, two Direct exceptions and one wheel constraint"
    );
    for pointer in found.keys() {
        *expected.pointer_mut(pointer).unwrap() = serde_json::to_value(identity).unwrap();
    }
    let members = expected["color"]["value"]["members"]
        .as_object_mut()
        .unwrap();
    let renamed = std::mem::take(members)
        .into_iter()
        .map(|(key, value)| {
            let id = Uuid::parse_str(&key).unwrap();
            (fixtures[&id].to_string(), value)
        })
        .collect();
    *members = renamed;
    expected
}

fn color_family(group: &GroupDefinition) -> &GroupFamilyAssignment {
    let AttributeValue::GroupFamily(family) = &group.programming[&AttributeKey("color".into())]
    else {
        panic!("stored Group Color is not a Group family")
    };
    family
}

fn program(value: &AttributeValue) -> &ColorProgram {
    let AttributeValue::ColorProgram(program) = value else {
        panic!("Color program expected")
    };
    program
}

fn compiled_group<'a>(snapshot: &'a EngineSnapshot, id: &str) -> &'a GroupDefinition {
    snapshot
        .groups
        .iter()
        .find(|group| group.id == id)
        .unwrap_or_else(|| panic!("Group {id} is not compiled"))
}

/// The imported Direct values replay exactly on the identity they are pinned to, and fall back
/// to their own saved portable knowledge (never a destination recipe) on a foreign head.
fn assert_replay(
    snapshot: &EngineSnapshot,
    family: &GroupFamilyAssignment,
    a: Uuid,
    c: Uuid,
    pinned: &NativeColorIdentity,
    foreign: Option<&NativeColorIdentity>,
) {
    let catalog = &snapshot.native_color_sources;
    catalog
        .resolve(pinned)
        .expect("pinned native source resolves in the compiled catalogue");
    for value in [&family.template, &family.members[&a], &family.members[&c]] {
        let program = program(value);
        let ColorProgram::Direct { recipe, portable } = program else {
            panic!("Direct expected")
        };
        assert_eq!(
            catalog.plan_direct_replay(program, Some(pinned)).unwrap(),
            DirectReplay::Exact {
                recipe: recipe.clone()
            },
            "pinned replay must keep the exact recorded recipe and spreads"
        );
        let DirectReplay::Fallback { fallback, .. } =
            catalog.plan_direct_replay(program, foreign).unwrap()
        else {
            panic!("a foreign destination must fall back")
        };
        assert_eq!(fallback, DirectFallback::from_portable(portable));
    }
    let uv = |value: &AttributeValue| {
        let ColorProgram::Direct { portable, .. } = program(value) else {
            unreachable!()
        };
        DirectFallback::from_portable(portable).uv
    };
    assert_eq!(
        uv(&family.template),
        UvFallback::ParkOff,
        "unknown UV parks off"
    );
    let UvFallback::Apply(known) = uv(&family.members[&a]) else {
        panic!("explicit zero UV must stay known")
    };
    assert_eq!(known.amount, 0.0);
}

/// AC1/AC2: with the profile, all four fixtures and the Group (plus the frozen parent) already
/// occupied, Duplicate and AddToEnd both remap every nested identity consistently while the
/// recipes, raw words, spreads and portable knowledge stay exact.
#[test]
fn duplicate_remaps_group_stored_direct_template_exceptions_and_dormant_members() {
    for membership in [Membership::Live, Membership::FrozenCanonical] {
        for collision in [Collision::Duplicate, Collision::AddToEnd] {
            let case = format!("{membership:?}/{collision:?}");
            let rig = TestRig::new();
            let show = seed(&rig, membership, |wheel| {
                conflicting_revision(&wheel.native)
            });
            let incompatible = conflicting_revision(&show.wheel.native);
            assert_duplicate_case(&rig, &show, membership, collision, &incompatible, &case);
        }
    }
}

fn assert_duplicate_case(
    rig: &TestRig,
    show: &GroupShow,
    membership: Membership,
    collision: Collision,
    incompatible: &FixtureProfileRevision,
    case: &str,
) {
    let source = &show.wheel.native;
    let preview = rig.preview(request(
        rig,
        show,
        membership,
        collision,
        ImportProfileConflictResolution::Duplicate,
    ));
    assert!(preview.can_apply(), "{case}: {:?}", preview.blockers);
    let profile = preview
        .profiles
        .iter()
        .find(|entry| entry.source == profile_key(source))
        .unwrap();
    assert!(
        matches!(profile.action, ImportProfileAction::Duplicate { .. }),
        "{case}: {:?}",
        profile.action
    );
    let duplicate_key = profile.destination;
    assert_ne!(duplicate_key.profile_id, profile_key(source).profile_id);
    for (id, _) in &show.fixtures {
        let dependency = key("patched_fixture", &id.0.to_string());
        assert!(
            preview
                .dependencies
                .iter()
                .any(|d| d.dependency == dependency),
            "{case}: {dependency:?} (member key or membership) missing from the closure"
        );
    }
    let before = rig.target_document();
    rig.apply(&preview).unwrap();

    let fixtures = show
        .fixtures
        .iter()
        .map(|(id, _)| {
            let mapped = destination(&preview, "patched_fixture", &id.0.to_string());
            (id.0, Uuid::parse_str(&mapped).unwrap())
        })
        .collect::<BTreeMap<_, _>>();
    for (from, to) in &fixtures {
        assert_ne!(from, to, "{case}: a colliding fixture kept its identity");
    }
    let group_id = destination(&preview, "group", GROUP);
    assert_ne!(group_id, GROUP, "{case}");

    let target = rig.target_document();
    // The duplicated profile is the complete source model under a new identity only.
    let duplicate = target
        .fixture_profile_revision(duplicate_key.profile_id, duplicate_key.revision)
        .expect("duplicated profile revision");
    let duplicate_profile: FixtureProfile =
        serde_json::from_value(duplicate.profile().clone()).unwrap();
    let mut duplicate_body = serde_json::to_value(&duplicate_profile).unwrap();
    let source_profile: FixtureProfile =
        serde_json::from_value(source.revision.profile().clone()).unwrap();
    duplicate_body["id"] = json!(source_profile.id.0);
    assert_eq!(
        duplicate_body,
        serde_json::to_value(&source_profile).unwrap(),
        "{case}: duplicate keeps every mode/head/path/channel/function identity"
    );
    let expected = duplicate_profile
        .native_color_identity(source.identity.mode_id, source.identity.head_id)
        .unwrap();
    assert_ne!(expected.profile_id, source.identity.profile_id);
    assert_ne!(expected.profile_digest, source.identity.profile_digest);
    assert_eq!(
        (
            expected.mode_id,
            expected.head_id,
            expected.path_id,
            expected.model_revision,
            &expected.native_layout_signature
        ),
        (
            source.identity.mode_id,
            source.identity.head_id,
            source.identity.path_id,
            source.identity.model_revision,
            &source.identity.native_layout_signature
        ),
        "{case}: only the profile identity and digest change"
    );
    assert_eq!(
        target
            .fixture_profile_revision(profile_key(source).profile_id, profile_key(source).revision)
            .unwrap()
            .digest(),
        incompatible.digest(),
        "{case}: the colliding destination profile is untouched"
    );

    // Destination objects under the source identities are untouched.
    assert_eq!(
        target.object("group", GROUP).unwrap().body()["name"],
        json!("Destination front"),
        "{case}"
    );
    for (id, body) in &show.fixtures {
        let id_text = id.0.to_string();
        assert_eq!(
            without_split_patches(target.object("patched_fixture", &id_text).unwrap().body()),
            without_split_patches(before.object("patched_fixture", &id_text).unwrap().body()),
            "{case}: colliding fixture changed beyond the split-patch migration rider"
        );
        let imported = target
            .object("patched_fixture", &fixtures[&id.0].to_string())
            .unwrap()
            .body();
        assert_eq!(
            imported["profile_id"],
            json!(duplicate_key.profile_id.0),
            "{case}: imported fixture must bind the duplicated profile"
        );
        assert_eq!(imported["mode_id"], body["mode_id"], "{case}");
    }

    let body = target.object("group", &group_id).unwrap().body();
    // Byte-level: the stored programming is the authored programming with identities rewritten.
    let expected_json = expected_programming(&show.group["programming"], &fixtures, &expected);
    assert_eq!(
        body["programming"], expected_json,
        "{case}: programming payload"
    );
    let mut found = BTreeMap::new();
    native_sources(&body["programming"], String::new(), &mut found);
    assert_eq!(found.len(), 4, "{case}");
    for (pointer, identity) in &found {
        assert_eq!(
            identity,
            &serde_json::to_value(&expected).unwrap(),
            "{case}: {pointer} kept a stale native identity"
        );
    }
    let text = body["programming"].to_string();
    for stale in show.fixtures.iter().map(|(id, _)| id.0.to_string()).chain([
        source.identity.profile_id.to_string(),
        source.identity.profile_digest.clone(),
    ]) {
        assert!(!text.contains(&stale), "{case}: stale identity {stale}");
    }
    for (pointer, raw) in [
        (
            "/color/value/template/value/recipe/channels",
            show.wheel.recipe(132, u32::MAX - 1),
        ),
        (
            &*format!(
                "/color/value/members/{}/value/recipe/channels",
                fixtures[&show.a().0]
            ),
            show.wheel.recipe(70, 0),
        ),
        (
            &*format!(
                "/color/value/members/{}/value/recipe/channels",
                fixtures[&show.c().0]
            ),
            show.wheel.recipe(255, u32::MAX),
        ),
    ] {
        assert_eq!(
            serde_json::to_string(body["programming"].pointer(pointer).unwrap()).unwrap(),
            serde_json::to_string(&raw).unwrap(),
            "{case}: {pointer} recipe bytes"
        );
    }
    assert_eq!(
        body["programming"]
            .pointer(&format!(
                "/color/value/members/{}/value/recipe/spreads/0/points",
                fixtures[&show.a().0]
            ))
            .unwrap(),
        &json!([0, u32::MAX, 0x8000_0001_u32]),
        "{case}: full-width spread"
    );

    // Typed decode: membership order, canonical/frozen source and the Group family.
    let imported: GroupDefinition = serde_json::from_value(body.clone()).unwrap();
    let ordered = show
        .membership()
        .iter()
        .map(|id| FixtureId(fixtures[&id.0]))
        .collect::<Vec<_>>();
    assert_eq!(
        imported.source,
        Some(GroupFixtureSource::Explicit {
            fixture_ids: ordered.clone()
        }),
        "{case}"
    );
    match membership {
        Membership::Live => {
            assert_eq!(imported.fixtures, ordered, "{case}");
            assert!(imported.frozen_from.is_none(), "{case}");
        }
        Membership::FrozenCanonical => {
            // Existing policy: the canonical source wins; the legacy projection beside it is
            // retained unscanned (see `semantic_intent::live_group_stored_programming_*`).
            assert_eq!(imported.fixtures, show.membership(), "{case}");
            let frozen = imported.frozen_from.as_ref().unwrap();
            let parent = destination(&preview, "group", PARENT);
            assert_ne!(parent, PARENT, "{case}");
            assert_eq!(frozen.source_group_id, parent, "{case}");
            assert_eq!(frozen.source_revision, 4, "{case}");
            assert_eq!(
                frozen.captured_at,
                chrono::DateTime::from_timestamp(1_760_000_000, 0).unwrap()
            );
        }
    }
    let expected_typed: HashMap<AttributeKey, AttributeValue> =
        serde_json::from_value(expected_json).unwrap();
    assert_eq!(imported.programming, expected_typed, "{case}");
    let family = color_family(&imported);
    let (a, b, c, d) = (
        FixtureId(fixtures[&show.a().0]),
        FixtureId(fixtures[&show.b().0]),
        FixtureId(fixtures[&show.c().0]),
        FixtureId(fixtures[&show.d().0]),
    );
    assert_eq!(
        family.members.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([a.0, c.0, d.0]),
        "{case}: member keys"
    );
    // Template fallback for B, exceptions for A/D, dormant C retained outside membership.
    assert_eq!(family.for_member(b), &family.template, "{case}");
    assert_ne!(family.for_member(a), &family.template, "{case}");
    assert!(
        !ordered.contains(&c),
        "{case}: dormant member joined the Group"
    );
    let ColorProgram::Semantic { intent } = program(family.for_member(d)) else {
        panic!("{case}: Semantic wheel exception changed kind")
    };
    assert_eq!(intent.wheel_constraints.len(), 1);
    assert_eq!(intent.wheel_constraints[0].source, expected);
    assert_eq!(
        serde_json::to_value(&intent.wheel_constraints[0].value).unwrap(),
        show.wheel.wheel_value(191),
        "{case}: discrete wheel slot"
    );
    let source_family: GroupDefinition = serde_json::from_value(show.group.clone()).unwrap();
    let source_family = color_family(&source_family);
    for (value, authored) in [
        (&family.template, &source_family.template),
        (&family.members[&a.0], &source_family.members[&show.a().0]),
        (&family.members[&c.0], &source_family.members[&show.c().0]),
    ] {
        let (
            ColorProgram::Direct { recipe, portable },
            ColorProgram::Direct {
                recipe: authored_recipe,
                portable: authored_portable,
            },
        ) = (program(value), program(authored))
        else {
            panic!("{case}: Direct changed kind")
        };
        // Still the SOURCE recipe pinned to the duplicated source model, not a destination one.
        assert_eq!(recipe.source, expected, "{case}");
        assert_eq!(recipe.channels, authored_recipe.channels, "{case}");
        assert_eq!(recipe.spreads, authored_recipe.spreads, "{case}");
        assert_eq!(portable, authored_portable, "{case}");
    }

    // The installed candidate and a fresh compile of the reopened target agree.
    let installed = rig.ports.installed.lock();
    let snapshot = installed.as_ref().expect("apply installs its candidate");
    assert_eq!(
        compiled_group(snapshot, &group_id).programming,
        imported.programming
    );
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let resolved = resolve_group(&group_id, &groups).unwrap();
    assert_eq!(resolved, ordered, "{case}: compiled membership");
    drop(installed);
    let reopened = rig.target_document();
    let (_, compiled) = prepare_show_candidate(&reopened, reopened.transaction())
        .unwrap()
        .into_parts();
    assert_eq!(
        compiled_group(&compiled, &group_id).programming,
        imported.programming
    );
    let foreign: FixtureProfile = serde_json::from_value(incompatible.profile().clone()).unwrap();
    let foreign = foreign
        .native_color_identity(source.identity.mode_id, source.identity.head_id)
        .unwrap();
    assert_replay(&compiled, family, a.0, c.0, &expected, Some(&foreign));
    assert_replay(&compiled, family, a.0, c.0, &expected, None);
}

/// AC3 (first half): a compatible destination profile at the same key is kept, not copied; the
/// imported values pin that destination's own identity and replay exactly on it.
#[test]
fn compatible_keep_binds_group_stored_direct_to_the_actual_destination_profile() {
    for membership in [Membership::Live, Membership::FrozenCanonical] {
        let case = format!("{membership:?}");
        let rig = TestRig::new();
        let show = seed(&rig, membership, WheelFixture::compatible_revision);
        let source = &show.wheel.native;
        let kept = show.wheel.compatible_revision();
        assert_ne!(
            kept.digest(),
            source.revision.digest(),
            "{case}: must conflict"
        );
        let preview = rig.preview(request(
            &rig,
            &show,
            membership,
            Collision::Duplicate,
            ImportProfileConflictResolution::KeepDestination,
        ));
        assert!(preview.can_apply(), "{case}: {:?}", preview.blockers);
        let profile = preview
            .profiles
            .iter()
            .find(|entry| entry.source == profile_key(source))
            .unwrap();
        assert_eq!(
            profile.action,
            ImportProfileAction::KeepDestination,
            "{case}"
        );
        assert_eq!(profile.destination, profile_key(source), "{case}");
        let result = rig.apply(&preview).unwrap();
        assert!(
            result.change.profiles.is_empty(),
            "{case}: no profile copied"
        );

        let target = rig.target_document();
        let destination_profile = target
            .fixture_profile_revision(profile_key(source).profile_id, profile_key(source).revision)
            .unwrap();
        assert_eq!(destination_profile.digest(), kept.digest(), "{case}");
        assert_eq!(
            destination_profile.profile()["future_profile"]["variant"],
            "destination",
            "{case}: the destination revision stays authoritative"
        );
        let typed: FixtureProfile =
            serde_json::from_value(destination_profile.profile().clone()).unwrap();
        let actual = typed
            .native_color_identity(source.identity.mode_id, source.identity.head_id)
            .unwrap();
        // The typed model is identical, so the destination's own identity equals the source's.
        assert_eq!(actual, source.identity, "{case}");

        let fixtures = show
            .fixtures
            .iter()
            .map(|(id, _)| {
                let mapped = destination(&preview, "patched_fixture", &id.0.to_string());
                (id.0, Uuid::parse_str(&mapped).unwrap())
            })
            .collect::<BTreeMap<_, _>>();
        for (_, to) in &fixtures {
            let imported = target
                .object("patched_fixture", &to.to_string())
                .unwrap()
                .body();
            assert_eq!(imported["profile_id"], json!(source.identity.profile_id));
        }
        let group_id = destination(&preview, "group", GROUP);
        assert_ne!(group_id, GROUP);
        let body = target.object("group", &group_id).unwrap().body();
        assert_eq!(
            body["programming"],
            expected_programming(&show.group["programming"], &fixtures, &actual),
            "{case}"
        );
        let imported: GroupDefinition = serde_json::from_value(body.clone()).unwrap();
        let (_, compiled) = prepare_show_candidate(&target, target.transaction())
            .unwrap()
            .into_parts();
        assert_eq!(
            compiled_group(&compiled, &group_id).programming,
            imported.programming
        );
        let family = color_family(&imported);
        assert_replay(
            &compiled,
            family,
            fixtures[&show.a().0],
            fixtures[&show.c().0],
            &actual,
            None,
        );
    }
}

/// AC3 (second half): Keep onto a destination with a different native model refuses the whole
/// import. The destination show, its revision, the installed runtime and the import undo
/// history are all exactly as they were, including an earlier successful import's inverse.
#[test]
fn incompatible_keep_refuses_group_stored_direct_atomically() {
    for membership in [Membership::Live, Membership::FrozenCanonical] {
        let case = format!("{membership:?}");
        let rig = TestRig::new();
        let show = seed(&rig, membership, |wheel| {
            conflicting_revision(&wheel.native)
        });
        let source = &show.wheel.native;

        // One earlier successful import gives the history and runtime a known state.
        rig.source_object(
            "group",
            UNRELATED,
            json!({"id": UNRELATED, "name": "Unrelated", "fixtures": []}),
        );
        let earlier = rig.preview(rig.request("group", UNRELATED));
        assert!(earlier.can_apply(), "{case}: {:?}", earlier.blockers);
        rig.apply(&earlier).unwrap();
        let history = rig.ports.recorded_undo.lock().clone();
        assert_eq!(history.len(), 1, "{case}");
        let installed_revision = rig.ports.installed.lock().as_ref().map(|s| s.revision);

        let before = rig.target_document();
        let preview = rig.preview(request(
            &rig,
            &show,
            membership,
            Collision::Duplicate,
            ImportProfileConflictResolution::KeepDestination,
        ));
        rig.clear_steps();
        let refused = rig.apply(&preview);

        // Atomicity first, so any write would be reported before the refusal reason.
        assert_eq!(
            rig.target_document(),
            before,
            "{case}: destination show changed"
        );
        assert_eq!(
            *rig.ports.recorded_undo.lock(),
            history,
            "{case}: import undo history changed"
        );
        assert_eq!(
            rig.ports.installed.lock().as_ref().map(|s| s.revision),
            installed_revision,
            "{case}: runtime was reinstalled"
        );
        for step in ["commit", "prepare", "install", "reconcile"] {
            assert!(!rig.steps().contains(&step), "{case}: {step} ran");
        }
        assert_eq!(
            rig.preview(request(
                &rig,
                &show,
                membership,
                Collision::Duplicate,
                ImportProfileConflictResolution::KeepDestination,
            ))
            .target_revision,
            preview.target_revision,
            "{case}: destination revision moved"
        );
        assert!(refused.is_err(), "{case}: incompatible Keep was applied");
        assert!(!preview.can_apply(), "{case}");
        assert!(
            preview.blockers.contains(&ImportBlocker::ReferenceRewrite {
                owner: key("group", GROUP),
                message: KEEP_BLOCKER.into(),
            }),
            "{case}: {:?}",
            preview.blockers
        );
        assert_eq!(
            before
                .fixture_profile_revision(
                    profile_key(source).profile_id,
                    profile_key(source).revision
                )
                .unwrap()
                .digest(),
            conflicting_revision(source).digest()
        );
    }
}
