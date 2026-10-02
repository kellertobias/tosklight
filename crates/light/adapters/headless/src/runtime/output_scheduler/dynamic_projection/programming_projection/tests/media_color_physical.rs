//! TL-593 seam test: the Media Color adapter as the destination writer inside the existing Live
//! hybrid composer, observer and engine finalizer. A running White Blend Dynamic composes over the
//! programmed base and reaches the Media personality controls every frame under one token; the
//! programmed intent is never rewritten by the derived controls.
use super::super::super::physical_adapter::color::tests::{intent, program};
use super::super::super::physical_adapter::media_color::tests::{
    media_fixture, shipped_media_server,
};
use super::super::super::physical_adapter::*;
use super::color_physical::{Live, color_lane, semantic};
use super::*;

fn raws<A: PhysicalFamilyAdapter>(result: &PhysicalHeadResult<A>) -> Vec<u32> {
    result.writes.iter().map(|w| w.raw).collect()
}

#[test]
fn a_white_blend_dynamic_reaches_the_media_layer_controls_without_feedback() {
    let (root, layer, other) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let base = intent([1., 0., 0.], 0.);
    let mut live = Live::start(
        layer,
        &base,
        color_lane("White Blend", ColorComponent::WhiteBlend, 1.),
    );
    live.install_fixtures(vec![media_fixture(
        &shipped_media_server(),
        root,
        &[layer, other],
    )]);
    let lane = PhysicalAdapterLane::live(MediaColorAdapter::default());

    let capture = live.capture();
    let published = live.run(&capture, &lane).unwrap();
    assert!(published.requirements.is_empty());
    let [result] = published.results.as_slice() else {
        panic!("one complete Color owner on the layer")
    };
    assert_eq!(
        (result.target, result.token.clone()),
        (layer, published.token.clone())
    );
    let composed = semantic(&result.value);
    assert_eq!(composed.white_blend, 1., "the Dynamic lane composed");
    assert_eq!(&result.requested, composed);
    assert_eq!(composed.recipe, base.recipe, "the base tint is retained");
    // Red tint kept at 100% White Blend: cyan 0, magenta 255, yellow 255, Grayscale 255.
    assert_eq!(raws(result), [0, 255, 255, 255]);
    assert_eq!(result.achieved.white_blend, Some(1.));
    assert_eq!(
        published
            .rendered
            .resolved_values
            .value(layer, &ProgrammingOwner::Color.key()),
        Some(&result.value),
        "the engine receives the value the sidecar describes"
    );

    // A new programmed base (an Update or cue change) reaches the next frame; the Dynamic keeps
    // White Blend and the stored request stays exactly what was programmed.
    let tinted = intent([1., 0.735, 0.], 0.);
    live.programmers.set(
        live.session,
        layer,
        ProgrammingOwner::Color.key(),
        program(&tinted),
    );
    let second = live.capture();
    let published = live.run(&second, &lane).unwrap();
    let result = &published.results[0];
    assert_eq!(result.token, second.frame_token());
    assert_eq!(raws(result), [0, 128, 255, 255]);
    let state = live.programmers.get(live.session).unwrap();
    let stored: Vec<_> = state
        .values
        .iter()
        .filter(|v| v.fixture_id == layer && v.attribute == ProgrammingOwner::Color.key())
        .map(|v| &v.value)
        .collect();
    assert_eq!(
        stored,
        [&program(&tinted)],
        "no feedback into the programmed intent"
    );
    let counters = lane.adapter().counters();
    assert_eq!((counters.descriptor_compiles, counters.resolves), (1, 2));
}
