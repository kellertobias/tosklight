use super::*;

fn engine_with_one_fixture() -> Engine {
    let (fixture, _) = schema_v2_fixture(&[("intensity", false, false, false, false, false)]);
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    engine
}

/// A frame is published dense and stays that way.
///
/// The output path point-queries the values it needs. Building the map would put a pass over the
/// whole show back into every frame, which is the cost this design exists to remove, so it is
/// asserted rather than left to be noticed in a profile later.
#[test]
fn rendering_never_builds_the_boundary_map() {
    let engine = engine_with_one_fixture();

    for _ in 0..8 {
        let rendered = engine.render(RenderOptions::default()).unwrap();
        assert!(
            rendered.resolved_values.is_dense(),
            "a frame the patch can hold is published dense"
        );
        assert!(
            !rendered.resolved_values.materialised_by_name(),
            "rendering must not build a map of the whole show"
        );
    }
}

/// Asking by name is what builds the map, and it is built once however often it is asked.
#[test]
fn asking_by_name_builds_the_map_once() {
    let engine = engine_with_one_fixture();

    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert!(!rendered.resolved_values.materialised_by_name());
    let first = rendered.resolved_values.values() as *const _;
    assert!(rendered.resolved_values.materialised_by_name());
    let second = rendered.resolved_values.values() as *const _;
    assert_eq!(first, second, "the map is built once, not per ask");
}

/// A held frame keeps its buffer, and a released one is refilled rather than replaced.
#[test]
fn a_steady_render_refills_its_buffers_instead_of_building_more() {
    let engine = engine_with_one_fixture();

    // Each frame is dropped before the next is rendered, as the output path does.
    for _ in 0..32 {
        let _ = engine.render(RenderOptions::default()).unwrap();
    }
    assert!(
        engine.frames_built() <= 2,
        "a desk holding its rate refills its buffers rather than building one per frame, built {}",
        engine.frames_built()
    );
}

/// The bound is what keeps a stalled consumer from costing the desk anything but a frame.
#[test]
fn frames_held_past_the_bound_do_not_stall_the_desk() {
    let engine = engine_with_one_fixture();

    // A consumer that never lets go: every frame is retained.
    let held = (0..16)
        .map(|_| engine.render(RenderOptions::default()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(held.len(), 16, "the desk keeps rendering regardless");
    for frame in &held {
        assert!(!frame.universes.is_empty(), "and keeps producing output");
    }
}
