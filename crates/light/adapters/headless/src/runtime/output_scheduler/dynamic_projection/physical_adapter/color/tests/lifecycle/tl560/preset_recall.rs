//! TL-560 matrix row "Preset recall", columns UV and Focus.
//!
//! Each Preset is recorded through the real Preset writer, the SQLite show is reopened, a fresh
//! contract-1 desk selects the fixtures statically and recalls the Preset through the real
//! `ProgrammingService::handle_preset_recall` planner (the recall environment is the reopened
//! portable document, see `replacement_presets::Recall`). The stored Preset body and revision
//! are unchanged by the recall, the Programmer holds exactly the stored intent, and the
//! production Live family frame encodes it.
use super::replacement_presets::Recall;
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::wash_a;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::{
    field, focus,
};
use light_application::{
    ProgrammingPresetRecallRequest, ProgrammingPresetRecallRevisionExpectation,
};
use light_core::programming::UvIntent;
use light_programmer::{PresetAddress, PresetFamily};

/// `(object id, revision, body)` of every stored Preset, sorted by id.
pub(super) fn stored_presets(show: &Show) -> Vec<(String, u64, Value)> {
    let mut presets: Vec<_> = show
        .document()
        .objects_of_kind("preset")
        .map(|o| (o.key().id().to_owned(), o.revision(), o.body().clone()))
        .collect();
    presets.sort_by(|a, b| a.0.cmp(&b.0));
    presets
}

/// Select `fixtures` statically on a fresh desk and recall Preset `family` `number`.
pub(super) fn recall_preset(
    show: &Show,
    desk: &Desk,
    session: SessionId,
    fixtures: &[FixtureId],
    family: PresetFamily,
    number: u32,
) {
    let programming = ProgrammingService::new(
        desk.programmers.clone(),
        EventBus::new(16),
        Arc::new(HighlightRegistry::default()),
    );
    desk.programmers.select(session, fixtures.to_vec());
    let current = ProgrammingPresetRecallRevisionExpectation::Current;
    programming
        .handle_preset_recall(
            ActionEnvelope {
                context: operator(session, "recall"),
                command: ProgrammingPresetRecallRequest {
                    show_id: show.ports.show_id,
                    address: PresetAddress::new(family, number).unwrap(),
                    expected_preset_revision: current,
                    expected_show_revision: current,
                    expected_values_revision: current,
                    expected_preload_values_revision: current,
                    expected_capture_mode_revision: current,
                    expected_selection_revision: current,
                },
            },
            &Recall { show },
        )
        .unwrap();
    desk.clock.advance_millis(600_000);
}

/// The Programmer's own value of `owner` on `fixture`.
pub(super) fn programmer_value(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    fixture: FixtureId,
    owner: ProgrammingOwner,
) -> Option<AttributeValue> {
    programmers.get(session).and_then(|state| {
        state
            .values
            .iter()
            .find(|value| value.fixture_id == fixture && value.attribute == owner.key())
            .map(|value| value.value.clone())
    })
}

fn with_uv(amount: f32) -> AttributeValue {
    program(&ColorIntent {
        uv: UvIntent { amount },
        ..magenta()
    })
}

fn uv_channel(profile: &FixtureProfile) -> usize {
    profile.modes[0]
        .channels
        .iter()
        .position(|c| c.attribute.0.contains("uv"))
        .expect("a UV channel")
}

#[tokio::test]
async fn recalled_semantic_uv_preset_restores_each_stored_uv_amount_without_rewriting_the_preset() {
    let show = Show::new();
    let (a, b) = (FixtureId::new(), FixtureId::new());
    let profile = rgbwauv(None);
    let fixtures = [fixture(&profile, a, 1, 1), fixture(&profile, b, 2, 40)];
    for patched in &fixtures {
        show.patch(patched);
    }
    let color = ProgrammingOwner::Color.key();
    // Two different UV amounts on the same visible colour: stored per fixture, not universal.
    show.programmers
        .set(show.session, a, color.clone(), with_uv(0.45));
    show.programmers
        .set(show.session, b, color.clone(), with_uv(0.9));
    let preset_id = record_preset(
        &show,
        &show.programming,
        show.session,
        PresetFamily::Color,
        1,
    );
    let recorded = stored_presets(&show);
    assert_eq!(recorded.len(), 1);
    let body = &recorded[0].2;
    assert_eq!(recorded[0].0, preset_id);
    for (id, amount) in [(a, 0.45), (b, 0.9)] {
        assert_eq!(
            body["values"][id.0.to_string()]["color"],
            serde_json::to_value(with_uv(amount)).unwrap(),
            "the exact requested intent with its own UV amount: {body}"
        );
    }

    let desk = Desk::open(show.compile());
    let session = SessionId::new();
    desk.programmers.start(session);
    recall_preset(&show, &desk, session, &[a, b], PresetFamily::Color, 1);
    assert_eq!(stored_presets(&show), recorded, "recall never rewrites");
    let frame = desk.frame();
    let mut visible = Vec::new();
    for ((patched, id), amount) in fixtures.iter().zip([a, b]).zip([0.45f32, 0.9]) {
        assert_eq!(
            programmer_value(&desk.programmers, session, id, ProgrammingOwner::Color),
            Some(with_uv(amount)),
            "the Programmer holds the stored UV intent"
        );
        assert_eq!(
            desk.played(id, ProgrammingOwner::Color),
            Some(with_uv(amount))
        );
        let mut native = frame.assert_encoded(patched);
        let channel = uv_channel(&profile);
        assert_eq!(
            native[channel],
            (f64::from(amount) * 255.).round() as u32,
            "UV emitter carries the recalled amount"
        );
        native.remove(channel);
        visible.push(native);
    }
    assert_eq!(
        visible[0], visible[1],
        "UV is independent of the visible magenta fit"
    );
}

#[tokio::test]
async fn recalled_focus_preset_sets_focus_only_and_leaves_the_programmer_zoom() {
    let show = Show::new();
    let f = FixtureId::new();
    let wash = fixture(&wash_a(), f, 1, 1);
    show.patch(&wash);
    show.programmers
        .set(show.session, f, ProgrammingOwner::Focus.key(), focus(0.37));
    record_preset(
        &show,
        &show.programming,
        show.session,
        PresetFamily::Beam,
        1,
    );
    let recorded = stored_presets(&show);
    assert_eq!(recorded.len(), 1);
    let body = &recorded[0].2;
    assert_eq!(
        body["values"][f.0.to_string()]["focus"],
        serde_json::to_value(focus(0.37)).unwrap(),
        "{body}"
    );
    assert!(
        body["values"][f.0.to_string()].get("zoom").is_none(),
        "Focus is recorded without Zoom: {body}"
    );

    let desk = Desk::open(show.compile());
    let session = SessionId::new();
    desk.programmers.start(session);
    // An unrelated Programmer Zoom and a different Focus before the recall.
    desk.programmers
        .set(session, f, ProgrammingOwner::Zoom.key(), field(30.));
    desk.programmers
        .set(session, f, ProgrammingOwner::Focus.key(), focus(0.8));
    recall_preset(&show, &desk, session, &[f], PresetFamily::Beam, 1);
    assert_eq!(stored_presets(&show), recorded, "recall never rewrites");
    assert_eq!(
        programmer_value(&desk.programmers, session, f, ProgrammingOwner::Focus),
        Some(focus(0.37)),
        "the recalled Focus replaces the Programmer Focus"
    );
    assert_eq!(
        programmer_value(&desk.programmers, session, f, ProgrammingOwner::Zoom),
        Some(field(30.)),
        "Zoom is an independent owner and keeps its Programmer value"
    );
    assert_eq!(desk.played(f, ProgrammingOwner::Focus), Some(focus(0.37)));
    assert_eq!(desk.played(f, ProgrammingOwner::Zoom), Some(field(30.)));
    // Encoded on the wire; the authored-curve value is the ignored BUG test in `focus`.
    desk.frame().assert_encoded(&wash);
}

#[tokio::test]
async fn recalled_media_color_preset_restores_each_layer_intent_without_rewriting_the_preset() {
    use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::media_color::tests::{
        media_fixture, shipped_media_server,
    };
    let show = Show::new();
    let (root, l1, l2) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let media = media_fixture(&shipped_media_server(), root, &[l1, l2]);
    show.patch(&media);
    let amber = program(&ColorIntent {
        white_blend: 0.25,
        ..super::super::super::super::tests::intent([1., 0.735, 0.], 0.)
    });
    let color = ProgrammingOwner::Color.key();
    show.programmers
        .set(show.session, l1, color.clone(), amber.clone());
    show.programmers
        .set(show.session, l2, color.clone(), program(&magenta()));
    record_preset(
        &show,
        &show.programming,
        show.session,
        PresetFamily::Color,
        1,
    );
    let recorded = stored_presets(&show);
    assert_eq!(recorded.len(), 1);
    for (layer, value) in [(l1, &amber), (l2, &program(&magenta()))] {
        assert_eq!(
            recorded[0].2["values"][layer.0.to_string()]["color"],
            serde_json::to_value(value).unwrap(),
            "each layer stores its own exact intent: {}",
            recorded[0].2
        );
    }

    let desk = Desk::open(show.compile());
    let session = SessionId::new();
    desk.programmers.start(session);
    recall_preset(&show, &desk, session, &[l1, l2], PresetFamily::Color, 1);
    assert_eq!(stored_presets(&show), recorded, "recall never rewrites");
    let frame = desk.frame();
    for (layer, value) in [(l1, amber.clone()), (l2, program(&magenta()))] {
        assert_eq!(
            programmer_value(&desk.programmers, session, layer, ProgrammingOwner::Color),
            Some(value.clone())
        );
        assert_eq!(desk.played(layer, ProgrammingOwner::Color), Some(value));
    }
    let written = |layer| -> Vec<u32> {
        frame
            .family(ProgrammingOwner::Color, layer)
            .into_iter()
            .map(|(_, raw)| raw)
            .collect()
    };
    assert_eq!(
        written(l1),
        [0, 128, 255, 64],
        "C, M, Y, Grayscale of layer 1"
    );
    assert_eq!(written(l2).len(), 4, "every Media Color control of layer 2");
    assert_ne!(written(l2), written(l1));
    frame.assert_encoded(&media);
}
