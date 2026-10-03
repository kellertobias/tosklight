//! TL-554 end to end through the HTTP routes, the real values service, the real Live family
//! frame and the accepted-frame colour report, on a mixed selection of two compatible RGBW heads
//! (Red U8, Green U16, Blue U24, White U32) and one lookalike (same names, attributes, widths and
//! slots; its own UUIDs).
use super::*;
use crate::runtime::output_scheduler::physical_adapters::color::profiles::patched;
use crate::runtime::output_scheduler::physical_adapters::color::tests::direct::{
    catalogue, identity, path_channels, rgbw_widths,
};
use crate::runtime::output_scheduler::physical_adapters::color::tests::{intent, program};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use light_core::programming::{ColorProgram, ProgrammingOwner};
use light_core::{AttributeValue, ManualClock, SessionId};
use light_fixture::FixtureProfile;
use light_programmer::ProgrammerRegistry;
use light_wire::v2::attribute_configuration::ColorIntentReport;
use light_wire::v2::native_color as wire;
use light_wire::v2::output_readouts::OutputReadoutSnapshot;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

pub(super) struct Desk {
    pub state: AppState,
    pub app: Router,
    pub token: String,
    pub session: SessionId,
    pub programmers: ProgrammerRegistry,
    pub clock: Arc<ManualClock>,
    pub profile: FixtureProfile,
    pub lookalike: FixtureProfile,
    /// a1, a2: `profile`; b: `lookalike`.
    pub fixtures: [FixtureId; 3],
    pub directory: std::path::PathBuf,
}

impl Desk {
    pub async fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(4_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let (state, directory) = crate::runtime::tests::test_state_with_family_adapters(
            programmers.clone(),
            Some(clock.clone()),
            light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        );
        let profile = rgbw_widths();
        let lookalike = rgbw_widths();
        let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
        let patch = [
            (&profile, fixtures[0], 1u16),
            (&profile, fixtures[1], 20),
            (&lookalike, fixtures[2], 40),
        ];
        // Finalized like a show activation, so the Live Dynamics runtime resolves the same
        // retained original models as the edit path.
        let prepared = state
            .output
            .prepare_snapshot(light_engine::EngineSnapshot {
                fixtures: (1u32..)
                    .zip(patch)
                    .map(|(number, (profile, id, address))| {
                        let mut fixture = patched(profile, id, address);
                        fixture.fixture_number = Some(number);
                        fixture
                    })
                    .collect::<Vec<_>>()
                    .into(),
                revision: 1,
                native_color_sources: catalogue(&[&profile, &lookalike]),
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
            profile,
            lookalike,
            fixtures,
            directory,
        }
    }

    /// Program a Semantic colour on every fixture and accept one Live frame.
    pub fn program_semantic(&self, rgb: [f32; 3]) {
        for fixture in self.fixtures {
            self.programmers.set(
                self.session,
                fixture,
                ProgrammingOwner::Color.key(),
                program(&intent(rgb, 0.0)),
            );
        }
        self.publish();
    }

    pub fn publish(&self) {
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
    }

    pub async fn get(&self, uri: &str) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::get(uri)
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    pub async fn pages(&self, query: &str) -> wire::NativeColorPagesSnapshot {
        let ids = self.ids(&self.fixtures);
        let response = self
            .get(&format!(
                "/api/v2/programming/color/native-pages?fixture_ids={ids}{query}"
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        json_body(response).await
    }

    pub async fn lease(&self) -> u64 {
        let ids = self.ids(&self.fixtures);
        let snapshot: OutputReadoutSnapshot = json_body(
            self.get(&format!(
                "/api/v2/output/readouts?lane=normal&fixture_ids={ids}"
            ))
            .await,
        )
        .await;
        snapshot.lease.expect("the accepted frame is leased")
    }

    /// The accepted-frame colour report (the route also requires an active show).
    pub async fn report(&self) -> ColorIntentReport {
        crate::runtime::color_intent_report::accepted_frame_report(&self.state, None)
    }

    pub fn ids(&self, fixtures: &[FixtureId]) -> String {
        fixtures
            .iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub async fn apply(&self, request: &str, action: Value) -> Value {
        let body = json!({
            "request_id": request,
            "expected_revision": self.programmers.normal_values_revision(),
            "expected_capture_mode_revision": self.programmers.capture_mode_revision(),
            "action": action,
        });
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v2/programmer/values/actions")
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let value: Value = json_body(response).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        value
    }

    /// One native component edit of channel `index` (path order) on the whole selection.
    pub fn native_edit(&self, index: usize, operation: Value, lease: Option<u64>) -> Value {
        let channel = path_channels(&self.profile)[index];
        let mut action = json!({
            "type": "apply_intent",
            "fixture_ids": self.fixtures.iter().map(|id| id.0).collect::<Vec<_>>(),
            "attribute": "color",
            "operation": {"type": "component_edits", "edits": [{
                "kind": "native",
                "binding": {"channel_id": channel.id, "function_id": channel.functions[0].id},
                "operation": operation,
            }]},
            "undo_group": "native-turn",
            "native_reference": {
                "fixture_id": self.fixtures[0].0,
                "head_id": self.profile.modes[0].heads[0].id,
            },
        });
        if let Some(lease) = lease {
            action["displayed_source"] = json!({"lane": "normal", "lease": lease});
        }
        action
    }

    pub fn value(&self, fixture: FixtureId) -> Option<AttributeValue> {
        self.programmers
            .get(self.session)?
            .values
            .iter()
            .find(|v| v.fixture_id == fixture && v.attribute == ProgrammingOwner::Color.key())
            .map(|v| v.value.clone())
    }

    /// The Direct recipe raws of `fixture`, in path order.
    pub fn recipe(&self, fixture: FixtureId) -> Vec<u32> {
        let Some(AttributeValue::ColorProgram(program)) = self.value(fixture) else {
            panic!("a Color program")
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            panic!("Direct expected, found {program:?}")
        };
        assert_eq!(
            recipe.source,
            identity(&self.profile),
            "the reference source"
        );
        path_channels(&self.profile)
            .iter()
            .map(|channel| {
                recipe
                    .channels
                    .iter()
                    .find(|value| value.channel_id == channel.id)
                    .expect("every participating channel")
                    .raw
            })
            .collect()
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
    let value: Value = json_body(response).await;
    (
        value["token"].as_str().unwrap().into(),
        SessionId(uuid::Uuid::parse_str(value["session_id"].as_str().unwrap()).unwrap()),
    )
}

pub(super) async fn json_body<T: serde::de::DeserializeOwned>(response: Response) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&bytes)))
}

#[path = "native_color_pages_tests/adoption.rs"]
mod adoption;
#[path = "native_color_pages_tests/pages.rs"]
mod pages;
