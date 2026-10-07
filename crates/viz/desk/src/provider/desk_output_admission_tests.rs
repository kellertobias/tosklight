//! Frame identity admission, through the provider's real ingestion and lifecycle.
//!
//! Every test drives a provider whose inbox is fed exactly the messages its connection thread
//! sends — connection states, a scene, desk-output snapshots — and reads the result back from
//! `poll`, so the guard is proven where the Stage gets its picture, not in a helper.

use super::super::{DeskConnection, DeskProvider, Message};
use light_wire::v2::output_control::{
    OutputDmxSnapshot, OutputDmxUniverse, OutputFrameIdentity, OutputNativeInstance,
    OutputNativeLane, OutputPointPose,
};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Instant;
use uuid::Uuid;
use viz_scene::{ConnectionState, ProviderDiagnostics, ProviderEvent, SceneProvider};

/// Pending (Preload) frame admission on the same harness.
#[path = "desk_output_preload_tests.rs"]
mod preload;

/// One patched or unpatched Cameo Root Par 6 on universe 1, in a show with a known identity.
struct Rig {
    profile: Arc<light_fixture::FixtureProfile>,
    mode_id: Uuid,
    patched: bool,
    show: Uuid,
    fixture: Uuid,
    instance: Uuid,
    native_identity: String,
}

fn rig(show: Uuid, patched: bool) -> Rig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = profile
        .modes
        .iter()
        .find(|m| m.channels.len() == 7)
        .unwrap();
    // The identity the server stamps on this instance's native row, computed the way the
    // compiled plan computes it, so a mismatch would show as a rejected source.
    let native_identity = light_fixture::forward::native_instance_identity(
        &profile,
        mode,
        None,
        light_fixture::forward::PositionInstallation {
            calibration: None,
            invert_pan: false,
            invert_tilt: false,
            bracket_degrees: 0.,
        },
        &Default::default(),
        if patched { &[(1, 1, 1)] } else { &[] },
    );
    Rig {
        mode_id: mode.id,
        profile: Arc::new(profile),
        patched,
        show,
        fixture: Uuid::new_v4(),
        instance: Uuid::new_v4(),
        native_identity,
    }
}

impl Rig {
    /// The compiled show. An unpatched fixture is still in it; only the native lane drives it.
    fn plan(&self) -> viz_project::ScenePlan {
        let mut plan = viz_project::compile(&[viz_project::PatchedFixture {
            fixture_id: self.fixture,
            name: "Admission".into(),
            number: Some(1),
            profile: self.profile.clone(),
            mode_id: self.mode_id,
            instances: vec![viz_project::PhysicalInstance {
                instance_id: self.instance,
                name: "Admission".into(),
                split_patches: vec![(1, self.patched.then_some((1, 1)))],
                position: glam::Vec3::ZERO,
                rotation_degrees: glam::Vec3::ZERO,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 0.,
                shaper_angle: None,
                installed_appearance: Default::default(),
                scenery_size_metres: None,
                scenery_options: Default::default(),
                model_scale: 1.,
                color_calibration: None,
                position_calibration: None,
            }],
        }]);
        plan.scene.show_id = Some(self.show);
        plan.scene.source_show_revision = 1;
        plan.scene.revision = 1;
        plan
    }
}

fn stamp(sequence: u64, generation: u64) -> OutputFrameIdentity {
    OutputFrameIdentity {
        generation,
        sequence,
        sampled_at: format!("2026-10-01T12:00:{:02}.000Z", sequence % 60),
        tracking: None,
    }
}

/// What a frame says, so each one is recognisably its own picture.
#[derive(Clone, Copy)]
struct Look {
    slot: u8,
    raw: u32,
    point_z: f32,
}

const A: Look = Look {
    slot: 40,
    raw: 40,
    point_z: -0.5,
};
const B: Look = Look {
    slot: 200,
    raw: 200,
    point_z: -1.5,
};

struct Desk {
    provider: DeskProvider,
    outbox: Sender<Message>,
    rig: Rig,
}

impl Desk {
    fn new(patched: bool) -> Self {
        let (provider, outbox) = DeskProvider::detached(
            DeskConnection {
                values_from_desk_output: true,
                ..DeskConnection::default()
            },
            Instant::now(),
        );
        let mut desk = Self {
            provider,
            outbox,
            rig: rig(Uuid::new_v4(), patched),
        };
        desk.connect();
        desk
    }

    /// The messages a successful `connect_once` sends before its watch loop starts reading.
    fn connect(&mut self) {
        let endpoint = "http://127.0.0.1:5000".to_owned();
        self.send(Message::Connection(ConnectionState::Resolving {
            endpoint: endpoint.clone(),
        }));
        self.send(self.scene_message());
        self.send(Message::Connection(ConnectionState::Connected {
            endpoint,
            revision: 1,
        }));
        let events = self.provider.poll();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ProviderEvent::Snapshot { .. }))
        );
    }

    fn scene_message(&self) -> Message {
        let plan = self.rig.plan();
        Message::Scene {
            plan: Box::new(plan.scene),
            bindings: plan.bindings,
            external_camera: plan.external_camera,
            position_points: plan.position_points,
            mappings: Vec::new(),
            diagnostics: Box::new(ProviderDiagnostics::default()),
        }
    }

    fn send(&self, message: Message) {
        self.outbox.send(message).unwrap();
    }

    fn output(&self, frame: Option<OutputFrameIdentity>, look: Look) -> OutputDmxSnapshot {
        let mut slots = vec![0_u8; viz_dmx::DMX_SLOTS];
        slots[..3].copy_from_slice(&[look.slot, look.slot, look.slot]);
        let point = OutputPointPose {
            fixture_id: self.rig.fixture,
            offset_metres: [0.0, 0.0, look.point_z],
            rotation_degrees: [0.0; 3],
        };
        OutputDmxSnapshot {
            native_protocol: 1,
            revision: 1,
            frame: frame.clone(),
            universes: vec![OutputDmxUniverse { universe: 1, slots }],
            overrides: Vec::new(),
            points: vec![point],
            native: Some(OutputNativeLane {
                show_id: Some(self.rig.show),
                revision: 1,
                frame,
                instances: vec![OutputNativeInstance {
                    fixture_id: self.rig.fixture,
                    instance_id: self.rig.instance,
                    native_identity: self.rig.native_identity.clone(),
                    raw: vec![look.raw, look.raw, look.raw, 0, 0, 0, 255],
                    owned_channels: None,
                }],
                points: vec![point],
            }),
            preload: None,
            preload_status: None,
        }
    }

    /// Deliver snapshots as one drain of the worker's messages, and count presented frames.
    fn deliver(&mut self, outputs: impl IntoIterator<Item = OutputDmxSnapshot>) -> usize {
        for output in outputs {
            self.send(Message::DeskOutput(Box::new(output)));
        }
        self.provider
            .poll()
            .iter()
            .filter(|e| matches!(e, ProviderEvent::Values(_)))
            .count()
    }

    /// The whole picture as the Stage would be handed it.
    fn picture(&self) -> String {
        format!("{:?}", self.provider.values)
    }

    fn point_height(&self) -> f32 {
        self.provider.values.position_points[0].offset_metres[1]
    }

    fn colour(&self) -> [f32; 3] {
        self.provider.values.emitters[0].colour
    }

    fn admission(&self) -> &super::DeskOutputAdmission {
        &self.provider.desk_output_admission
    }
}

/// The newer picture, whichever lane put it there, survives a replay of an older frame from the
/// same epoch: DMX through patched slots, native raw for an unpatched rig, and the 3D Point.
#[test]
fn an_older_stamped_frame_never_replaces_the_newer_dmx_native_or_point_picture() {
    for patched in [true, false] {
        let mut desk = Desk::new(patched);
        assert_eq!(desk.deliver([desk.output(Some(stamp(5, 2)), A)]), 1);
        let first = desk.picture();
        let first_colour = desk.colour();
        assert_eq!(desk.deliver([desk.output(Some(stamp(6, 2)), B)]), 1);
        let newer = desk.picture();
        assert_ne!(newer, first, "the newer frame is a different picture");
        assert_ne!(
            desk.colour(),
            first_colour,
            "patched={patched}: the newer frame reached the emitter"
        );
        assert_eq!(desk.point_height(), -1.5);

        assert_eq!(
            desk.deliver([desk.output(Some(stamp(5, 2)), A)]),
            0,
            "an older frame is not presented"
        );
        assert_eq!(
            desk.picture(),
            newer,
            "patched={patched}: nothing was mutated"
        );
        assert_eq!(desk.admission().stale, 1);

        // A later generation does not make an older publication newer: order is the sequence.
        assert_eq!(desk.deliver([desk.output(Some(stamp(4, 9)), A)]), 0);
        assert_eq!(desk.picture(), newer);
        assert_eq!(desk.admission().stale, 2);
    }
}

/// Within one drain, the one pending slot keeps the newer stamped snapshot rather than whichever
/// was read last; a burst never becomes a queue.
#[test]
fn one_pending_snapshot_keeps_the_newer_frame_of_a_burst() {
    let mut desk = Desk::new(true);
    let burst: Vec<_> = (1..=200)
        .map(|n| desk.output(Some(stamp(n, 1)), if n == 200 { B } else { A }))
        .chain([desk.output(Some(stamp(150, 1)), A)])
        .collect();
    assert_eq!(desk.deliver(burst), 1, "one presented frame for the burst");
    assert_eq!(desk.point_height(), -1.5, "the newest frame of the burst");
    assert_eq!(
        desk.admission().decoded,
        1,
        "only one snapshot was ever held"
    );
}

/// Outer and native Live identities that name different frames are not one picture. Nothing is
/// decoded — not the universes, the native lane or the Points — and nothing is presented.
#[test]
fn incoherent_outer_and_native_identities_are_rejected_before_mutation() {
    let mut desk = Desk::new(true);
    assert_eq!(desk.deliver([desk.output(Some(stamp(5, 2)), A)]), 1);
    let held = desk.picture();

    let mut split = desk.output(Some(stamp(7, 2)), B);
    split.native.as_mut().unwrap().frame = Some(stamp(8, 2));
    assert_eq!(desk.deliver([split]), 0);
    assert_eq!(desk.picture(), held);

    let mut retimed = desk.output(Some(stamp(7, 2)), B);
    retimed
        .native
        .as_mut()
        .unwrap()
        .frame
        .as_mut()
        .unwrap()
        .sampled_at = "later".into();
    assert_eq!(desk.deliver([retimed]), 0);
    assert_eq!(desk.picture(), held);

    // One sequence cannot name two frames within an epoch.
    assert_eq!(desk.deliver([desk.output(Some(stamp(5, 3)), B)]), 0);
    assert_eq!(desk.picture(), held);
    assert_eq!(desk.admission().incoherent, 3);
    assert_eq!(desk.admission().decoded, 1);

    // A coherent newer frame is still accepted afterwards: rejection held nothing back.
    assert_eq!(desk.deliver([desk.output(Some(stamp(9, 2)), B)]), 1);
    assert_eq!(desk.point_height(), -1.5);

    // A single supplied identity speaks for the whole Live picture, outer or native.
    let mut outer_only = desk.output(Some(stamp(8, 2)), A);
    outer_only.native.as_mut().unwrap().frame = None;
    assert_eq!(
        desk.deliver([outer_only]),
        0,
        "older by its outer stamp alone"
    );
    let mut native_only = desk.output(None, A);
    native_only.native.as_mut().unwrap().frame = Some(stamp(8, 2));
    assert_eq!(
        desk.deliver([native_only]),
        0,
        "older by its native stamp alone"
    );
    assert_eq!(desk.point_height(), -1.5);
}

/// The very frame already applied is not decoded or presented again; the same frame read through
/// a changed scene or a changed preload is still a new picture.
#[test]
fn a_duplicate_stamped_frame_is_neither_decoded_nor_presented_again() {
    let mut desk = Desk::new(true);
    assert_eq!(desk.deliver([desk.output(Some(stamp(3, 1)), A)]), 1);
    let picture = desk.picture();
    for _ in 0..5 {
        assert_eq!(desk.deliver([desk.output(Some(stamp(3, 1)), A)]), 0);
    }
    assert_eq!(desk.picture(), picture);
    assert_eq!(desk.admission().decoded, 1, "no decode for a duplicate");
    assert_eq!(desk.admission().held, 5);

    // Following the preload changes what the same Live frame looks like.
    desk.provider.follow_preload(true);
    let mut with_preload = desk.output(Some(stamp(3, 1)), A);
    with_preload.preload = Some(OutputNativeLane {
        frame: None,
        ..with_preload.native.clone().unwrap()
    });
    with_preload.preload.as_mut().unwrap().instances[0].raw = vec![255, 0, 0, 0, 0, 0, 255];
    assert_eq!(desk.deliver([with_preload]), 1);
    assert_eq!(desk.admission().decoded, 2);
}

/// Preload is its own evaluation. Its identity — absent today, or its own stamp — is never
/// compared with Live, never required to agree with it, and never replaced by it.
#[test]
fn independent_preload_identity_is_neither_compared_with_nor_given_live_identity() {
    let mut desk = Desk::new(false);
    desk.provider.follow_preload(true);
    let preloaded = |desk: &Desk, live: u64, preload: Option<OutputFrameIdentity>, raw: u32| {
        let mut output = desk.output(Some(stamp(live, 4)), A);
        let mut lane = output.native.clone().unwrap();
        lane.frame = preload;
        lane.instances[0].raw = vec![raw, 0, 0, 0, 0, 0, 255];
        lane.instances[0].owned_channels = Some(vec![true; 7]);
        lane.points[0].offset_metres[2] = -2.5;
        output.preload = Some(lane);
        output
    };

    // An unstamped Preload, and one stamped far behind Live, are both just Preload.
    assert_eq!(desk.deliver([preloaded(&desk, 10, None, 255)]), 1);
    assert_eq!(desk.point_height(), -2.5, "the followed preload Point");
    let unstamped = desk.picture();
    assert_eq!(
        desk.deliver([preloaded(&desk, 11, Some(stamp(2, 99)), 128)]),
        1
    );
    assert_ne!(desk.picture(), unstamped);
    assert_eq!(desk.admission().incoherent, 0);

    // The same Live frame with a different Preload is a new picture, not a duplicate.
    assert_eq!(
        desk.deliver([preloaded(&desk, 11, Some(stamp(3, 99)), 64)]),
        1
    );
    let current = desk.picture();

    // A newer Preload inside an older Live snapshot does not make the snapshot newer.
    assert_eq!(
        desk.deliver([preloaded(&desk, 9, Some(stamp(50, 99)), 255)]),
        0
    );
    assert_eq!(desk.picture(), current);

    // The admitted Live identity is Live's own; the Preload stamp was never adopted.
    // A Preload stamp that happens to coincide with Live's is not checked either way: the frame
    // is admitted and decoded (its picture equals the presented one, so it is not re-presented).
    let decoded = desk.admission().decoded;
    let mut next = preloaded(&desk, 12, None, 64);
    next.preload.as_mut().unwrap().frame = Some(stamp(12, 4));
    assert_eq!(desk.deliver([next]), 0);
    assert_eq!(desk.admission().decoded, decoded + 1);
    assert_eq!(desk.admission().incoherent, 0);
}

/// A producer that stamps nothing proves nothing and keeps today's latest-wins behaviour, legacy
/// normalized Preload overlay included.
#[test]
fn unstamped_and_legacy_payloads_remain_latest_wins() {
    let mut desk = Desk::new(true);
    assert_eq!(desk.deliver([desk.output(None, A)]), 1);
    let a = desk.picture();
    assert_eq!(desk.deliver([desk.output(None, B)]), 1);
    assert_eq!(desk.deliver([desk.output(None, A)]), 1, "no invented proof");
    assert_eq!(desk.point_height(), -0.5);
    assert_eq!(desk.deliver([desk.output(None, A)]), 0, "same picture held");
    assert_ne!(desk.picture(), String::new());
    let _ = a;

    // A legacy, non-native server: universes and Points only, with the normalized overlay.
    let mut legacy = desk.output(None, B);
    legacy.native_protocol = 0;
    legacy.native = None;
    desk.provider.follow_preload(true);
    desk.send(Message::Preload2(Box::new(
        crate::wire::PreloadProjection {
            fixture_values: vec![crate::wire::PreloadFixtureValue {
                fixture_id: desk.rig.fixture,
                attribute: "intensity".into(),
                value: crate::wire::PreloadAttributeValue::Normalized(0.5),
            }],
        },
    )));
    assert_eq!(desk.deliver([legacy.clone()]), 1);
    assert_eq!(desk.point_height(), -1.5);

    // Stamped frames, then an unstamped one in the same epoch: accepted, the stamp unchanged.
    assert_eq!(desk.deliver([desk.output(Some(stamp(20, 1)), A)]), 1);
    assert_eq!(desk.deliver([desk.output(None, B)]), 1);
    assert_eq!(desk.deliver([desk.output(Some(stamp(19, 1)), A)]), 0);
    assert_eq!(desk.point_height(), -1.5);
}

/// A reconnect may reach a restarted server whose counters began again. Its frames are not
/// compared with the last connection's, a snapshot still waiting from that connection is not
/// applied to the new scene, and the first picture is presented even if it looks the same.
#[test]
fn reconnect_resets_acceptance_without_comparing_generations_across_epochs() {
    let mut desk = Desk::new(true);
    assert_eq!(desk.deliver([desk.output(Some(stamp(500, 70)), A)]), 1);

    // Read from the old connection, then the worker reconnects within the same drain.
    desk.send(Message::DeskOutput(Box::new(
        desk.output(Some(stamp(501, 70)), B),
    )));
    desk.send(Message::Connection(ConnectionState::Stale {
        endpoint: "http://127.0.0.1:5000".into(),
        reason: "the configuration event stream closed".into(),
    }));
    desk.connect();
    assert_eq!(
        desk.admission().decoded,
        1,
        "the old connection's snapshot is not decoded into the new scene"
    );

    // The restarted server's first frame has a lower sequence and generation, and is accepted;
    // its picture equals what was presented before, and is presented again into rebuilt values.
    assert_eq!(desk.deliver([desk.output(Some(stamp(1, 1)), A)]), 1);
    assert_eq!(desk.point_height(), -0.5);
    assert_eq!(desk.admission().stale, 0);
    // And order is enforced again inside the new epoch.
    assert_eq!(desk.deliver([desk.output(Some(stamp(2, 1)), B)]), 1);
    assert_eq!(desk.deliver([desk.output(Some(stamp(1, 1)), A)]), 0);
    assert_eq!(desk.point_height(), -1.5);
}

/// A different show is a different source. A scene replacement resets acceptance; a same-show
/// delta keeps the epoch's order but re-decodes the same frame through the new bindings.
#[test]
fn scene_replacement_resets_acceptance_and_a_delta_keeps_the_epoch_order() {
    let mut desk = Desk::new(true);
    assert_eq!(desk.deliver([desk.output(Some(stamp(40, 3)), B)]), 1);

    // Same show re-read: order still holds, the same frame is decoded through the new scene.
    let mut delta = desk.rig.plan();
    delta.scene.revision = 2;
    desk.send(Message::Delta {
        plan: Box::new(delta.scene),
        bindings: delta.bindings,
        external_camera: delta.external_camera,
        position_points: delta.position_points,
        mappings: Vec::new(),
        diagnostics: Box::new(ProviderDiagnostics::default()),
    });
    assert_eq!(desk.deliver([desk.output(Some(stamp(39, 3)), A)]), 0);
    assert_eq!(desk.deliver([desk.output(Some(stamp(40, 3)), B)]), 1);
    assert_eq!(desk.admission().decoded, 2);

    // A different show, as the worker stages it after a show change: a whole new scene.
    desk.rig = rig(Uuid::new_v4(), true);
    desk.send(desk.scene_message());
    assert_eq!(desk.deliver([desk.output(Some(stamp(3, 1)), A)]), 1);
    assert_eq!(desk.point_height(), -0.5);
    assert_eq!(
        desk.admission().stale,
        1,
        "only the delta-epoch replay was stale"
    );
}
