//! TL-560 matrix row "Preload GO", columns Focus and Media color: the GO commit itself.
//!
//! On a fresh contract-1 desk the Programmer arms Preload and stages, as one pending batch, a
//! Focus on Wash A and a Media colour on a live Group holding layer 1 of the shipped Media
//! Server. While Pending nothing reaches Live. The production headless GO
//! (`preload::commit_preload`) commits both unchanged into Live, where the family frame encodes
//! them. Recording that Live Preload through the real Cue writer stores the same intents, and a
//! reopened show plays them back to the same native output.
use super::preload_go::session;
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::media_color::tests::{
    media_fixture, shipped_media_server,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::wash_a;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::focus;
use light_programmer::{
    CueRecordingCapturedSource, PreloadProgrammerValueMutation, PreloadProgrammerValueTiming,
};

const CUE: f64 = 1.0;

fn media_amber() -> AttributeValue {
    program(&ColorIntent {
        white_blend: 0.25,
        ..super::super::super::super::tests::intent([1., 0.735, 0.], 0.)
    })
}

/// The Live output of both committed owners: `(wash native, media native)`.
fn assert_output(
    desk: &Desk,
    wash: &PatchedFixture,
    media: &PatchedFixture,
    layer: FixtureId,
) -> (Vec<u32>, Vec<u32>) {
    let frame = desk.frame();
    let written: Vec<u32> = frame
        .family(ProgrammingOwner::Color, layer)
        .into_iter()
        .map(|(_, raw)| raw)
        .collect();
    assert_eq!(
        written,
        [0, 128, 255, 64],
        "C, M, Y, Grayscale of the Media layer"
    );
    let wash_native = frame.assert_encoded(wash);
    // Focus reaches the wire (never the default 0); its authored-curve value is the ignored
    // BUG test in `focus`.
    assert_ne!(wash_native[2], 0, "Focus is output");
    (wash_native, frame.assert_encoded(media))
}

#[tokio::test]
async fn preload_go_commits_focus_and_media_color_unchanged_into_live_and_the_recorded_show() {
    let show = Show::new();
    let (o, root, layer) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let wash = fixture(&wash_a(), o, 1, 1);
    let mut media = media_fixture(&shipped_media_server(), root, &[layer, FixtureId::new()]);
    media.fixture_number = Some(2);
    media.universe = Some(2);
    show.patch(&wash);
    show.patch(&media);
    show.group(&[layer]);

    let desk = Desk::open(show.compile());
    let operator = session(SessionId::new());
    desk.state.programming.start(operator.id);
    let active = desk
        .state
        .installation
        .upsert_show(
            "TL-560 Preload Focus/Media",
            &show.ports.path.display().to_string(),
            false,
        )
        .unwrap();
    desk.state.active_show.replace_current(Some(active));
    assert!(desk.state.programming.arm_preload(operator.id, true));
    let timing = PreloadProgrammerValueTiming::default();
    let staged = [
        PreloadProgrammerValueMutation::SetFixture {
            fixture_id: o,
            attribute: ProgrammingOwner::Focus.key(),
            value: focus(0.37),
            timing,
        },
        PreloadProgrammerValueMutation::SetGroup {
            group_id: GROUP.into(),
            attribute: ProgrammingOwner::Color.key(),
            value: media_amber(),
            timing,
        },
    ];
    assert!(desk.programmers.apply_preload_values(operator.id, &staged));
    let expected = [
        (o, ProgrammingOwner::Focus, focus(0.37)),
        (layer, ProgrammingOwner::Color, media_amber()),
    ];

    // Pending: blind.
    let frame = desk.frame();
    for (id, owner, _) in &expected {
        assert_eq!(desk.played(*id, *owner), None, "{owner:?} is not Live");
        assert!(frame.family(*owner, *id).is_empty());
    }

    // GO through the production headless Preload transaction.
    crate::runtime::preload::commit_preload(&desk.state, &operator).unwrap();
    let committed = desk.programmers.get(operator.id).unwrap();
    assert!(committed.preload_pending.is_empty());
    let row = committed
        .preload_active
        .iter()
        .find(|row| row.fixture_id == o && row.attribute == ProgrammingOwner::Focus.key())
        .expect("Focus committed");
    assert_eq!(row.value, focus(0.37), "Focus committed unchanged");
    assert_eq!(
        committed.preload_group_active[GROUP][&ProgrammingOwner::Color.key()].value,
        media_amber(),
        "Media colour committed unchanged"
    );
    desk.clock.advance_millis(600_000);
    for (id, owner, value) in &expected {
        assert_eq!(
            desk.played(*id, *owner).as_ref(),
            Some(value),
            "{owner:?} is Live after GO"
        );
    }
    let live = assert_output(&desk, &wash, &media, layer);

    // Record the committed Live Preload through the real Cue writer.
    let programming = ProgrammingService::new(
        desk.programmers.clone(),
        EventBus::new(16),
        Arc::new(HighlightRegistry::default()),
    );
    assert_eq!(
        record_from(
            &show,
            &programming,
            operator.id,
            CUE,
            ProgrammingCueCapturePolicy::PendingOrActivePreload,
        ),
        CueRecordingCapturedSource::ActivePreload
    );
    let (_, recorded) = show.cue_list();
    assert_eq!(stored_fixture_value(&recorded, 0, o, "focus"), focus(0.37));
    assert_eq!(stored_group_value(&recorded, 0, "color"), media_amber());

    // Reopen: the recorded Cue plays the same intents to the same native output.
    drop(desk);
    let reopened = Desk::open(show.compile());
    reopened.go(CUE);
    for (id, owner, value) in &expected {
        assert_eq!(
            reopened.played(*id, *owner).as_ref(),
            Some(value),
            "{owner:?}"
        );
    }
    let played = assert_output(&reopened, &wash, &media, layer);
    assert_eq!(played, live, "same native output after reopen");
}
