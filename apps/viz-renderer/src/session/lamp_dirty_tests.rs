//! CPU-only regression for a final lamp picture settling before a drawable becomes available.
use super::*;
use crate::redraw::{RedrawGate, RedrawState};
use std::time::Duration;
use viz_scene::{LampResponseProfile, PersistencePreference, ProviderCapabilities};

struct QuietProvider;

impl SceneProvider for QuietProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            kind: ProviderKind::PlanningSoftware,
            available: true,
            unavailable_reason: None,
            default_host: String::new(),
            default_port: 0,
            uses_network_input: false,
        }
    }
    fn poll(&mut self) -> Vec<ProviderEvent> {
        Vec::new()
    }
    fn request_resync(&mut self) {}
    fn shutdown(&mut self) {}
}

fn cold_session(now: Instant, shutter: f32) -> Session {
    let mut session = Session::new(Box::new(QuietProvider), ProviderKind::PlanningSoftware, now);
    session.scene = crate::demo::build_scene();
    let fixture = session.scene.emitters[0].fixture_index as usize;
    session.scene.fixtures[fixture].lamp_response = LampResponseProfile::TungstenNominal;
    session.values.resize(session.scene.emitters.len());
    session.values.advance_lamp_response(&session.scene, 0.0);
    let mut incoming = SceneValues::default();
    incoming.resize(session.scene.emitters.len());
    incoming.frame = 42;
    incoming.emitters[0].intensity = 1.0;
    incoming.emitters[0].shutter = shutter;
    session.accept_values(incoming, now);
    assert!(!session.lamp_picture_dirty());
    session
}

fn picture(session: &Session) -> RedrawState {
    RedrawState::new(
        session.scene.revision,
        0,
        &session.values,
        &session.source_view,
        (640, 360),
        &[],
    )
}

#[test]
fn settled_final_lamp_picture_stays_owed_without_incrementing_provider_frame() {
    let now = Instant::now();
    let mut session = cold_session(now, 1.0);
    let previous = picture(&session);
    let mut gate = RedrawGate::default();
    gate.succeeded(previous.clone());

    // The CPU simulation can finish while occluded or waiting for its first drawable.
    session.pump(now + Duration::from_secs(2));
    assert_eq!(session.values.emitters[0].visible_intensity(), 1.0);
    assert!(
        !session
            .values
            .is_time_driven(&PersistencePreference::default())
    );
    assert_eq!(
        session.values.frame, 42,
        "local simulation never authors a provider frame"
    );
    let settled = picture(&session);
    assert_eq!(
        settled, previous,
        "the unchanged stamp cannot identify a final lamp update"
    );
    assert!(
        !gate.should_draw(&settled, false),
        "the previous gating path lost this final picture"
    );
    assert!(session.lamp_picture_dirty());
    assert!(gate.should_draw(&settled, session.lamp_picture_dirty()));

    // Both failed and skipped admissions leave the successful-picture acknowledgment owed.
    session.observe_lamp_submission(false);
    session.observe_lamp_submission(false);
    session.pump(now + Duration::from_secs(20));
    assert!(session.lamp_picture_dirty());
    session.observe_lamp_submission(true);
    gate.succeeded(settled.clone());
    assert!(!session.lamp_picture_dirty());
    assert!(!gate.should_draw(&settled, session.lamp_picture_dirty()));
}

#[test]
fn invisible_thermal_progress_does_not_latch_a_picture_debt() {
    let now = Instant::now();
    let mut session = cold_session(now, 0.0);
    session.pump(now + Duration::from_secs(2));
    assert_eq!(
        session.values.emitters[0]
            .lamp_response
            .as_ref()
            .unwrap()
            .output(),
        1.0
    );
    assert_eq!(session.values.emitters[0].visible_intensity(), 0.0);
    assert!(!session.lamp_picture_dirty());
    assert_eq!(session.values.frame, 42);
}

#[test]
fn adopting_another_value_packet_cannot_acknowledge_an_owed_lamp_picture() {
    let now = Instant::now();
    let mut session = cold_session(now, 1.0);
    session.pump(now + Duration::from_secs(2));
    let mut incoming = SceneValues::default();
    incoming.resize(session.scene.emitters.len());
    incoming.frame = 43;
    incoming.emitters[0].intensity = 1.0;
    session.accept_values(incoming, now + Duration::from_secs(2));
    assert!(session.lamp_picture_dirty());
    assert_eq!(session.values.frame, 43);
}
