//! TL-557 AC4/AC8 seam references: orthogonal Dynamic White Blend and UV lanes compose over one
//! semantic Color owner, reach the lamp adapter under one token, and never bake the achieved
//! output or the Dynamic value back into the programmed request.
use super::super::super::physical_adapter::color::profiles::{patched, rgbw, rgbwauv};
use super::super::super::physical_adapter::color::tests::{intent, magenta};
use super::super::super::physical_adapter::*;
use super::color_physical::{Live, color_lane, semantic};
use super::*;

fn raws(result: &PhysicalHeadResult<ColorAdapter>) -> Vec<u32> {
    result.writes.iter().map(|w| w.raw).collect()
}

fn programmed(live: &Live, target: FixtureId) -> Vec<AttributeValue> {
    let state = live.programmers.get(live.session).unwrap();
    state
        .values
        .iter()
        .filter(|v| v.fixture_id == target && v.attribute == ProgrammingOwner::Color.key())
        .map(|v| v.value.clone())
        .collect()
}

#[test]
fn a_white_blend_dynamic_composes_orthogonally_over_the_base_and_drives_the_white_emitter() {
    let target = FixtureId::new();
    let base = magenta();
    let mut live = Live::start(
        target,
        &base,
        color_lane("White Blend", ColorComponent::WhiteBlend, 1.),
    );
    live.install_fixtures(vec![patched(&rgbw(), target, 1)]);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    for _ in 0..2 {
        let capture = live.capture();
        let published = live.run(&capture, &lane).unwrap();
        assert!(published.requirements.is_empty());
        let [result] = published.results.as_slice() else {
            panic!("one complete Color owner")
        };
        let composed = semantic(&result.value);
        assert_eq!(
            composed.white_blend, 1.,
            "the Dynamic lane owns White Blend"
        );
        let mut expected = base.clone();
        expected.white_blend = 1.;
        assert_eq!(
            composed, &expected,
            "every other field is the programmed base"
        );
        assert_eq!(&result.requested, composed);
        let written = raws(result);
        assert_eq!(written.len(), 4);
        assert!(
            written[3] > 0,
            "White Blend 1 drives the white emitter: {written:?}"
        );
    }
    assert_eq!(
        programmed(&live, target),
        [super::super::super::physical_adapter::color::tests::program(&base)],
        "neither the Dynamic value nor the achieved output is baked into the Programmer"
    );
}

#[test]
fn a_uv_dynamic_adds_independent_excitation_and_never_turns_purple_into_uv() {
    let target = FixtureId::new();
    let purple = intent([1., 0., 1.], 0.);
    let profile = rgbwauv(None);
    let mut off = Live::start(target, &purple, color_lane("UV", ColorComponent::Uv, 0.));
    off.install_fixtures(vec![patched(&profile, target, 1)]);
    let mut on = Live::start(target, &purple, color_lane("UV", ColorComponent::Uv, 0.6));
    on.install_fixtures(vec![patched(&profile, target, 1)]);
    let (off_lane, on_lane) = (
        PhysicalAdapterLane::live(ColorAdapter::default()),
        PhysicalAdapterLane::live(ColorAdapter::default()),
    );
    let capture = off.capture();
    let dark = off.run(&capture, &off_lane).unwrap();
    let capture = on.capture();
    let lit = on.run(&capture, &on_lane).unwrap();
    let (dark, lit) = (&dark.results[0], &lit.results[0]);
    assert_eq!(semantic(&lit.value).uv.amount, 0.6);
    assert_eq!(semantic(&lit.value).recipe, purple.recipe);
    let uv = |r: &PhysicalHeadResult<ColorAdapter>| *r.writes.last().unwrap();
    assert_eq!((uv(dark).raw, uv(dark).parked), (0, true), "UV 0 is parked");
    assert_eq!(uv(lit).raw, (0.6f64 * 255.).round() as u32);
    assert_eq!(
        raws(dark)[..5],
        raws(lit)[..5],
        "visible purple is fitted identically with UV frozen"
    );
    assert_eq!(
        dark.quality.uv,
        light_fixture::forward::UvFitStatus::Applied
    );
    assert!(
        !lit.quality.uv_appearance_known && lit.achieved.visible.is_none(),
        "unknown UV appearance degrades the total prediction without blocking"
    );
}
