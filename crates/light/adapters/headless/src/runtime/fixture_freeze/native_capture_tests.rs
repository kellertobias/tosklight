//! Actual accepted-output Freeze capture through active-show patch, storage and Undo.
//! Synthetic physical profiles demonstrate native ownership, not measured lamp motion.
use super::*;
use crate::runtime::{router, visualization_frame::RenderedSemanticFrame};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use light_application::ActionSource;
use light_core::{MergeMode, TimedValue};
use light_engine::{ContributionBatch, ContributionSample};
use light_fixture::*;
use light_wire::v2::visualization::VisualizationScope;
use serde_json::{Value, json};
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

fn channel(head: Uuid, name: &str, slot: u16) -> FixtureChannel {
    let attribute = AttributeKey(name.into());
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute: attribute.clone(),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U16,
        secondary_slots: vec![slot + 1],
        default_raw: 32768,
        highlight_raw: 65535,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(name, attribute, 65535)],
    }
}

fn profile(logical: bool, aliases: bool) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "TL-556 accepted native Freeze".into();
    profile.modes[0].heads[0].master_shared = !logical;
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    let names = if aliases {
        ["motor.base.rotation", "motor.head.rotation"]
    } else {
        ["pan", "tilt"]
    };
    for (index, (attribute, role)) in names
        .into_iter()
        .zip([PositionAxisRole::Pan, PositionAxisRole::Tilt])
        .enumerate()
    {
        let mut channel = channel(head, attribute, 1 + 2 * index as u16);
        channel.id = Uuid::new_v4();
        channel.head_id = head;
        channel.attribute = AttributeKey(attribute.into());
        channel.fixture_attribute = channel.attribute.clone();
        channel.resolution = ChannelResolution::U16;
        channel.secondary_slots = vec![2 + 2 * index as u16];
        channel.default_raw = 32768;
        channel.highlight_raw = 65535;
        channel.physical_min = None;
        channel.physical_max = None;
        channel.reacts_to_grand_master = false;
        channel.functions = vec![ChannelFunction::continuous(
            attribute,
            channel.attribute.clone(),
            65535,
        )];
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: -720.,
            physical_max: 720.,
            unit: Some("deg".into()),
        };
        channel.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
        profile.modes[0].channels.push(channel);
    }
    profile.modes[0].splits[0].footprint = 4;
    profile.modes[0].position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 1,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3::default(),
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    profile.modes[0].emitter_heads = vec![EmitterHeadBinding {
        emitter_id: emitter.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![emitter];
    for (index, name) in ["intensity", "color.red", "beam.focus"]
        .into_iter()
        .enumerate()
    {
        let mut value = channel(head, name, 5 + index as u16);
        value.resolution = ChannelResolution::U8;
        value.secondary_slots.clear();
        value.default_raw = 0;
        value.highlight_raw = 255;
        value.functions = vec![ChannelFunction::continuous(
            name,
            value.attribute.clone(),
            255,
        )];
        profile.modes[0].channels.push(value);
    }
    profile.modes[0].splits[0].footprint = 7;
    profile.validate().unwrap();
    profile
}

async fn body(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

struct Rig {
    state: AppState,
    app: Router,
    token: String,
    session: Session,
    root: FixtureId,
    owner: FixtureId,
    copy: Option<Uuid>,
    show_id: light_core::ShowId,
    directory: PathBuf,
    motor_names: [&'static str; 2],
}
impl Rig {
    async fn new(logical: bool, aliases: bool, copy: bool) -> Self {
        let (state, directory) = crate::runtime::tests::test_state();
        let profile = state
            .installation
            .save_fixture_profile(profile(logical, aliases), 0)
            .unwrap();
        let app = router(state.clone());
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v2/sessions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({"username":"Operator"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let token = body(response).await["token"].as_str().unwrap().to_owned();
        let session = state
            .sessions
            .sessions()
            .into_iter()
            .find(|session| session.token == token)
            .unwrap();
        let root = FixtureId::new();
        let copy_id = copy.then(Uuid::new_v4);
        let mut rig = Self {
            state,
            app,
            token,
            session,
            root,
            owner: root,
            copy: copy_id,
            show_id: light_core::ShowId(Uuid::nil()),
            directory,
            motor_names: if aliases {
                ["motor.base.rotation", "motor.head.rotation"]
            } else {
                ["pan", "tilt"]
            },
        };
        let created = rig.post("/api/v2/shows", json!({"request_id":Uuid::new_v4(),"action":{
            "type":"create","name":"TL-556 native Freeze","data_base64":null,"overwrite":false
        }}), None).await;
        rig.show_id = light_core::ShowId(
            Uuid::parse_str(created["result"]["show"]["id"].as_str().unwrap()).unwrap(),
        );
        rig.post("/api/v2/shows", json!({"request_id":Uuid::new_v4(),"action":{
            "type":"open","show_id":rig.show_id.0,"transition":"safe_blackout","transition_millis":null
        }}), None).await;
        let copies = copy_id.map(|id| vec![json!({
            "id":id,"name":"Independently inverted copy","split_patches":[{"split":1,"universe":1,"address":20}],
            "location":{"x":2000,"y":1000,"z":4000},"rotation":{"x":0.,"y":0.,"z":30.},
            "invert_pan":true,"invert_tilt":false
        })]).unwrap_or_default();
        rig.post("/api/v2/patch/fixtures", json!({"request_id":Uuid::new_v4(),"fixtures":[{
            "fixture_id":root.0,"fixture_number":1,"virtual_fixture_number":null,"name":"Accepted Freeze mover",
            "profile_id":profile.id.0,"profile_revision":profile.revision,"mode_id":profile.modes[0].id,
            "split_patches":[{"split":1,"universe":1,"address":1}],"layer_id":"default","direct_control":null,
            "location":{"x":0,"y":0,"z":3000},"rotation":{"x":0.,"y":0.,"z":0.},"multipatch":copies,
            "move_in_black_enabled":true,"move_in_black_delay_millis":0,"highlight_overrides":[]
        }]}), Some(0)).await;
        if logical {
            rig.owner = rig.fixture().logical_heads[0].fixture_id;
        }
        rig.state.programming.select(rig.session.id, [rig.owner]);
        // Test setup has completed show activation. Accepted output publication must exercise the
        // ordinary non-Hold boundary rather than the activation transition's retained old frame.
        rig.state.output.set_transition_hold(false);
        rig
    }

    async fn post(&self, path: &str, payload: Value, patch_revision: Option<u64>) -> Value {
        let mut request = Request::post(path)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header("x-tosk-desk", self.session.desk.id.to_string())
            .header(header::CONTENT_TYPE, "application/json");
        if path.starts_with("/api/v2/patch") {
            request = request.header("x-tosk-show", self.show_id.0.to_string());
        }
        if let Some(revision) = patch_revision {
            request = request.header(header::IF_MATCH, revision.to_string());
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(payload.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let value = body(response).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    fn fixture(&self) -> PatchedFixture {
        self.state
            .output
            .snapshot()
            .fixtures
            .iter()
            .find(|fixture| fixture.fixture_id == self.root)
            .unwrap()
            .clone()
    }
    fn context(&self) -> ActionContext {
        ActionContext::operator(self.session.desk.id, self.session.id.0, ActionSource::Http)
            .with_request_id(Uuid::new_v4().to_string())
    }
    fn render(&self, pan: f32, tilt: f32) -> RenderedSemanticFrame {
        let values = [
            (self.motor_names[0], pan),
            (self.motor_names[1], tilt),
            ("intensity", 0.7),
            ("color.red", 0.4),
            ("beam.focus", 0.3),
        ];
        let samples = values
            .into_iter()
            .map(|(key, amount)| {
                ContributionSample::independent(TimedValue {
                    fixture_id: self.owner,
                    attribute: AttributeKey(key.into()),
                    value: AttributeValue::Normalized(amount),
                    priority: 100,
                    changed_at: chrono::Utc::now(),
                    programmer_order: 0,
                    merge_mode: MergeMode::Ltp,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            })
            .collect::<Vec<_>>();
        RenderedSemanticFrame::untraced(
            self.state
                .output
                .engine()
                .render_with_contribution_batches(
                    Default::default(),
                    &[ContributionBatch::new(samples)],
                )
                .unwrap(),
            Default::default(),
        )
    }
    fn publish(&self, frame: &RenderedSemanticFrame, show_id: Uuid) {
        self.state.output.render_frames_and_publish(
            frame,
            VisualizationScope {
                show_id: Some(show_id),
            },
        );
        assert_eq!(
            self.state
                .output
                .latest_visualization_frame()
                .unwrap()
                .generation,
            frame.rendered.generation
        );
    }
    async fn freeze(
        &self,
        operation: FixtureFreezeOperation,
        families: Vec<FixtureFreezeFamily>,
    ) -> FixtureFreezeActionOutcome {
        let _activation = self.state.active_show.acquire().await;
        self.state
            .programming
            .programmers()
            .serialized(|| {
                apply_selected_with_activation(
                    &self.state,
                    &self.session,
                    &FixtureFreezeLiveActionRequest {
                        operation,
                        families,
                    },
                    &self.context(),
                )
            })
            .unwrap()
    }
    async fn undo(&self) -> Option<bool> {
        let _activation = self.state.active_show.acquire().await;
        self.state
            .programming
            .programmers()
            .serialized(|| undo_latest(&self.state, &self.session, &self.context()))
            .unwrap()
    }
    fn persisted_freeze(&self) -> FixtureFreezeState {
        let entry = self.state.active_show.current().unwrap();
        let store = light_show::ShowStore::open(&entry.path).unwrap();
        let object = store
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|object| object.id == self.root.0.to_string())
            .unwrap();
        object
            .body
            .get("freeze")
            .map(|value| serde_json::from_value(value.clone()).unwrap())
            .unwrap_or_default()
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[tokio::test]
async fn freeze_stores_each_accepted_copy_native_word_and_undo_preserves_exact_payload() {
    let rig = Rig::new(false, false, true).await;
    let frame = rig.render(1., 0.);
    let expected = rig
        .state
        .output
        .engine()
        .position_freeze_from_physical(
            frame.rendered.generation,
            &frame.rendered.physical,
            rig.owner,
        )
        .unwrap();
    assert_eq!(expected.instances.len(), 2);
    assert_eq!(
        expected.instances[0]
            .controls
            .iter()
            .map(|control| control.raw)
            .collect::<Vec<_>>(),
        [65535, 0]
    );
    assert_eq!(expected.instances[1].instance_id, rig.copy.unwrap());
    assert_eq!(
        expected.instances[1]
            .controls
            .iter()
            .map(|control| control.raw)
            .collect::<Vec<_>>(),
        [0, 0]
    );
    rig.publish(&frame, rig.show_id.0);
    let newer = rig.render(0., 1.);
    assert_ne!(
        newer.rendered.physical.instances[0].native_raw,
        frame.rendered.physical.instances[0].native_raw
    );
    let outcome = rig
        .freeze(
            FixtureFreezeOperation::Freeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(outcome.changed);
    assert_eq!(outcome.affected_fixtures, 1);
    let frozen = rig.fixture().freeze;
    assert_eq!(
        frozen.targets[&rig.owner].position_native.as_ref(),
        Some(&expected)
    );
    assert_eq!(rig.persisted_freeze(), frozen);
    let same = rig
        .freeze(
            FixtureFreezeOperation::Freeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(
        !same.changed,
        "idempotent Freeze must not require a new post-patch publication"
    );
    let removed = rig
        .freeze(
            FixtureFreezeOperation::Unfreeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(
        removed.changed,
        "removing a hold must not need a fresh accepted pose"
    );
    assert!(rig.fixture().freeze.is_empty());
    assert_eq!(rig.undo().await, Some(true));
    assert_eq!(rig.fixture().freeze, frozen);
    assert_eq!(rig.persisted_freeze(), frozen);
}

#[tokio::test]
async fn missing_stale_wrong_show_and_empty_selection_are_quiet_without_patch_or_history_changes() {
    let rig = Rig::new(false, true, false).await;
    assert!(rig.state.output.latest_visualization_frame().is_none());
    let before = rig.fixture().freeze;
    let missing = rig
        .freeze(
            FixtureFreezeOperation::Freeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(!missing.changed);
    assert_eq!(missing.affected_fixtures, 0);
    assert_eq!(rig.fixture().freeze, before);
    assert_eq!(rig.undo().await, None);
    let frame = rig.render(1., 0.);
    rig.publish(&frame, Uuid::new_v4());
    let wrong = rig
        .freeze(
            FixtureFreezeOperation::Freeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(!wrong.changed);
    assert_eq!(wrong.patch_revision, missing.patch_revision);
    rig.publish(&frame, rig.show_id.0);
    let mut replacement = rig.state.output.snapshot().as_ref().clone();
    replacement.revision += 1;
    rig.state.output.replace_snapshot(replacement).unwrap();
    let stale = rig
        .freeze(
            FixtureFreezeOperation::Freeze,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(!stale.changed);
    assert_eq!(stale.patch_revision, missing.patch_revision);
    rig.state.programming.select(rig.session.id, []);
    let empty = rig.freeze(FixtureFreezeOperation::Toggle, vec![]).await;
    assert!(!empty.changed);
    assert_eq!(empty.affected_fixtures, 0);
    assert_eq!(empty.patch_revision, missing.patch_revision);
    assert_eq!(rig.fixture().freeze, before);
    assert_eq!(rig.persisted_freeze(), before);
    assert_eq!(rig.undo().await, None);
}

#[tokio::test]
async fn root_and_logical_head_selection_toggles_once_and_captures_custom_motor_aliases() {
    let rig = Rig::new(true, true, false).await;
    assert_ne!(rig.root, rig.owner);
    rig.state
        .programming
        .select(rig.session.id, [rig.root, rig.owner]);
    let frame = rig.render(1., 0.);
    let expected = rig
        .state
        .output
        .engine()
        .position_freeze_from_physical(
            frame.rendered.generation,
            &frame.rendered.physical,
            rig.owner,
        )
        .unwrap();
    rig.publish(&frame, rig.show_id.0);
    let result = rig
        .freeze(
            FixtureFreezeOperation::Toggle,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(result.changed);
    assert_eq!(result.affected_fixtures, 1);
    let frozen = rig.fixture().freeze;
    assert_eq!(frozen.targets.len(), 1);
    let target = &frozen.targets[&rig.owner];
    assert_eq!(target.position_native.as_ref(), Some(&expected));
    assert_eq!(target.families, [FreezeFamily::Position]);
    assert!(
        !target
            .values
            .contains_key(&AttributeKey("motor.base.rotation".into()))
    );
    assert!(
        !target
            .values
            .contains_key(&AttributeKey("motor.head.rotation".into()))
    );
    assert_eq!(rig.persisted_freeze(), frozen);
    let removed = rig
        .freeze(
            FixtureFreezeOperation::Toggle,
            vec![FixtureFreezeFamily::Position],
        )
        .await;
    assert!(removed.changed);
    assert!(
        rig.fixture().freeze.is_empty(),
        "root expansion and explicit head must not toggle twice"
    );
}

#[tokio::test]
async fn full_to_color_family_toggle_drops_native_position_and_unrelated_scalar_holds() {
    let rig = Rig::new(false, true, false).await;
    let frame = rig.render(1., 0.);
    rig.publish(&frame, rig.show_id.0);
    assert!(
        rig.freeze(FixtureFreezeOperation::Freeze, vec![])
            .await
            .changed
    );
    let full = rig.fixture().freeze.targets[&rig.owner].clone();
    assert!(full.full);
    assert!(full.position_native.is_some());
    assert!(full.values.contains_key(&AttributeKey::intensity()));
    assert!(full.values.contains_key(&AttributeKey("beam.focus".into())));
    assert!(
        !full
            .values
            .contains_key(&AttributeKey("motor.base.rotation".into()))
    );
    assert!(
        rig.freeze(
            FixtureFreezeOperation::Toggle,
            vec![FixtureFreezeFamily::Color]
        )
        .await
        .changed
    );
    let partial = rig.fixture().freeze.targets[&rig.owner].clone();
    assert!(!partial.full);
    assert_eq!(partial.families, [FreezeFamily::Color]);
    assert!(partial.position_native.is_none());
    assert!(!partial.values.contains_key(&AttributeKey::intensity()));
    assert!(
        !partial
            .values
            .contains_key(&AttributeKey("beam.focus".into()))
    );
    assert!(
        !partial
            .values
            .contains_key(&AttributeKey("motor.base.rotation".into()))
    );
    assert!(
        !partial
            .values
            .contains_key(&AttributeKey("motor.head.rotation".into()))
    );
    assert!(
        partial
            .values
            .keys()
            .all(|key| FreezeFamily::Color.accepts(key))
    );
    assert_eq!(rig.persisted_freeze(), rig.fixture().freeze);
}

#[path = "native_transport_tests.rs"]
mod native_transport_tests;
