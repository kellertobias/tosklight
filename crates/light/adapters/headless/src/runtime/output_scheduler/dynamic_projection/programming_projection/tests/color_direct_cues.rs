//! TL-603: actual Cue playback of portable Direct Color fades, and explicit endpoint output
//! suppression of a running Direct Dynamic, through the real captured Playback rows and output
//! controls, the shared Live hybrid resolver, the Color adapter, the generation's original source
//! catalogue and the engine finalizer (`Rig::frame`, which also asserts ONE Color owner).
//!
//! - Cue GO fades a whole Fixed Color mask (`CueDynamicChange` `ProgrammingFixAt`) from a static
//!   Cue underlay into a foreign Direct recipe; interior samples are the shared appearance
//!   interpolation of the ORIGINAL endpoints and completion is the exact tagged recipe. Back to
//!   the underlay Cue releases the mask.
//! - Swap on another Cuelist sets the running Cue Dynamic's captured `output_enabled` to false,
//!   which `CapturedFamilyEndpointControls` turns into `FamilyEndpointOutputControl::Suppressed`
//!   (not a higher FixAT). An uninterrupted reference replays the same Dynamic over the same
//!   captured times.
//!
//! No fitted DMX injection (TL-548) or cross-representation Size is asserted here.
use super::super::super::physical_adapter::color::profiles::{rgbal, rgbw};
use super::super::super::physical_adapter::color::tests::direct::{direct, identity};
use super::super::super::physical_adapter::color::tests::intent;
use super::super::super::physical_adapter::*;
use super::color_direct_dynamic::{
    Lane, assert_forward_fallback, direct_dynamic, dynamic_on, forward, portable,
};
use super::color_direct_transition::{Rig, semantic_value, uv_amount};
use super::color_direct_whole_fade::sample_appearance_fade;
use super::staged_geometry::{test_cue_list, test_playback};
use super::*;
use light_engine::PoolPlaybackAction;
use light_playback::{Cue, CueChange, CueDynamicChange, CueList};

/// A whole Fixed Color mask authored in a Cue, fading in over `fade_millis`.
fn fixed_mask(target: FixtureId, value: AttributeValue, fade_millis: u64) -> CueDynamicChange {
    CueDynamicChange {
        fixture_id: target,
        attribute: ProgrammingOwner::Color.key(),
        automatic_restore: false,
        value: DynamicSemanticValue::ProgrammingFixAt {
            mask: ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, None, value)
                .unwrap(),
            timing: DynamicValueTiming {
                fade_millis: Some(fade_millis),
                delay_millis: None,
            },
        },
    }
}

/// Cue 1 snaps the static `underlay`; Cue 2 fades a whole Fixed mask to `destination` in 1 s.
/// The Programmer no longer owns Color, so the Cuelist is the only source.
fn install_fade_cues(
    rig: &Rig,
    underlay: &AttributeValue,
    destination: &AttributeValue,
) -> CueList {
    rig.release_programmer_color();
    let mut first = Cue::new(1_u16.into());
    first.fade_millis = 0;
    first.changes = vec![CueChange::set(
        rig.target,
        ProgrammingOwner::Color.key(),
        underlay.clone(),
    )];
    let mut second = Cue::new(2_u16.into());
    second.fade_millis = 0;
    second.dynamic_changes = vec![fixed_mask(rig.target, destination.clone(), 1_000)];
    let list = test_cue_list(vec![first, second]);
    rig.install_show(vec![], vec![list.clone()], vec![test_playback(list.id)]);
    list
}

/// GO Cue 1 (static underlay, no typed owner), GO Cue 2 and sample the whole fade.
fn go_through_cue_fade(
    rig: &mut Rig,
    lane: &PhysicalAdapterLane<ColorAdapter>,
    underlay: &AttributeValue,
    destination: &AttributeValue,
) -> Vec<PhysicalHeadResult<ColorAdapter>> {
    rig.playback(1, PoolPlaybackAction::Go);
    assert!(rig.frame(10, lane).is_none(), "Cue 1 is a static owner");
    assert_eq!(rig.last_static.as_ref(), Some(underlay));
    rig.playback(1, PoolPlaybackAction::Go);
    let results = sample_appearance_fade(rig, lane, underlay, destination);
    assert!(
        rig.cue_rows.iter().any(|row| {
            row.fixture_id == rig.target
                && matches!(row.value, DynamicSemanticValue::ProgrammingFixAt { .. })
        }),
        "the fade is the captured Cue Fixed row"
    );
    results
}

/// Back to Cue 1 releases the Cue 2 mask and reveals the static underlay without a stale owner.
fn back_releases_the_mask(
    rig: &mut Rig,
    lane: &PhysicalAdapterLane<ColorAdapter>,
    underlay: &AttributeValue,
) {
    rig.playback(1, PoolPlaybackAction::Back);
    for _ in 0..4 {
        if let Some(result) = rig.frame(250, lane) {
            assert_eq!(&result.value, underlay);
        }
        assert!(rig.requirements.is_empty());
    }
    assert_eq!(rig.last_static.as_ref(), Some(underlay));
}

/// Semantic Cue → foreign Direct(B) Cue: the actual Cue Fixed mask is interpolated by known
/// portable appearance, restores B exactly and Back releases it. The stored Cue is unchanged.
#[test]
fn cue_go_fades_semantic_into_a_foreign_direct_recipe_and_back_releases_it() {
    let foreign = rgbal();
    let mut rig = Rig::new(|_, _| semantic_value(&ColorIntent::default()), &[&foreign]);
    let base = semantic_value(&intent([1., 0., 0.], 0.));
    let blue = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    let list = install_fade_cues(&rig, &base, &blue);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let results = go_through_cue_fade(&mut rig, &lane, &base, &blue);
    let red = |r: &PhysicalHeadResult<ColorAdapter>| r.writes[0].raw;
    let blue_raw = |r: &PhysicalHeadResult<ColorAdapter>| r.writes[2].raw;
    for pair in results[..3].windows(2) {
        assert!(red(&pair[1]) < red(&pair[0]) && blue_raw(&pair[1]) > blue_raw(&pair[0]));
    }
    back_releases_the_mask(&mut rig, &lane, &base);
    assert_eq!(rig.engine.snapshot().cue_lists[0], list, "stored Cues");
}

/// Foreign Direct(A) Cue → foreign Direct(B) Cue: the ORIGINAL A (never an adopted copy) is the
/// from endpoint, B is restored exactly, and Back restores A exactly.
#[test]
fn cue_go_fades_foreign_direct_a_into_foreign_direct_b_and_back_restores_a() {
    let a = rgbw();
    let b = rgbal();
    let mut rig = Rig::new(|_, _| semantic_value(&ColorIntent::default()), &[&a, &b]);
    let from = direct(&rig.catalogue, &a, &[65535, 0, 0, 0]);
    let to = direct(&rig.catalogue, &b, &[0, 255, 0, 0, 0]);
    let list = install_fade_cues(&rig, &from, &to);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    go_through_cue_fade(&mut rig, &lane, &from, &to);
    back_releases_the_mask(&mut rig, &lane, &from);
    assert_eq!(rig.engine.snapshot().cue_lists[0], list, "stored Cues");
}

/// Native Direct(P) Cue with full UV → foreign Direct(B) Cue without UV: UV fades on its own
/// linear amount (independent of the visible blend) to zero and never leaks from P.
#[test]
fn cue_fade_takes_uv_independently_to_zero() {
    let foreign = rgbal();
    let mut rig = Rig::new(|_, _| semantic_value(&ColorIntent::default()), &[&foreign]);
    let from = direct(&rig.catalogue, &rig.profile, &[200, 40, 0, 0, 0, 255]);
    let to = direct(&rig.catalogue, &foreign, &[0, 0, 255, 0, 0]);
    install_fade_cues(&rig, &from, &to);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let results = go_through_cue_fade(&mut rig, &lane, &from, &to);
    for (step, (result, progress)) in results[..3].iter().zip([0.25f32, 0.5, 0.75]).enumerate() {
        let uv = uv_amount(&result.value).unwrap();
        assert!(
            (uv - (1. - progress)).abs() < 1e-4,
            "step {step}: UV follows its own amounts ({uv})"
        );
        let expected = (f64::from(uv) * 255.).round() as u32;
        assert!(rig.uv_write(result).abs_diff(expected) <= 1);
    }
    assert_eq!(uv_amount(&results[3].value), Some(0.), "B carries no UV");
    assert_eq!(rig.uv_write(&results[3]), 0, "previous UV never leaks");
}

/// What a frame observed, kept for the uninterrupted reference.
struct Step {
    advance: i64,
    suppressed: bool,
    value: Option<AttributeValue>,
    clocks: Vec<String>,
}

/// A running whole Direct Dynamic (foreign source) owned by Cue 1 is explicitly suppressed by
/// Swap on another Cuelist: the captured row's `output_enabled == false` becomes
/// `FamilyEndpointOutputControl::Suppressed`. Its instance, clock/phase and Random stream advance
/// exactly as an uninterrupted reference while suppressed; re-enable shows the reference's
/// current recipe, forward-evaluated by the original model before fallback.
fn suppressed_cue_dynamic_keeps_running_and_reenables_on_its_current_recipe(kind: Lane) {
    let source = rgbal();
    let base = semantic_value(&ColorIntent::default());
    // The Programmer's static Semantic Color is the underlay the Cue Dynamic activates over.
    let mut rig = Rig::new(|_, _| base.clone(), &[&source]);
    let low = direct(&rig.catalogue, &source, &[65535, 0, 0, 0, 0]);
    let high = direct(&rig.catalogue, &source, &[0, 255, 0, 0, 0]);
    let definition = direct_dynamic(kind, &low, &high);
    let link = Uuid::from_u128(6031);
    let mut cue = Cue::new(1_u16.into());
    cue.dynamic_changes = definition
        .lanes
        .iter()
        .map(|lane| CueDynamicChange {
            fixture_id: rig.target,
            attribute: ProgrammingOwner::Color.key(),
            automatic_restore: false,
            value: dynamic_on(&definition, lane.id, link),
        })
        .collect();
    let list = test_cue_list(vec![cue]);
    let swap_list = test_cue_list(vec![Cue::new(1_u16.into())]);
    let mut swap = test_playback(swap_list.id);
    swap.number = 2;
    rig.install_show(
        vec![definition.clone()],
        vec![list.clone(), swap_list.clone()],
        vec![test_playback(list.id), swap],
    );
    rig.runtime
        .install_definitions([definition.clone()])
        .unwrap();
    rig.playback(1, PoolPlaybackAction::Go);
    let lane = PhysicalAdapterLane::live(ColorAdapter::default());
    for _ in 0..3 {
        let running = rig.typed(130, &lane);
        assert_forward_fallback(&running, "running");
    }
    let instances = rig.runtime.instance_ids();
    assert_eq!(instances.len(), 1);
    let started = light_core::ApplicationClock::now(rig.clock.as_ref());
    let fork = rig.runtime.fork_for_cold_install();
    let fork_origins = rig.origins.clone();

    let mut steps = Vec::new();
    let running = rig.typed(130, &lane);
    assert_forward_fallback(&running, "running");
    steps.push(Step {
        advance: 130,
        suppressed: false,
        value: Some(running.value),
        clocks: rig.clocks(),
    });
    // Explicit output suppression: the captured Cue row is disabled, the rows still reconcile.
    rig.playback(2, PoolPlaybackAction::SetSwap(true));
    for index in 0..4 {
        // The single Color owner shows the Programmer underlay, never a stale Dynamic recipe.
        let shown = rig.typed(150, &lane);
        assert_eq!(shown.value, base, "suppressed {index}: underlay");
        assert!(shown.quality.direct.is_none(), "suppressed {index}");
        let rows = rig
            .cue_rows
            .iter()
            .filter(|row| row.cue_list_id == list.id)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), definition.lanes.len(), "suppressed {index}");
        assert!(
            rows.iter().all(|row| !row.output_enabled),
            "suppressed {index}: the captured output gate is closed"
        );
        assert!(rig.requirements.is_empty());
        assert_eq!(rig.runtime.instance_ids(), instances, "suppressed {index}");
        steps.push(Step {
            advance: 150,
            suppressed: true,
            value: None,
            clocks: rig.clocks(),
        });
    }
    rig.playback(2, PoolPlaybackAction::SetSwap(false));
    for index in 0..4 {
        let enabled = rig.typed(170, &lane);
        assert!(
            rig.cue_rows.iter().all(|row| row.output_enabled),
            "re-enabled {index}: the captured output gate is open"
        );
        assert_forward_fallback(&enabled, "re-enabled");
        assert_eq!(
            portable(&enabled.value),
            forward(&rig, &enabled.value),
            "re-enabled {index}: current original recipe and forward estimate"
        );
        steps.push(Step {
            advance: 170,
            suppressed: false,
            value: Some(enabled.value),
            clocks: rig.clocks(),
        });
    }
    assert_eq!(rig.runtime.instance_ids(), instances, "same instance");
    let stored = rig
        .runtime
        .instance_definition(instances[0])
        .unwrap()
        .clone();
    assert_eq!(stored.lanes, definition.lanes, "stored Dynamic recipe");
    assert_eq!(stored.random_groups, definition.random_groups);
    let show = rig.engine.snapshot();
    assert_eq!(show.cue_lists[0], list, "stored Cue rows");
    assert_eq!(show.dynamics[0], definition, "stored Dynamic pool entry");

    // Uninterrupted reference: the same runtime fork over the same captured times, no Swap.
    rig.clock.set(started);
    rig.runtime = fork;
    rig.origins = fork_origins;
    let reference_lane = PhysicalAdapterLane::live(ColorAdapter::default());
    let mut suppressed = 0;
    for (index, step) in steps.into_iter().enumerate() {
        let reference = rig.typed(step.advance, &reference_lane);
        assert_forward_fallback(&reference, "reference");
        assert_eq!(rig.runtime.instance_ids(), instances, "reference {index}");
        assert_eq!(
            rig.clocks(),
            step.clocks,
            "step {index}: identity, clock/phase and Random advanced identically"
        );
        if step.suppressed {
            suppressed += 1;
            continue;
        }
        assert_eq!(
            step.value,
            Some(reference.value),
            "step {index}: same recipe as the uninterrupted Dynamic"
        );
    }
    assert_eq!(suppressed, 4);
    assert!(rig.catalogue.resolve(&identity(&source)).is_ok());
}

#[test]
fn suppressed_cue_direct_keyframes_keep_identity_and_phase_and_reenable_on_the_current_recipe() {
    suppressed_cue_dynamic_keeps_running_and_reenables_on_its_current_recipe(Lane::Keyframes);
}

#[test]
fn suppressed_cue_direct_random_keeps_its_stream_and_reenables_on_the_current_recipe() {
    suppressed_cue_dynamic_keeps_running_and_reenables_on_its_current_recipe(Lane::Random);
}
