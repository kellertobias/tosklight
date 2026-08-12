use super::super::{
    MatterColorMode, MatterColorState, MatterColorWrite, MatterLightKind, MatterPlaybackLight,
    MatterPlaybackWrite,
};
use super::bridge::{AttributeChanges, BridgeLights};
use super::commissioning::{IDENTITY_FILE, load_or_create_identity, pairing_data};
use super::model::{
    AGGREGATOR_ENDPOINT_ID, EndpointShape, TransportLight, endpoint_shape, matter_string,
    validate_lights,
};
use super::node::build_endpoints;
use super::*;
use rs_matter::dm::Dataver;
use std::fs;
use std::sync::mpsc;

fn light(endpoint_id: u16, name: &str, on: bool, level: u8) -> MatterPlaybackLight {
    MatterPlaybackLight {
        endpoint_id,
        page: 1,
        playback: endpoint_id as u8,
        playback_number: endpoint_id,
        name: name.into(),
        on,
        level,
        kind: super::super::MatterLightKind::Dimmable,
        color: None,
    }
}

fn color_light(endpoint_id: u16, name: &str, on: bool, level: u8) -> MatterPlaybackLight {
    MatterPlaybackLight {
        kind: MatterLightKind::Color,
        color: Some(MatterColorState::default()),
        ..light(endpoint_id, name, on, level)
    }
}

#[test]
fn identity_and_pairing_material_are_stable_in_the_desk_data_directory() {
    let directory = std::env::temp_dir().join(format!("light-matter-{}", uuid::Uuid::new_v4()));
    let first = load_or_create_identity(&directory).unwrap();
    let second = load_or_create_identity(&directory).unwrap();
    assert_eq!(first, second);
    let pairing = pairing_data(&first).unwrap();
    assert!(pairing.qr_code.starts_with("MT:"));
    assert_eq!(pairing.manual_code.len(), 13);
    assert_eq!(pairing.discriminator, first.discriminator);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "binds the standard Matter and mDNS ports"]
fn commissionable_network_transport_smoke() {
    let directory =
        std::env::temp_dir().join(format!("light-matter-smoke-{}", uuid::Uuid::new_v4()));
    let transport = MatterTransport::new(&directory);
    let running = transport.reconcile(true, &[light(1, "First", true, 127)]);
    assert_eq!(
        running.lifecycle,
        MatterTransportLifecycle::Running,
        "{running:?}"
    );
    assert!(running.network_running);
    assert!(running.commissioning_window_open);
    assert!(running.commissionable);
    assert!(running.pairing.is_some());
    assert_eq!(running.endpoint_count, 1);

    let identity_path = directory.join("matter").join(IDENTITY_FILE);
    let identity = fs::read(&identity_path).unwrap();
    let value_only = transport.reconcile(true, &[light(1, "First", true, 64)]);
    assert_eq!(value_only.lifecycle, MatterTransportLifecycle::Running);
    assert_eq!(value_only.endpoint_count, 1);
    assert_eq!(value_only.pairing, running.pairing);

    let after_removal = transport.reconcile(true, &[]);
    assert_eq!(
        after_removal.lifecycle,
        MatterTransportLifecycle::Running,
        "{after_removal:?}"
    );
    assert_eq!(after_removal.endpoint_count, 0);
    assert_eq!(after_removal.pairing, running.pairing);
    assert_eq!(fs::read(identity_path).unwrap(), identity);

    transport.stop();
    assert_eq!(transport.snapshot(), MatterTransportSnapshot::default());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn endpoint_validation_preserves_stable_ids_and_rejects_reserved_or_duplicate_ids() {
    let validated = validate_lights(&[
        light(128, "Second page", false, 0),
        light(1, "First page", true, 254),
    ])
    .unwrap();
    assert_eq!(
        validated
            .iter()
            .map(|light| light.endpoint_id)
            .collect::<Vec<_>>(),
        vec![1, 128]
    );
    assert!(validate_lights(&[light(0, "Root", false, 0)]).is_err());
    assert!(validate_lights(&[light(1, "A", false, 0), light(1, "B", false, 0)]).is_err());
}

#[test]
fn endpoint_metadata_removes_empty_playbacks_without_renumbering_survivors() {
    let initial = build_endpoints(&[
        EndpointShape {
            endpoint_id: 1,
            name: "First".into(),
            kind: super::super::MatterLightKind::Dimmable,
        },
        EndpointShape {
            endpoint_id: 128,
            name: "Second page".into(),
            kind: super::super::MatterLightKind::Dimmable,
        },
    ]);
    assert_eq!(
        initial
            .iter()
            .map(|endpoint| endpoint.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 128, AGGREGATOR_ENDPOINT_ID]
    );

    let after_removal = build_endpoints(&[EndpointShape {
        endpoint_id: 128,
        name: "Second page".into(),
        kind: super::super::MatterLightKind::Dimmable,
    }]);
    assert_eq!(
        after_removal
            .iter()
            .map(|endpoint| endpoint.id)
            .collect::<Vec<_>>(),
        vec![0, 128, AGGREGATOR_ENDPOINT_ID]
    );
}

#[test]
fn endpoint_shape_distinguishes_dimmable_and_extended_color_lights() {
    let dimmable = TransportLight::from(&light(1, "Cues", true, 127));
    let color = TransportLight::from(&color_light(2, "Front", true, 127));
    let shape = endpoint_shape(&[dimmable, color]);
    assert_eq!(shape[0].kind, MatterLightKind::Dimmable);
    assert_eq!(shape[1].kind, MatterLightKind::Color);
    assert_ne!(shape[0], shape[1]);

    let endpoints = build_endpoints(&shape);
    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, AGGREGATOR_ENDPOINT_ID]
    );
}

#[test]
fn endpoint_removal_replaces_runtime_shape_without_touching_persisted_kv_storage() {
    let directory = std::env::temp_dir().join(format!("light-matter-{}", uuid::Uuid::new_v4()));
    // This is deliberately an offline orchestration test, not controller commissioning proof.
    // It verifies the restart seam keeps the rs-matter KV directory that owns fabric state.
    let kv_sentinel = directory
        .join("matter")
        .join("kv")
        .join("persisted-sentinel");
    fs::create_dir_all(kv_sentinel.parent().unwrap()).unwrap();
    fs::write(&kv_sentinel, b"persisted-kv").unwrap();
    let initial_shape = vec![
        EndpointShape {
            endpoint_id: 1,
            name: "First".into(),
            kind: super::super::MatterLightKind::Dimmable,
        },
        EndpointShape {
            endpoint_id: 128,
            name: "Second page".into(),
            kind: super::super::MatterLightKind::Dimmable,
        },
    ];
    let (control, control_rx) = mpsc::channel();
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let join = std::thread::spawn(move || {
        if matches!(control_rx.recv(), Ok(ControlCommand::Shutdown)) {
            let _ = stopped_tx.send(());
        }
    });
    let mut runtime = Some(RuntimeHandle {
        shape: initial_shape.clone(),
        control,
        join,
    });
    let survivor_shape = initial_shape[1..].to_vec();
    let (replacement_control, replacement_rx) = mpsc::channel();
    let replacement_shape = survivor_shape.clone();

    assert!(replace_runtime_for_shape(
        &mut runtime,
        &survivor_shape,
        move || {
            Some(RuntimeHandle {
                shape: replacement_shape,
                control: replacement_control,
                join: std::thread::spawn(move || {
                    let _ = replacement_rx.recv();
                }),
            })
        }
    ));
    stopped_rx.recv().unwrap();
    assert_eq!(runtime.as_ref().unwrap().shape, survivor_shape);
    assert_eq!(fs::read(&kv_sentinel).unwrap(), b"persisted-kv");
    assert!(!replace_runtime_for_shape(
        &mut runtime,
        &survivor_shape,
        || panic!("value-only reconciliation must not replace the runtime")
    ));

    stop_runtime(&mut runtime);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn outbound_tracking_updates_change_only_the_subscription_attributes_that_moved() {
    let (sender, _receiver) = mpsc::channel();
    let handler = BridgeLights::new(
        vec![TransportLight::from(&light(1, "Look", true, 127))],
        sender,
        Dataver::new(1),
        Dataver::new(2),
        Dataver::new(3),
        Dataver::new(4),
    );
    let changes = handler.reconcile(vec![TransportLight::from(&light(1, "Look", false, 0))]);
    assert_eq!(
        changes,
        vec![AttributeChanges {
            endpoint_id: 1,
            on: true,
            level: false,
            color: false,
        }]
    );
    assert_eq!(handler.endpoint(1).unwrap().level, 127);
}

#[test]
fn controller_mutations_are_forwarded_as_onoff_and_level_writes() {
    let (sender, receiver) = mpsc::channel();
    let handler = BridgeLights::new(
        vec![TransportLight::from(&light(128, "Look", false, 0))],
        sender,
        Dataver::new(1),
        Dataver::new(2),
        Dataver::new(3),
        Dataver::new(4),
    );
    handler.set_level(128, 64).unwrap();
    assert_eq!(
        receiver.recv().unwrap(),
        MatterRemoteWrite {
            endpoint_id: 128,
            write: MatterPlaybackWrite {
                on: Some(true),
                level: Some(64),
                color: None,
            },
        }
    );
    handler.set_on(128, false).unwrap();
    assert_eq!(receiver.recv().unwrap().write.on, Some(false));
}

#[test]
fn color_mutations_are_forwarded_and_tracking_reports_only_color_changes() {
    let (sender, receiver) = mpsc::channel();
    let handler = BridgeLights::new(
        vec![TransportLight::from(&color_light(1, "Front", true, 127))],
        sender,
        Dataver::new(1),
        Dataver::new(2),
        Dataver::new(3),
        Dataver::new(4),
    );
    let write = MatterColorWrite::ColorTemperature { mireds: 250 };
    handler.set_color(1, write).unwrap();
    assert_eq!(
        receiver.recv().unwrap().write,
        MatterPlaybackWrite {
            on: None,
            level: None,
            color: Some(write),
        }
    );
    assert_eq!(
        handler.endpoint(1).unwrap().color.unwrap().mode,
        MatterColorMode::ColorTemperature
    );

    let mut tracked = color_light(1, "Front", true, 127);
    tracked.color = Some(MatterColorState::from_xyz(
        light_core::Xyz {
            x: 0.3,
            y: 0.6,
            z: 0.1,
        },
        MatterColorMode::ColorTemperature,
    ));
    assert_eq!(
        handler.reconcile(vec![TransportLight::from(&tracked)]),
        vec![AttributeChanges {
            endpoint_id: 1,
            on: false,
            level: false,
            color: true,
        }]
    );
}

#[test]
fn matter_names_are_valid_utf8_and_fit_the_cluster_limit() {
    assert_eq!(matter_string("A short name", 32), "A short name");
    let truncated = matter_string("Page 127 Playback 127: 🎭🎭🎭🎭", 32);
    assert!(truncated.len() <= 32);
    assert!(truncated.is_char_boundary(truncated.len()));
}
