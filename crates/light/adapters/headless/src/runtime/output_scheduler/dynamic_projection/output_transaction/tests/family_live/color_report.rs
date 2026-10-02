//! TL-594 track B: with the family adapters engaged, the colour report names the Color results
//! of the accepted output frame the publication hub holds, never a re-resolution of the current
//! target or of an invented white.
use super::*;
use crate::runtime::color_intent_report::accepted_frame_report;
use crate::runtime::tests::test_state_with_family_adapters;
use light_wire::v2::attribute_configuration::{
    ColorIntentFrameState, ColorIntentUvStatus, ColorResolutionQuality,
};

fn render_and_publish(state: &crate::runtime::AppState) {
    let rendered = state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            state.output.render_options(),
        )
        .unwrap();
    state.output.render_frames_and_publish(
        &rendered,
        light_wire::v2::visualization::VisualizationScope { show_id: None },
    );
}

#[test]
fn the_accepted_frame_report_reads_published_color_results_and_invents_no_white() {
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(2_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let (state, data_dir) = test_state_with_family_adapters(
        programmers.clone(),
        Some(clock.clone()),
        PROGRAMMING_CONTRACT_VERSION,
    );
    let show = Show::new();
    // An RGB wash nobody programs a colour for: the legacy report shows it against white.
    let idle = FixtureId::new();
    let mut fixtures = show.fixtures();
    let mut unprogrammed = patched(&rgb(), idle, 1);
    unprogrammed.universe = Some(7);
    unprogrammed.fixture_number = Some(99);
    fixtures.push(unprogrammed);
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let session = SessionId::new();
    programmers.start(session);
    program_show(&programmers, &clock, session, &show, None);

    // Nothing accepted yet: passive, no rows.
    let report = accepted_frame_report(&state, None);
    let frame = report.accepted_frame.unwrap();
    assert_eq!(frame.state, ColorIntentFrameState::NotYetAvailable);
    assert!(frame.frame.is_none());
    assert!(report.heads.is_empty());

    clock.advance_millis(25);
    render_and_publish(&state);
    let published = state.output.latest_visualization_frame().unwrap();
    let accepted = state
        .output
        .live_family_adapters()
        .accepted_color(published.generation, published.sampled_at)
        .expect("the published frame's Color results");
    let report = accepted_frame_report(&state, None);
    let frame = report.accepted_frame.clone().unwrap();
    assert_eq!(frame.state, ColorIntentFrameState::Accepted);
    assert_eq!(frame.frame, Some(published.identity()));

    // The programmed wash is reported from its own encoded Color sidecar.
    let wash = report
        .heads
        .iter()
        .filter(|head| head.fixture_id == show.wash.0)
        .collect::<Vec<_>>();
    assert_eq!(wash.len(), 1, "{:?}", report.heads);
    let record = accepted
        .heads
        .iter()
        .find(|row| row.target == show.wash)
        .expect("the wash's Color sidecar");
    assert!(wash[0].has_target);
    assert_eq!(wash[0].delta_uv, record.delta_uv);
    assert!(record.delta_uv.is_some(), "the fitter's own match figure");
    assert_eq!(
        format!("{:?}", wash[0].quality),
        format!("{:?}", record.quality),
        "quality is the sidecar's"
    );
    assert!(wash[0].engine.is_none() && wash[0].calibration_revision.is_none());
    // TL-550: the lamp's UV result travels beside, never inside, its visible match.
    let uv = wash[0].uv.expect("a lamp head reports its UV result");
    assert_eq!(
        Some(uv.status),
        record.uv.map(|uv| match uv.status {
            light_fixture::forward::UvFitStatus::NotRequested => ColorIntentUvStatus::NotRequested,
            light_fixture::forward::UvFitStatus::Applied => ColorIntentUvStatus::Applied,
            light_fixture::forward::UvFitStatus::Unsupported => ColorIntentUvStatus::Unsupported,
        })
    );

    // No active requested colour, no row: nothing is reported against an invented white.
    let legacy = state.output.engine().color_intent_report(None).unwrap();
    assert!(
        legacy
            .iter()
            .any(|head| head.fixture_id == idle && head.target.is_none()),
        "the legacy report resolves the unprogrammed wash against white"
    );
    for absent in [idle, show.mover, show.optics, show.dimmer] {
        assert!(
            report.heads.iter().all(|head| head.fixture_id != absent.0),
            "{absent:?} has no active requested colour"
        );
    }
    // The Media layer is reported from its Media Color sidecar, which has no u'v' figure.
    let layer = report
        .heads
        .iter()
        .find(|head| head.owner_id == show.layer.0)
        .expect("the programmed Media layer");
    assert_eq!(layer.quality, ColorResolutionQuality::Exact);
    assert!(layer.delta_uv.is_none());
    assert!(layer.uv.is_none(), "Media has no lamp UV result");
    assert_eq!(report.heads.len(), 2, "only the two programmed colours");

    // A patch edit moves the generation past the published frame: passive until it is output.
    let mut edited = (*state.output.snapshot()).clone();
    edited.revision += 1;
    state.output.replace_snapshot(edited).unwrap();
    let report = accepted_frame_report(&state, None);
    assert_eq!(
        report.accepted_frame.unwrap().state,
        ColorIntentFrameState::NotYetAvailable
    );
    assert!(report.heads.is_empty());
    clock.advance_millis(25);
    render_and_publish(&state);
    let report = accepted_frame_report(&state, None);
    let frame = report.accepted_frame.unwrap();
    assert_eq!(frame.state, ColorIntentFrameState::Accepted);
    assert_eq!(
        frame.frame,
        Some(
            state
                .output
                .latest_visualization_frame()
                .unwrap()
                .identity()
        )
    );
    assert!(
        report
            .heads
            .iter()
            .any(|head| head.fixture_id == show.wash.0)
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}

/// TL-552: a head shown through a derived model says what was parked; a requested colour on a
/// head with no colour model is a row with the reason, never silently missing.
#[test]
fn the_report_names_parked_controls_and_explains_a_head_without_a_colour_model() {
    let shipped = |name: &str, mode: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/fixture-library")
            .join(format!("{name}.toskfixture"));
        let mut profile =
            light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        profile.modes.retain(|m| m.name == mode);
        light_fixture::apply_runtime_profile_compatibility(&mut profile);
        profile
    };
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(2_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let (state, data_dir) = test_state_with_family_adapters(
        programmers.clone(),
        Some(clock.clone()),
        PROGRAMMING_CONTRACT_VERSION,
    );
    let (jbled, lustr) = (FixtureId::new(), FixtureId::new());
    let mut fixtures = Vec::new();
    for (number, (id, profile)) in [
        (
            jbled,
            shipped("jb-lighting--jbled-a7", "Compressed RGB 8 Bit (C8)"),
        ),
        (
            lustr,
            shipped("etc--source-four-led-series-2-lustr", "HSI Plus 7"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut fixture = patched(&profile, id, 1);
        fixture.universe = Some(number as u16 + 1);
        fixture.fixture_number = Some(number as u32 + 1);
        fixtures.push(fixture);
    }
    state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: fixtures.into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let session = SessionId::new();
    programmers.start(session);
    for id in [jbled, lustr] {
        programmers.set(
            session,
            id,
            ProgrammingOwner::Color.key(),
            program(&intent([1., 0., 0.], 0.)),
        );
    }
    clock.advance_millis(25);
    render_and_publish(&state);
    let report = accepted_frame_report(&state, None);
    let row = |id: FixtureId| {
        report
            .heads
            .iter()
            .find(|head| head.fixture_id == id.0)
            .unwrap_or_else(|| panic!("a report row for {id:?}: {:?}", report.heads))
    };
    let derived = row(jbled);
    assert_eq!(derived.quality, ColorResolutionQuality::Uncalibrated);
    let note = derived
        .note
        .as_deref()
        .expect("the parked controls are named");
    assert!(
        note.contains("parked at neutral") && note.contains("CTC"),
        "{note}"
    );
    let excluded = row(lustr);
    assert_eq!(excluded.quality, ColorResolutionQuality::Unsupported);
    assert!(excluded.has_target);
    let reason = excluded.note.as_deref().expect("the reason is reported");
    assert!(
        reason.starts_with("No colour model:") && reason.contains("hue/saturation"),
        "{reason}"
    );
    assert_eq!(report.heads.len(), 2);
    std::fs::remove_dir_all(data_dir).unwrap();
}
