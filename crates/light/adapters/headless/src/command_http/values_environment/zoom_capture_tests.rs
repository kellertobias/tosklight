//! TL-637 follow-up: the first Zoom edit from scratch adopts the displayed accepted output's
//! opening through the compiled optics forward model, under the same displayed-source lease rules
//! as Position. Without a measurable opening in a known convention the whole action holds
//! quietly (`zoom_unavailable`): no mutation, no revision, no Undo step, no error.
use super::*;
use crate::runtime::output_readouts::read_readouts;
use crate::runtime::output_scheduler::physical_adapters::optics::profiles::{
    Curve, OpticsBuilder, patched,
};
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use light_application::{
    ProgrammingDisplayedLane, ProgrammingDisplayedSource, ProgrammingValueIntent,
    ProgrammingValueOperation, ProgrammingValuesHold,
};
use light_core::programming::{
    ComponentEdit, ProgrammingComponent, ProgrammingOwner, ScalarEdit, ScalarIntent, ZoomIntent,
};
use light_core::{ManualClock, MergeMode, OpeningConvention, SessionId, TimedValue};
use light_engine::{ContributionBatch, ContributionSample, EngineSnapshot};
use light_fixture::{ChannelResolution, FixtureProfile, PhysicalDataQuality};
use light_programmer::ProgrammerRegistry;
use light_wire::v2::visualization::{VisualizationLane, VisualizationScope};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

/// A Beam zoom 10° (raw 0) → 17° (raw 128) → 25° (raw 255), like the AURO SPOT Z300 range.
fn beam_zoom() -> FixtureProfile {
    let mut curve = Curve::zoom(0, 255, (10., 25.), &[(128, 17.)]);
    curve.quality = PhysicalDataQuality::Manufacturer;
    curve.convention = Some(OpeningConvention::Beam);
    OpticsBuilder::new("TL-637 follow-up Beam zoom")
        .zoom(ChannelResolution::U8, curve)
        .build()
}

/// Native Zoom travel only: no unit, no calibration, no convention. Nothing can be measured.
fn nominal_zoom() -> FixtureProfile {
    OpticsBuilder::new("TL-637 follow-up nominal zoom")
        .zoom(ChannelResolution::U8, Curve::nominal(0, 255))
        .build()
}

fn beam(degrees: f32) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention: OpeningConvention::Beam,
    }))
}

fn zoom_key() -> AttributeKey {
    ProgrammingOwner::Zoom.key()
}

// ---------------------------------------------------------------------------------------------
// Capture: exactly the displayed frame, through the compiled optics model.

fn install(state: &AppState, profile: &FixtureProfile) -> FixtureId {
    let fixture = patched(profile, FixtureId::new(), 1);
    let owner = fixture.fixture_id;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    owner
}

/// Accept one Live frame whose legacy native Zoom is `normalized` (no typed Zoom request).
fn publish_native(state: &AppState, owner: FixtureId, normalized: f32) {
    let sample = ContributionSample::independent(TimedValue {
        fixture_id: owner,
        attribute: AttributeKey("zoom".into()),
        value: AttributeValue::Normalized(normalized),
        priority: 100,
        changed_at: chrono::Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    });
    let frame = RenderedSemanticFrame::untraced(
        state
            .output
            .engine()
            .render_with_contribution_batches(
                Default::default(),
                &[ContributionBatch::new(vec![sample])],
            )
            .unwrap(),
        Default::default(),
    );
    state
        .output
        .render_frames_and_publish(&frame, VisualizationScope { show_id: None });
}

fn zoom_intent(
    owner: FixtureId,
    displayed: Option<ProgrammingDisplayedSource>,
) -> ProgrammingValueIntent {
    ProgrammingValueIntent {
        fixture_ids: vec![owner],
        group_id: None,
        attribute: zoom_key(),
        operation: ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Zoom,
            operation: ScalarEdit::Relative(1.),
        }]),
        undo_group: Some("zoom-turn".into()),
        timing: Default::default(),
        displayed_source: displayed,
        color_adoption: Default::default(),
    }
}

fn adopt(
    state: &AppState,
    session: SessionId,
    owner: FixtureId,
    preload: bool,
    displayed: Option<ProgrammingDisplayedSource>,
) -> ProgrammingValuesEnvironment {
    let mut environment = values_environment(state);
    prepare_family_edit_context(
        state,
        session,
        preload,
        &zoom_intent(owner, displayed),
        &mut environment,
    );
    environment
}

fn live(lease: u64) -> ProgrammingDisplayedSource {
    ProgrammingDisplayedSource {
        lane: ProgrammingDisplayedLane::Normal,
        lease,
    }
}

#[tokio::test]
async fn the_first_zoom_edit_adopts_the_displayed_frames_measured_opening_not_latest() {
    let (state, directory) = crate::runtime::tests::test_state();
    let owner = install(&state, &beam_zoom());
    let session = SessionId::new();
    state.programming.start(session);
    // Shown: native raw 255 = 25° Beam. Then latest advances to raw 0 = 10°.
    publish_native(&state, owner, 1.);
    let lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner])
        .lease
        .expect("the accepted frame is leased");
    publish_native(&state, owner, 0.);

    let shown = adopt(&state, session, owner, false, Some(live(lease)));
    assert_eq!(shown.displayed_source_hold, None);
    assert_eq!(
        shown.current_values.get(&(owner, zoom_key())),
        Some(&beam(25.)),
        "the seed is the displayed frame's opening in degrees, never the raw percentage"
    );
    // Without a displayed source (OSC, HTTP integrators) adoption reads the latest frame.
    let latest = adopt(&state, session, owner, false, None);
    assert_eq!(
        latest.current_values.get(&(owner, zoom_key())),
        Some(&beam(10.))
    );

    // A gone or foreign lease holds with the re-read reason and seeds nothing from latest.
    let held = adopt(&state, session, owner, false, Some(live(9_999_999)));
    assert_eq!(
        held.displayed_source_hold,
        Some(ProgrammingValuesHold::DisplayedSourceUnavailable)
    );
    assert!(!matches!(
        held.current_values.get(&(owner, zoom_key())),
        Some(AttributeValue::Zoom(_))
    ));
    // Preload never adopts from Live: without an accepted Pending pair nothing is seeded.
    let preload = adopt(&state, session, owner, true, None);
    assert!(!matches!(
        preload.current_values.get(&(owner, zoom_key())),
        Some(AttributeValue::Zoom(_))
    ));
    let _ = std::fs::remove_dir_all(directory);
}

/// A fixed accepted Pending capture of one Programmer (the TL-548 source seam).
struct PendingCapture {
    programmer: light_core::ProgrammerId,
    captured: crate::runtime::position_readout::CapturedPositionReadouts,
}

impl crate::runtime::position_readout::PendingPositionReadoutSource for PendingCapture {
    fn capture(
        &self,
        programmer: light_core::ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<crate::runtime::position_readout::CapturedPositionReadouts> {
        let mut captured = self.captured.clone();
        (programmer == self.programmer).then_some(())?;
        captured
            .owners
            .retain(|owner| owners.contains(&owner.owner));
        Some(captured)
    }
}

#[tokio::test]
async fn a_preload_zoom_edit_adopts_its_own_pending_lease_and_never_live() {
    let (state, directory) = crate::runtime::tests::test_state();
    let owner = install(&state, &beam_zoom());
    let session = SessionId::new();
    state.programming.start(session);
    // The Pending pair's After branch shows raw 128 = 17° Beam; Live then shows raw 255 = 25°.
    publish_native(&state, owner, 128. / 255.);
    let published = state.output.latest_visualization_frame().unwrap();
    let pending = crate::runtime::position_readout::capture_position_readouts(
        state.output.engine(),
        &published,
        &[owner],
    )
    .unwrap();
    state
        .output
        .pending_position_readouts()
        .install(Arc::new(PendingCapture {
            programmer: state.programming.get(session).unwrap().id,
            captured: pending,
        }));
    publish_native(&state, owner, 1.);
    let live_lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner])
        .lease
        .unwrap();
    let pending_lease = read_readouts(&state, session, VisualizationLane::Preload, &[owner])
        .lease
        .expect("an accepted Pending capture is leased");

    let preload = ProgrammingDisplayedSource {
        lane: ProgrammingDisplayedLane::Preload,
        lease: pending_lease,
    };
    let adopted = adopt(&state, session, owner, true, Some(preload));
    assert_eq!(adopted.displayed_source_hold, None);
    assert_eq!(
        adopted.current_values.get(&(owner, zoom_key())),
        Some(&beam(17.)),
        "the Pending frame's opening, not Live's 25°"
    );
    let held = adopt(&state, session, owner, true, Some(live(live_lease)));
    assert_eq!(
        held.displayed_source_hold,
        Some(ProgrammingValuesHold::DisplayedSourceUnavailable),
        "Preload never adopts a Live lease"
    );
    assert_eq!(
        adopt(&state, session, owner, false, Some(preload)).displayed_source_hold,
        Some(ProgrammingValuesHold::DisplayedSourceUnavailable),
        "a Normal edit cannot adopt a Pending lease"
    );
    state.output.pending_position_readouts().clear();
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn an_unmeasurable_zoom_seeds_nothing() {
    let (state, directory) = crate::runtime::tests::test_state();
    let owner = install(&state, &nominal_zoom());
    let session = SessionId::new();
    state.programming.start(session);
    publish_native(&state, owner, 0.5);
    let lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner])
        .lease
        .unwrap();
    let environment = adopt(&state, session, owner, false, Some(live(lease)));
    assert_eq!(environment.displayed_source_hold, None);
    assert!(
        !matches!(
            environment.current_values.get(&(owner, zoom_key())),
            Some(AttributeValue::Zoom(_))
        ),
        "no convention: the percentage is never reinterpreted as degrees"
    );
    let _ = std::fs::remove_dir_all(directory);
}

// ---------------------------------------------------------------------------------------------
// End to end through `POST /api/v2/programmer/values/actions` on the semantic contract.

struct Desk {
    state: AppState,
    app: Router,
    token: String,
    session: SessionId,
    programmers: ProgrammerRegistry,
    clock: Arc<ManualClock>,
    fixtures: [FixtureId; 2],
    directory: std::path::PathBuf,
}

impl Desk {
    async fn new(profile: FixtureProfile) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(4_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let (state, directory) = crate::runtime::tests::test_state_with_family_adapters(
            programmers.clone(),
            Some(clock.clone()),
            light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        );
        let fixtures = [FixtureId::new(), FixtureId::new()];
        let prepared = state
            .output
            .prepare_snapshot(EngineSnapshot {
                fixtures: (1u32..)
                    .zip([(fixtures[0], 1u16), (fixtures[1], 20)])
                    .map(|(number, (id, address))| {
                        let mut fixture = patched(&profile, id, address);
                        fixture.fixture_number = Some(number);
                        fixture
                    })
                    .collect::<Vec<_>>()
                    .into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        state
            .output
            .finalize_snapshot(&state.playback.render_capability(), prepared, || Ok(()))
            .unwrap();
        let app = crate::runtime::http_router::build(state.clone());
        let (token, session) = login(&app).await;
        Self {
            state,
            app,
            token,
            session,
            programmers,
            clock,
            fixtures,
            directory,
        }
    }

    fn publish(&self) {
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
        self.state
            .output
            .render_frames_and_publish(&rendered, VisualizationScope { show_id: None });
    }

    async fn send(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn lease(&self) -> u64 {
        let ids = self
            .fixtures
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let (status, snapshot) = self
            .send(
                "GET",
                &format!("/api/v2/output/readouts?lane=normal&fixture_ids={ids}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{snapshot}");
        snapshot["lease"]
            .as_u64()
            .expect("the accepted frame is leased")
    }

    async fn apply(&self, request: &str, action: Value) -> (StatusCode, Value) {
        let body = json!({
            "request_id": request,
            "expected_revision": self.programmers.normal_values_revision(),
            "expected_capture_mode_revision": self.programmers.capture_mode_revision(),
            "action": action,
        });
        self.send("POST", "/api/v2/programmer/values/actions", Some(body))
            .await
    }

    fn zoom_edit(&self, operation: Value, lease: u64, undo_group: &str) -> Value {
        json!({
            "type": "apply_intent",
            "fixture_ids": self.fixtures.iter().map(|id| id.0).collect::<Vec<_>>(),
            "attribute": "zoom",
            "operation": {"type": "component_edits", "edits": [{
                "kind": "scalar",
                "component": {"kind": "zoom"},
                "operation": operation,
            }]},
            "undo_group": undo_group,
            "displayed_source": {"lane": "normal", "lease": lease},
        })
    }

    fn zoom(&self, fixture: FixtureId) -> Option<AttributeValue> {
        self.programmers
            .get(self.session)?
            .values
            .iter()
            .find(|v| v.fixture_id == fixture && v.attribute == zoom_key())
            .map(|v| v.value.clone())
    }
}

async fn login(app: &Router) -> (String, SessionId) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (
        value["token"].as_str().unwrap().into(),
        SessionId(Uuid::parse_str(value["session_id"].as_str().unwrap()).unwrap()),
    )
}

#[tokio::test]
async fn a_first_zoom_edit_from_scratch_is_accepted_from_the_displayed_output() {
    let desk = Desk::new(beam_zoom()).await;
    // Nothing is programmed: the output shows the profile default (raw 0 = 10° Beam).
    desk.publish();
    let lease = desk.lease().await;
    let revision = desk.programmers.normal_values_revision();
    let (status, first) = desk
        .apply(
            "zoom-1",
            desk.zoom_edit(
                json!({"kind": "relative", "value": 1.0}),
                lease,
                "zoom-turn",
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "changed", "{first}");
    assert!(first.get("hold").is_none(), "{first}");
    for fixture in desk.fixtures {
        assert_eq!(desk.zoom(fixture), Some(beam(11.)), "adopted 10° + 1°");
    }
    assert_eq!(desk.programmers.normal_values_revision(), revision + 1);

    // An absolute first edit from scratch is accepted too and keeps the measured convention.
    desk.apply(
        "finish",
        json!({"type": "finish_gesture", "attribute": "zoom", "undo_group": "zoom-turn"}),
    )
    .await;
    let (status, set) = desk
        .apply(
            "zoom-2",
            desk.zoom_edit(
                json!({"kind": "set", "value": {"kind": "value", "value": 20.0}}),
                desk.lease().await,
                "zoom-set",
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["status"], "changed", "{set}");
    assert_eq!(desk.zoom(desk.fixtures[0]), Some(beam(20.)));
    let _ = std::fs::remove_dir_all(&desk.directory);
}

#[tokio::test]
async fn an_unmeasurable_first_zoom_edit_holds_quietly() {
    let desk = Desk::new(nominal_zoom()).await;
    desk.publish();
    let lease = desk.lease().await;
    let revision = desk.programmers.normal_values_revision();
    let (status, held) = desk
        .apply(
            "zoom-unknown",
            desk.zoom_edit(
                json!({"kind": "relative", "value": 1.0}),
                lease,
                "zoom-turn",
            ),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a quiet hold, never an error: {held}"
    );
    assert_eq!(held["status"], "no_change", "{held}");
    assert_eq!(held["hold"], "zoom_unavailable", "{held}");
    assert_eq!(desk.programmers.normal_values_revision(), revision);
    assert!(
        desk.fixtures
            .iter()
            .all(|fixture| desk.zoom(*fixture).is_none())
    );
    let _ = std::fs::remove_dir_all(&desk.directory);
}
