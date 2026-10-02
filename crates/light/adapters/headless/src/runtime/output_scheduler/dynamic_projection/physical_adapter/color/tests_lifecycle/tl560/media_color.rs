//! TL-560 column "Media color" beyond Cue record/reload: Preset record, Update, Undo and a live
//! Group whose membership changes after recording.
//!
//! The shipped Media Server personality is patched with two layer heads. One shared semantic
//! Color is recorded against a live Group holding layer 1 into a Color Preset through the real
//! Preset writer, which stores it as a universal Preset, then changed through the real Update
//! writer (preview, then apply that preview); it stays universal (TL-638). Programmer undo of a
//! layer Group colour restores the exact earlier intent. Every assertion reopens the SQLite show.
//! The updated Group value is recorded into a Cue, layer 2 is
//! added to the live Group, and a fresh contract-1 desk plays the reopened Cue: both layers get
//! the Media tint/Grayscale controls of the stored intent (Grayscale is White Blend; the layer
//! dimmer is never a Color write) without any Preset or Cue rewrite.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::media_color::tests::{
    media_fixture, shipped_media_server,
};
use light_programmer::PresetFamily;

fn red() -> ColorIntent {
    ColorIntent {
        white_blend: 1.,
        ..super::super::super::tests::intent([1., 0., 0.], 0.)
    }
}

fn amber() -> ColorIntent {
    ColorIntent {
        white_blend: 0.25,
        ..super::super::super::tests::intent([1., 0.735, 0.], 0.)
    }
}

/// The stored Preset: `(universal colour, body)`. Recording one shared colour stores a universal
/// Preset that names no fixture or Group.
fn preset_color(show: &Show, object_id: &str) -> (AttributeValue, Value) {
    let document = show.document();
    let body = document
        .object("preset", object_id)
        .expect("the recorded Preset")
        .body()
        .clone();
    let preset: light_programmer::Preset = serde_json::from_value(body.clone()).unwrap();
    assert!(preset.is_universal(), "{body}");
    assert!(preset.values.is_empty(), "{body}");
    assert!(preset.group_values.is_empty(), "{body}");
    assert_eq!(preset.universal_values.len(), 1, "{body}");
    (
        preset.universal_values[&ProgrammingOwner::Color.key()].clone(),
        body,
    )
}

#[tokio::test]
async fn media_layer_color_survives_preset_record_update_undo_and_live_group_growth() {
    let show = Show::new();
    let (root, l1, l2) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let media = media_fixture(&shipped_media_server(), root, &[l1, l2]);
    show.patch(&media);
    show.group(&[l1]);
    let color = ProgrammingOwner::Color.key();
    let session = show.session;

    // Programmer undo of a Media layer Group colour restores the exact earlier intent.
    show.programmers
        .set_group(session, GROUP.into(), color.clone(), program(&red()));
    show.programmers
        .set_group(session, GROUP.into(), color.clone(), program(&amber()));
    assert!(show.programmers.undo(session));
    assert_eq!(
        show.programmers.get(session).unwrap().group_values[GROUP][&color].value,
        program(&red())
    );

    // Preset record: one shared live Group colour, stored as a universal Preset.
    let preset_id = record_preset(&show, &show.programming, session, PresetFamily::Color, 1);
    let (stored, recorded) = preset_color(&show, &preset_id);
    assert_eq!(stored, program(&red()));
    assert_eq!(
        show.document().objects_of_kind("preset").count(),
        1,
        "one Preset"
    );

    // Update Existing with the amber intent: the universal Preset takes it and stays universal.
    show.programmers
        .set_group(session, GROUP.into(), color.clone(), program(&amber()));
    update_preset(&show, &show.programming, session, &preset_id);
    let (updated, updated_body) = preset_color(&show, &preset_id);
    assert_eq!(
        updated,
        program(&amber()),
        "Update stores the new exact intent"
    );
    assert_ne!(updated_body, recorded);
    assert_eq!(
        updated_body["name"], recorded["name"],
        "Update keeps the Preset"
    );
    assert_eq!(show.document().objects_of_kind("preset").count(), 1);
    // The reopened show compiles through the show-open reader and requires contract 1.
    let snapshot = show.compile();
    assert_eq!(snapshot.required_programming_contract, 1);

    // Record only the Group value into a Cue, then grow the live Group.
    show.programmers.clear(session);
    show.programmers.start(session);
    show.programmers
        .set_group(session, GROUP.into(), color.clone(), program(&amber()));
    show.record(1.0);
    let (_, cue_list) = show.cue_list();
    assert_eq!(stored_group_value(&cue_list, 0, "color"), program(&amber()));
    show.group(&[l1, l2]);
    assert_eq!(show.cue_list().1, cue_list, "no Cue rewrite");
    assert_eq!(
        preset_color(&show, &preset_id).1,
        updated_body,
        "no Preset rewrite"
    );

    let desk = Desk::open(show.compile());
    desk.go(1.0);
    let frame = desk.frame();
    for layer in [l1, l2] {
        assert_eq!(
            desk.played(layer, ProgrammingOwner::Color),
            Some(program(&amber()))
        );
        let written: Vec<u32> = frame
            .family(ProgrammingOwner::Color, layer)
            .into_iter()
            .map(|(_, raw)| raw)
            .collect();
        assert_eq!(
            written,
            [0, 128, 255, 64],
            "C, M, Y, Grayscale of {layer:?}"
        );
    }
    frame.assert_encoded(&media);
}
