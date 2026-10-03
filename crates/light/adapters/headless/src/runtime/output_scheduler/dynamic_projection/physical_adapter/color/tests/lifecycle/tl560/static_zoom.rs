//! Regression for a defect found while filling TL-560 "Preload GO" Zoom: the all-family Live
//! frame only took static Position and Color owners (`FamilyFrameObserver::
//! static_program_targets`). A static typed Zoom from the Programmer, a played Cue or a
//! committed Preload therefore reached no adapter, and because the scalar path cannot render a
//! typed opening, the Zoom control kept its default (0) on the wire. Only a Dynamic/FixAT Zoom
//! was ever fitted. Focus stays a Normalized scalar and is not affected.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::OpticsAdapter;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::wash_a;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::field;

/// Wash A layout: 0 Intensity, 1 U16 Zoom, 2 Focus.
const ZOOM: u32 = 1;

#[tokio::test]
async fn a_static_programmer_zoom_is_fitted_and_encoded_by_the_live_family_frame() {
    let id = FixtureId::new();
    let wash = fixture(&wash_a(), id, 1, 1);
    let desk = Desk::open(EngineSnapshot {
        fixtures: vec![wash.clone()].into(),
        revision: 1,
        ..Default::default()
    });
    let session = SessionId::new();
    desk.programmers.start(session);
    desk.programmers
        .set(session, id, ProgrammingOwner::Zoom.key(), field(20.));
    let frame = desk.frame();
    let written = frame.family(ProgrammingOwner::Zoom, id);
    assert_eq!(
        written.len(),
        1,
        "the static Zoom is a family write on its owning control"
    );
    assert_eq!(written[0].0, ZOOM);
    let native = frame.assert_encoded(&wash);
    assert_eq!(native[ZOOM as usize], written[0].1);
    // The same value the Zoom adapter fits on its own from this desk's capture.
    let engine = desk.engine();
    let capture = engine.prepare_output_frame(RenderOptions::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let snapshot = capture.snapshot();
    let adapter = OpticsAdapter::pair().1;
    let descriptor = adapter.compile(&snapshot, id).unwrap().unwrap();
    let value = field(20.);
    let fitted = adapter
        .resolve(PhysicalRequest {
            frame: HybridFrameContext {
                capture: &capture,
                geometry: &geometry,
                native_models: snapshot.native_color_sources.as_ref(),
                token: &token,
                scalar: &scalar,
            },
            target: id,
            owner: ProgrammingOwner::Zoom,
            descriptor: &descriptor,
            value: &value,
            previous: None,
        })
        .unwrap();
    assert_eq!(
        fitted.writes[0].raw, written[0].1,
        "the adapter's fit is on the wire"
    );
    assert_ne!(written[0].1, 0, "never the default opening");
}
