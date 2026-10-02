//! TL-574: recording a semantic Preset publishes every Dynamic whose retained source it rewrote.
//!
//! `commit_programming_preset` prepares its candidate through the show compiler, which stages
//! dependent Dynamic retention whenever a Preset changes. Those Dynamics are committed with the
//! Preset, so the one completion event must carry their exact committed bodies and revisions;
//! otherwise clients keep stale Dynamic bodies. Recording writes go through the real service,
//! SQLite store and event bus.
use super::semantic_intent::{beam_preset, color_preset};
use super::support::*;
use crate::programming::semantic_intent_cases::{color, warm_white_3200};
use crate::{
    ActiveShowObjectChange, ActiveShowObjectKind, ApplicationEvent, EventFilter, EventReplay,
    ProgrammingPresetCommit, ProgrammingPresetCommitResult, ProgrammingPresetRecordRequest,
    ProgrammingPresetRevisionExpectation, ShowEvent,
};
use light_core::FixtureId;
use light_core::programming::ProgrammingOwner;
use light_dynamics::{
    DynamicFamilyRepresentation, DynamicSemanticColorBasis, DynamicValueAddress, DynamicValueSource,
};
use light_programmer::{GroupDefinition, Preset, PresetAddress, PresetStoreMode};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use uuid::Uuid;

const GROUP: &str = "semantic-front";
const REFERENCING: &str = "00000000-0000-0000-0000-0000000574d1";
const UNRELATED: &str = "00000000-0000-0000-0000-0000000574d2";

fn color_address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        component: None,
    }
}

/// A whole-Color keyframe sourced from `preset_id`; retention is left to the show compiler.
fn color_dynamic(id: &str, pool_number: u32, preset_id: &str) -> Value {
    let id = Uuid::parse_str(id).unwrap();
    let source = DynamicValueSource::Preset {
        preset_id: preset_id.into(),
        address: color_address(),
        last_valid_by_target: Vec::new(),
        retained: None,
    };
    let mut body = dynamic_with_dependencies(id, GROUP, preset_id);
    body["pool_number"] = json!(pool_number);
    body["target_binding"] = json!({"type": "targetless"});
    body["lanes"] = json!([{
        "id": Uuid::new_v5(&id, b"lane"),
        "programming": {
            "address": color_address(),
            "configuration": {"mode": "keyframes", "configuration": {
                "points": [
                    {"position": 0.0, "source": source, "interpolation": "linear"},
                    {"position": 0.5, "source": {"kind": "current"}, "interpolation": "linear"}
                ],
                "size": 1.0
            }}
        },
        "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
    }]);
    body
}

fn with_number(mut preset: Preset, number: u32) -> Preset {
    preset.number = number;
    preset
}

/// Seeds P (Color 1) referenced by D, and Color 2 referenced only by the unrelated Dynamic.
/// A warm-up recording of an unrelated Beam Preset settles every compatibility migration and
/// the initial retention, so later events describe only what the recording itself changed.
fn seed(rig: &TestRig) -> FixtureId {
    let fixture = portable_fixture_record(574_000, 1);
    rig.target_profile(&fixture.profile);
    rig.target_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body.clone(),
    );
    rig.target_object(
        "group",
        GROUP,
        serde_json::to_value(GroupDefinition {
            id: GROUP.into(),
            name: "Front".into(),
            fixtures: vec![fixture.fixture_id],
            ..Default::default()
        })
        .unwrap(),
    );
    let id = fixture.fixture_id;
    rig.target_object(
        "preset",
        "2.1",
        serde_json::to_value(color_preset(id)).unwrap(),
    );
    rig.target_object(
        "preset",
        "2.2",
        serde_json::to_value(with_number(color_preset(id), 2)).unwrap(),
    );
    rig.target_object("dynamic", REFERENCING, color_dynamic(REFERENCING, 7, "2.1"));
    rig.target_object("dynamic", UNRELATED, color_dynamic(UNRELATED, 8, "2.2"));
    assert!(record(rig, &beam_preset(id)).unwrap().changed);
    for dynamic in [REFERENCING, UNRELATED] {
        let document = rig.target_document();
        let body = document.object("dynamic", dynamic).unwrap().body();
        assert!(
            body.to_string().contains("\"retained\":{"),
            "warm-up must settle the retained template of {dynamic}: {body}"
        );
    }
    id
}

fn record(
    rig: &TestRig,
    preset: &Preset,
) -> Result<ProgrammingPresetCommitResult, crate::ActionError> {
    let request = ProgrammingPresetRecordRequest {
        show_id: rig.target_id,
        address: PresetAddress::new(preset.family, preset.number).unwrap(),
        name: preset.name.clone(),
        mode: PresetStoreMode::Overwrite,
        expected_object_revision: ProgrammingPresetRevisionExpectation::Current,
        expected_show_revision: None,
    };
    let commit = ProgrammingPresetCommit::new(&request, preset.clone());
    rig.active_show
        .commit_programming_preset(&context(), &commit, &rig.ports)
}

/// P with a changed universal Color intent, which changes D's retained template.
fn changed_color_preset(fixture: FixtureId) -> Preset {
    let mut preset = color_preset(fixture);
    preset
        .universal_values
        .insert(ProgrammingOwner::Color.key(), color(warm_white_3200()));
    preset
}

fn events_after(rig: &TestRig, sequence: u64) -> Vec<Vec<ActiveShowObjectChange>> {
    let EventReplay::Events(events) = rig
        .active_show
        .events()
        .replay(sequence, &EventFilter::default())
    else {
        panic!("recording events should be retained");
    };
    events
        .iter()
        .map(|event| match &event.payload {
            ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) => change.changes.clone(),
            other => panic!("unexpected event {other:?}"),
        })
        .collect()
}

fn revision(rig: &TestRig, kind: &str, id: &str) -> u64 {
    rig.target_document().object(kind, id).unwrap().revision()
}

#[test]
fn semantic_preset_recording_publishes_every_rewritten_dynamic() {
    let rig = TestRig::new();
    let fixture = seed(&rig);
    let referencing_before = revision(&rig, "dynamic", REFERENCING);
    let unrelated_before = revision(&rig, "dynamic", UNRELATED);
    let sequence = rig.active_show.events().latest_sequence();

    let result = record(&rig, &changed_color_preset(fixture)).unwrap();

    assert!(result.changed);
    assert_eq!(result.event_sequence, Some(sequence + 1));
    let events = events_after(&rig, sequence);
    assert_eq!(events.len(), 1, "one completion event per recording");
    let changes = &events[0];
    let document = rig.target_document();
    assert_eq!(result.show_revision, document.revision());

    // The recorded Preset leads, exactly as the result reports it.
    assert_eq!(changes[0].kind, ActiveShowObjectKind::Preset);
    assert_eq!(changes[0].object_id, result.projection.object_id);
    assert_eq!(
        changes[0].object_revision,
        result.projection.object_revision
    );

    // D advanced, and the event carries its exact committed body and revision.
    let stored = document.object("dynamic", REFERENCING).unwrap();
    assert!(stored.revision() > referencing_before);
    let dynamic = changes
        .iter()
        .find(|change| change.kind == ActiveShowObjectKind::Dynamic)
        .expect("the completion event must carry the rewritten Dynamic");
    assert_eq!(dynamic.object_id, REFERENCING);
    assert_eq!(dynamic.object_revision, stored.revision());
    assert!(!dynamic.deleted);
    assert_eq!(
        &dynamic.body.as_ref().unwrap().encode(),
        stored.body(),
        "the event body must be the committed Dynamic body"
    );

    // The Dynamic sourced from an untouched Preset gets no spurious change or revision.
    assert_eq!(revision(&rig, "dynamic", UNRELATED), unrelated_before);
    assert!(
        changes.iter().all(|change| change.object_id != UNRELATED),
        "unrelated Dynamic must not be published: {changes:?}"
    );
    // Every published object is exactly one the commit wrote.
    for change in changes {
        let kind = match change.kind {
            ActiveShowObjectKind::Preset => "preset",
            ActiveShowObjectKind::Dynamic => "dynamic",
            other => panic!("unexpected published kind {other:?}"),
        };
        let object = document.object(kind, &change.object_id).unwrap();
        assert_eq!(change.object_revision, object.revision());
        assert_eq!(&change.body.as_ref().unwrap().encode(), object.body());
    }
    assert_eq!(changes.len(), 2, "{changes:?}");
}

#[test]
fn rerecording_identical_semantic_preset_publishes_nothing() {
    let rig = TestRig::new();
    let fixture = seed(&rig);
    let preset = changed_color_preset(fixture);
    assert!(record(&rig, &preset).unwrap().changed);
    let show_revision = rig.target_document().revision();
    let referencing = revision(&rig, "dynamic", REFERENCING);
    let unrelated = revision(&rig, "dynamic", UNRELATED);
    let sequence = rig.active_show.events().latest_sequence();

    let result = record(&rig, &preset).unwrap();

    assert!(!result.changed);
    assert_eq!(result.event_sequence, None);
    assert_eq!(rig.active_show.events().latest_sequence(), sequence);
    assert_eq!(rig.target_document().revision(), show_revision);
    assert_eq!(revision(&rig, "dynamic", REFERENCING), referencing);
    assert_eq!(revision(&rig, "dynamic", UNRELATED), unrelated);
}

#[test]
fn failed_recording_publishes_no_partial_object_changes() {
    for fail in ["prepare", "commit"] {
        let rig = TestRig::new();
        let fixture = seed(&rig);
        let show_revision = rig.target_document().revision();
        let preset_revision = revision(&rig, "preset", "2.1");
        let referencing = revision(&rig, "dynamic", REFERENCING);
        let sequence = rig.active_show.events().latest_sequence();
        match fail {
            "prepare" => rig.ports.fail_prepare.store(true, Ordering::SeqCst),
            _ => rig.ports.fail_commit.store(true, Ordering::SeqCst),
        }

        assert!(
            record(&rig, &changed_color_preset(fixture)).is_err(),
            "{fail}"
        );

        assert_eq!(
            rig.active_show.events().latest_sequence(),
            sequence,
            "{fail}"
        );
        assert_eq!(rig.target_document().revision(), show_revision, "{fail}");
        assert_eq!(revision(&rig, "preset", "2.1"), preset_revision, "{fail}");
        assert_eq!(
            revision(&rig, "dynamic", REFERENCING),
            referencing,
            "{fail}"
        );
    }
}

impl crate::programming::update::ProgrammingUpdatePorts for TestPorts {
    fn authorize_programming_update(
        &self,
        _context: &crate::ActionContext,
    ) -> Result<(), crate::ActionError> {
        Ok(())
    }

    fn active_update_cue_contexts(
        &self,
        _context: &crate::ActionContext,
    ) -> Result<Vec<crate::programming::update::ActiveCueContext>, crate::ActionError> {
        Ok(Vec::new())
    }
}

/// TL-557: a Preset *Update* of a semantic Color value stores the requested intent and, like
/// Preset recording, publishes every Dynamic whose retained source the show compiler restaged in
/// the same commit. Before TL-557 the Update completion event carried only the Preset.
#[test]
fn semantic_preset_update_publishes_every_rewritten_dynamic() {
    use crate::programming::semantic_intent_cases::magenta;
    use crate::programming::update::{
        ExistingContentMode, ProgrammingUpdateCommand, ProgrammingUpdatePreviewRequest,
        ProgrammingUpdateTargetRequest, UpdateMode,
    };
    use crate::{ActionContext, ActionEnvelope, ActionSource, EventBus, ProgrammingService};
    use light_core::SessionId;
    use light_programmer::{HighlightRegistry, ProgrammerRegistry};

    let rig = TestRig::new();
    let fixture = seed(&rig);
    let referencing_before = revision(&rig, "dynamic", REFERENCING);
    let unrelated_before = revision(&rig, "dynamic", UNRELATED);
    let registry = ProgrammerRegistry::default();
    let (desk, session) = (Uuid::from_u128(11), SessionId::new());
    registry.start(session);
    assert!(registry.attach_command_context(session, SessionId(desk)));
    let service = ProgrammingService::new(
        registry.clone(),
        EventBus::new(16),
        std::sync::Arc::new(HighlightRegistry::default()),
    );
    let key = ProgrammingOwner::Color.key();
    registry.set(session, fixture, key.clone(), color(magenta()));
    let context = |request: &str| {
        ActionContext::operator(desk, session.0, ActionSource::Http).with_request_id(request)
    };
    let target = ProgrammingUpdateTargetRequest::Preset {
        object_id: "2.1".into(),
    };
    let preview = service
        .preview_update(
            ActionEnvelope {
                context: context("tl557-update-preview"),
                command: ProgrammingUpdatePreviewRequest {
                    show_id: rig.target_id,
                    target: target.clone(),
                    mode: UpdateMode::ExistingContent(ExistingContentMode::UpdateExisting),
                },
            },
            &rig.active_show,
            &rig.ports,
        )
        .unwrap();
    let sequence = rig.active_show.events().latest_sequence();
    let result = service
        .handle_update(
            ActionEnvelope {
                context: context("tl557-update-apply"),
                command: ProgrammingUpdateCommand {
                    show_id: rig.target_id,
                    target,
                    mode: preview.preview.mode,
                    expected_object_revision: Some(preview.object_revision),
                    expected_programmer_revision: Some(preview.programmer_revision),
                    expected_show_revision: Some(preview.show_revision),
                },
            },
            &rig.active_show,
            &rig.ports,
        )
        .unwrap();

    let document = rig.target_document();
    let preset = document.object("preset", "2.1").unwrap();
    let stored: Preset = serde_json::from_value(preset.body().clone()).unwrap();
    assert_eq!(
        stored.values[&fixture][&key],
        color(magenta()),
        "the requested semantic intent is stored exactly"
    );
    let events = events_after(&rig, sequence);
    assert_eq!(events.len(), 1, "one completion event per Update");
    assert_eq!(result.outcome.event_sequence, sequence + 1);
    let changes = &events[0];
    assert_eq!(changes[0].kind, ActiveShowObjectKind::Preset);
    let stored_dynamic = document.object("dynamic", REFERENCING).unwrap();
    assert!(stored_dynamic.revision() > referencing_before);
    let dynamic = changes
        .iter()
        .find(|change| change.kind == ActiveShowObjectKind::Dynamic)
        .expect("the Update completion event must carry the rewritten Dynamic");
    assert_eq!(dynamic.object_id, REFERENCING);
    assert_eq!(dynamic.object_revision, stored_dynamic.revision());
    assert_eq!(
        &dynamic.body.as_ref().unwrap().encode(),
        stored_dynamic.body()
    );
    assert_eq!(revision(&rig, "dynamic", UNRELATED), unrelated_before);
    assert_eq!(changes.len(), 2, "{changes:?}");
}
