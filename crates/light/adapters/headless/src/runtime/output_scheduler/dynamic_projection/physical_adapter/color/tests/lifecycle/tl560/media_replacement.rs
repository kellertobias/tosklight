//! TL-560 column "Media color", row "Fixture replacement (output)".
//!
//! The shipped Media Server is patched in its 2-layer personality. A live Group of both layers
//! is programmed one Media colour and layer 2 gets its own colour; both are recorded into one
//! Cue through the real Cue writer and the SQLite show is reopened. The fixture is then replaced
//! by the 8-layer personality of the same package under the same root and layer identities
//! (layers 3–8 are new heads, layer 3 joins the live Group). On a fresh contract-1 desk the
//! reopened Cue plays the stored intents unchanged and every layer of the new personality gets
//! the Media tint/Grayscale controls of its intent at the new layout, without a Cue rewrite.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::media_color::tests::shipped_media_server;
use light_fixture::PatchedHead;

const CUE: f64 = 1.0;

fn amber() -> AttributeValue {
    program(&ColorIntent {
        white_blend: 0.25,
        ..super::super::super::super::tests::intent([1., 0.735, 0.], 0.)
    })
}

fn red() -> AttributeValue {
    program(&ColorIntent {
        white_blend: 1.,
        ..super::super::super::super::tests::intent([1., 0., 0.], 0.)
    })
}

/// The Media Server in personality `mode` at universe 1 address 1, each layer a logical head.
fn media(
    profile: &FixtureProfile,
    mode: usize,
    root: FixtureId,
    layers: &[FixtureId],
) -> PatchedFixture {
    let mut fixture = fixture(profile, root, 1, 1);
    let personality = &profile.modes[mode];
    fixture.definition = profile.resolved_definition(personality.id).unwrap();
    let mut ids = layers.iter();
    for (index, head) in personality.heads.iter().enumerate() {
        if head.master_shared {
            continue;
        }
        fixture.logical_heads.push(PatchedHead {
            profile_head_id: Some(head.id),
            head_index: fixture.definition.heads[index].index,
            fixture_id: *ids.next().expect("one id per layer"),
        });
    }
    assert!(ids.next().is_none(), "one id per layer");
    fixture
}

fn written(frame: &DeskFrame, layer: FixtureId) -> Vec<u32> {
    frame
        .family(ProgrammingOwner::Color, layer)
        .into_iter()
        .map(|(_, raw)| raw)
        .collect()
}

#[tokio::test]
async fn media_layer_cue_survives_reopen_and_replacement_by_the_eight_layer_personality() {
    let show = Show::new();
    let profile = shipped_media_server();
    assert_eq!(
        profile
            .modes
            .iter()
            .map(|m| m.heads.len())
            .collect::<Vec<_>>(),
        [3, 9],
        "2-layer and 8-layer personalities plus their master heads"
    );
    let root = FixtureId::new();
    let layers: [FixtureId; 8] = std::array::from_fn(|_| FixtureId::new());
    let two = media(&profile, 0, root, &layers[..2]);
    show.patch(&two);
    show.group(&layers[..2]);
    show.programmers.set_group(
        show.session,
        GROUP.into(),
        ProgrammingOwner::Color.key(),
        amber(),
    );
    show.programmers.set(
        show.session,
        layers[1],
        ProgrammingOwner::Color.key(),
        red(),
    );
    show.record(CUE);
    let (list_id, recorded) = show.cue_list();
    assert_eq!(stored_group_value(&recorded, 0, "color"), amber());
    assert_eq!(
        stored_fixture_value(&recorded, 0, layers[1], "color"),
        red()
    );

    let desk = Desk::open(show.compile());
    desk.go(CUE);
    let frame = desk.frame();
    assert_eq!(
        desk.played(layers[0], ProgrammingOwner::Color),
        Some(amber())
    );
    assert_eq!(desk.played(layers[1], ProgrammingOwner::Color), Some(red()));
    let amber_writes = written(&frame, layers[0]);
    let red_writes = written(&frame, layers[1]);
    assert_eq!(amber_writes, [0, 128, 255, 64], "C, M, Y, Grayscale");
    assert_ne!(red_writes, amber_writes);
    frame.assert_encoded(&two);
    drop(desk);

    // Replace the personality under the same identities; a new layer joins the live Group.
    let eight = media(&profile, 1, root, &layers);
    show.patch(&eight);
    show.group(&[layers[0], layers[1], layers[2]]);
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
    let desk = Desk::open(show.compile());
    desk.go(CUE);
    let frame = desk.frame();
    for (layer, value, writes) in [
        (layers[0], amber(), &amber_writes),
        (layers[1], red(), &red_writes),
        (layers[2], amber(), &amber_writes),
    ] {
        assert_eq!(desk.played(layer, ProgrammingOwner::Color), Some(value));
        assert_eq!(
            &written(&frame, layer),
            writes,
            "the same Media controls at the 8-layer layout"
        );
    }
    for layer in &layers[3..] {
        assert!(written(&frame, *layer).is_empty(), "unprogrammed layers");
    }
    frame.assert_encoded(&eight);
}
