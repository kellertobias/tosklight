//! TL-544 A2 (G10): a static Focus (Programmer, Cue, committed Preload) is a typed family like
//! Zoom. The Focus adapter fits and encodes it through the profile's focus function, and it
//! stays independent of Zoom.
use super::*;
use crate::runtime::AppState;
use crate::runtime::output_scheduler::dynamic_projection::output_transaction::family_frame::PublishedFamilyWrites;
use crate::runtime::tests::test_state_with_family_adapters;

/// Wash A layout: 0 Intensity, 1 U16 Zoom, 2 U8 Focus (100 % at raw 10 → 0 % at raw 200).
const ZOOM: u32 = 1;
const FOCUS: u32 = 2;

struct Desk {
    state: AppState,
    clock: Arc<ManualClock>,
    programmers: ProgrammerRegistry,
    session: SessionId,
    data_dir: std::path::PathBuf,
}

impl Drop for Desk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

impl Desk {
    fn new(fixtures: Vec<PatchedFixture>) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(3_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let (state, data_dir) = test_state_with_family_adapters(
            programmers.clone(),
            Some(clock.clone()),
            PROGRAMMING_CONTRACT_VERSION,
        );
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
        Self {
            state,
            clock,
            programmers,
            session,
            data_dir,
        }
    }

    fn set(&self, target: FixtureId, owner: ProgrammingOwner, value: AttributeValue) {
        self.clock.advance_millis(10);
        self.programmers
            .set(self.session, target, owner.key(), value);
    }

    /// Render and publish one Live frame; returns its rendered output and family writes.
    fn frame(&self) -> (RenderResult, PublishedFamilyWrites) {
        self.clock.advance_millis(25);
        let rendered = self
            .state
            .output
            .render_with_playback_events(
                &self.state.active_show.output_projection(),
                &self.state.playback.render_capability(),
                self.state.output.render_options(),
            )
            .unwrap();
        self.state.output.render_frames_and_publish(
            &rendered,
            light_wire::v2::visualization::VisualizationScope { show_id: None },
        );
        let published = self
            .state
            .output
            .live_family_adapters()
            .take_published()
            .expect("the family frame published its writes");
        (rendered.rendered, published)
    }
}

fn writes(
    published: &PublishedFamilyWrites,
    owner: ProgrammingOwner,
    target: FixtureId,
) -> Vec<(u32, u32)> {
    let mut writes: Vec<_> = published
        .writes
        .iter()
        .filter(|(o, t, _)| *o == owner && *t == target)
        .map(|(_, _, write)| (write.slot.channel_index, write.raw))
        .collect();
    writes.sort_unstable();
    writes
}

/// Wash A's measured reversed Focus: 100 % at raw 10, 0 % at raw 200.
fn wash_a_focus_raw(normalized: f64) -> f64 {
    10. + (1. - normalized) * 190.
}

#[test]
fn a_static_programmer_focus_is_fitted_by_the_focus_adapter_independently_of_zoom() {
    let id = FixtureId::new();
    let desk = Desk::new(vec![patched(&wash_a(), id, 1)]);
    desk.set(id, ProgrammingOwner::Focus, focus(0.37));
    let (rendered, published) = desk.frame();
    let focus_writes = writes(&published, ProgrammingOwner::Focus, id);
    assert_eq!(
        focus_writes.len(),
        1,
        "a static Focus is a family write on its owning control: {:?}",
        published.writes
    );
    assert_eq!(focus_writes[0].0, FOCUS);
    assert!(
        (f64::from(focus_writes[0].1) - wash_a_focus_raw(0.37)).abs() <= 1.,
        "Focus 37 % through the measured reversed curve: {focus_writes:?}"
    );
    assert_eq!(
        instance_raw(&rendered, id, FOCUS),
        focus_writes[0].1,
        "the fitted write is the rendered native output"
    );
    assert!(
        writes(&published, ProgrammingOwner::Zoom, id).is_empty(),
        "Focus never writes or activates Zoom"
    );

    // Zoom joins as its own owner; Focus keeps its write.
    desk.set(id, ProgrammingOwner::Zoom, field(20.));
    let (_, published) = desk.frame();
    assert_eq!(
        writes(&published, ProgrammingOwner::Focus, id),
        focus_writes
    );
    let zoom = writes(&published, ProgrammingOwner::Zoom, id);
    assert_eq!(zoom.len(), 1);
    assert_eq!(zoom[0].0, ZOOM);

    // Releasing Focus leaves the Zoom write exactly where it was.
    desk.clock.advance_millis(10);
    desk.programmers
        .release_fixture_attribute(desk.session, id, &ProgrammingOwner::Focus.key());
    let (_, published) = desk.frame();
    assert!(writes(&published, ProgrammingOwner::Focus, id).is_empty());
    assert_eq!(writes(&published, ProgrammingOwner::Zoom, id), zoom);
}
