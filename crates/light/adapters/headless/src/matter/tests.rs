use super::*;
use light_playback::{FlashReleaseMode, PlaybackButtonAction, PlaybackFaderMode, PlaybackTarget};

fn definition(number: u16, name: &str, has_fader: bool) -> PlaybackDefinition {
    PlaybackDefinition {
        number,
        name: name.into(),
        target: PlaybackTarget::CueList {
            cue_list_id: light_core::CueListId::new(),
        },
        buttons: [PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn group_definition(number: u16, name: &str, group_id: &str) -> PlaybackDefinition {
    PlaybackDefinition {
        target: PlaybackTarget::Group {
            group_id: group_id.into(),
            initial_master: None,
        },
        ..definition(number, name, true)
    }
}

#[test]
fn endpoint_ids_are_stable_global_page_playback_addresses() {
    assert_eq!(endpoint_id(1, 1), Some(1));
    assert_eq!(endpoint_id(1, 127), Some(127));
    assert_eq!(endpoint_id(2, 1), Some(128));
    assert_eq!(endpoint_id(127, 127), Some(16_129));
    assert_eq!(endpoint_id(0, 1), None);
    assert_eq!(endpoint_id(1, 0), None);
}

#[test]
fn disabled_bridge_exposes_nothing_and_does_not_churn_revision() {
    let adapter = MatterBridgeAdapter::default();
    let page = PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: HashMap::from([(1, 7)]),
        virtual_playbacks: HashMap::new(),
    };
    let first = adapter.reconcile(
        false,
        std::slice::from_ref(&page),
        &[definition(7, "Look", true)],
        &HashMap::new(),
    );
    let second = adapter.reconcile(
        false,
        &[page],
        &[definition(7, "Look", true)],
        &HashMap::new(),
    );
    assert_eq!(first, MatterBridgeStatus::default());
    assert_eq!(second.revision, first.revision);
    assert_eq!(
        adapter.resolve_write(
            1,
            MatterPlaybackWrite {
                on: Some(true),
                level: None,
                color: None,
            }
        ),
        Err(MatterBridgeError::Disabled)
    );
}

#[test]
fn eligible_assigned_playbacks_with_values_are_exposed_in_global_address_order() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [
        PlaybackPage {
            number: 2,
            name: "Second".into(),
            slots: HashMap::from([(1, 20), (3, 30)]),
            virtual_playbacks: HashMap::new(),
        },
        PlaybackPage {
            number: 1,
            name: "First".into(),
            slots: HashMap::from([(2, 10)]),
            virtual_playbacks: HashMap::new(),
        },
    ];
    let values = HashMap::from([
        (10, PlaybackValue::new(0.5, true)),
        (20, PlaybackValue::new(1.0, true)),
        (30, PlaybackValue::new(0.25, true)),
    ]);
    let status = adapter.reconcile(
        true,
        &pages,
        &[
            definition(10, "Half", true),
            definition(20, "Full", true),
            definition(30, "Button only", false),
            definition(40, "Pool only", true),
        ],
        &values,
    );

    assert_eq!(status.transport, MatterTransportState::AdapterReady);
    assert!(!status.commissionable);
    assert_eq!(
        status
            .lights
            .iter()
            .map(|light| (
                light.page,
                light.playback,
                light.playback_number,
                light.level
            ))
            .collect::<Vec<_>>(),
        vec![(1, 2, 10, 127), (2, 1, 20, 254), (2, 3, 30, 64)]
    );
    assert_eq!(status.lights[2].name, "Page 2 Playback 3: Button only");
    assert_eq!(status.lights[2].endpoint_id, endpoint_id(2, 3).unwrap());
    assert!(
        status
            .lights
            .iter()
            .all(|light| light.playback_number != 40)
    );
    assert!(status.limitation.is_some());
}

#[test]
fn eligible_assignment_without_an_authoritative_value_is_not_exposed() {
    let adapter = MatterBridgeAdapter::default();
    let page = PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: HashMap::from([(1, 7), (2, 8)]),
        virtual_playbacks: HashMap::new(),
    };

    let status = adapter.reconcile(
        true,
        &[page],
        &[
            definition(7, "Available", true),
            definition(8, "Missing", true),
        ],
        &HashMap::from([(7, PlaybackValue::new(1.0, true))]),
    );

    assert_eq!(status.lights.len(), 1);
    assert_eq!(status.lights[0].playback_number, 7);
    assert_eq!(status.lights[0].endpoint_id, endpoint_id(1, 1).unwrap());
}

#[test]
fn runtime_reconciliation_publishes_remote_and_tracking_changes_bidirectionally() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [PlaybackPage {
        number: 4,
        name: "Looks".into(),
        slots: HashMap::from([(7, 25)]),
        virtual_playbacks: HashMap::new(),
    }];
    let definitions = [definition(25, "Tracked", true)];
    let active = adapter.reconcile(
        true,
        &pages,
        &definitions,
        &HashMap::from([(25, PlaybackValue::new(0.5, true))]),
    );
    assert!(active.lights[0].on);
    assert_eq!(active.lights[0].level, 127);

    let tracked_off = adapter.reconcile(
        true,
        &pages,
        &definitions,
        &HashMap::from([(25, PlaybackValue::new(0.5, false))]),
    );
    assert!(!tracked_off.lights[0].on);
    assert_eq!(tracked_off.lights[0].level, 0);
    assert!(tracked_off.revision > active.revision);
}

#[test]
fn transport_status_is_commissionable_only_after_network_start_and_open_window() {
    let adapter = MatterBridgeAdapter::default();
    adapter.reconcile(
        true,
        &[PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::from([(1, 7)]),
            virtual_playbacks: HashMap::new(),
        }],
        &[definition(7, "Look", true)],
        &HashMap::new(),
    );
    let pairing = MatterPairingData {
        qr_code: "MT:TEST".into(),
        manual_code: "1234-567-8901".into(),
        discriminator: 42,
    };

    let starting = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
        lifecycle: MatterTransportLifecycle::Starting,
        pairing: Some(pairing.clone()),
        ..MatterTransportSnapshot::default()
    });
    assert_eq!(starting.transport, MatterTransportState::Starting);
    assert!(!starting.commissionable);
    assert!(!starting.network_running);
    assert_eq!(starting.pairing, Some(pairing.clone()));

    let running_without_window = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
        lifecycle: MatterTransportLifecycle::Running,
        network_running: true,
        commissioned: true,
        commissioning_window_open: false,
        commissionable: true,
        pairing: Some(pairing.clone()),
        ..MatterTransportSnapshot::default()
    });
    assert_eq!(
        running_without_window.transport,
        MatterTransportState::Running
    );
    assert!(running_without_window.network_running);
    assert!(running_without_window.commissioned);
    assert!(!running_without_window.commissionable);

    let commissionable = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
        lifecycle: MatterTransportLifecycle::Running,
        network_running: true,
        commissioned: false,
        commissioning_window_open: true,
        commissionable: true,
        pairing: Some(pairing),
        ..MatterTransportSnapshot::default()
    });
    assert!(commissionable.commissionable);
    assert!(commissionable.commissioning_window_open);

    adapter.reconcile(false, &[], &[], &HashMap::new());
    let disabled = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
        lifecycle: MatterTransportLifecycle::Running,
        network_running: true,
        commissioning_window_open: true,
        commissionable: true,
        ..MatterTransportSnapshot::default()
    });
    assert_eq!(disabled.transport, MatterTransportState::Disabled);
    assert!(!disabled.network_running);
    assert!(!disabled.commissionable);
    assert!(disabled.pairing.is_none());
}

#[test]
fn matter_writes_resolve_the_explicit_address_without_a_desk_page() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [PlaybackPage {
        number: 4,
        name: "Looks".into(),
        slots: HashMap::from([(7, 25)]),
        virtual_playbacks: HashMap::new(),
    }];
    adapter.reconcile(
        true,
        &pages,
        &[definition(25, "Look", true)],
        &HashMap::from([(25, PlaybackValue::new(0.5, true))]),
    );
    let endpoint = endpoint_id(4, 7).unwrap();
    let half = adapter
        .resolve_write(
            endpoint,
            MatterPlaybackWrite {
                on: None,
                level: Some(127),
                color: None,
            },
        )
        .unwrap();
    assert_eq!((half.page, half.playback, half.playback_number), (4, 7, 25));
    assert!((half.level - 0.5).abs() < 0.001);

    let off = adapter
        .resolve_write(
            endpoint,
            MatterPlaybackWrite {
                on: Some(false),
                level: Some(254),
                color: None,
            },
        )
        .unwrap();
    assert_eq!(off.level, 0.0);
    assert_eq!(
        adapter.resolve_write(
            endpoint,
            MatterPlaybackWrite {
                on: None,
                level: Some(255),
                color: None,
            }
        ),
        Err(MatterBridgeError::ReservedLevel)
    );
}

#[test]
fn group_playbacks_are_extended_color_lights_and_accept_color_writes() {
    let adapter = MatterBridgeAdapter::default();
    let page = PlaybackPage {
        number: 1,
        name: "Groups".into(),
        slots: HashMap::from([(1, 10), (2, 20)]),
        virtual_playbacks: HashMap::new(),
    };
    let group_color = light_core::Xyz {
        x: 0.4,
        y: 0.5,
        z: 0.1,
    };
    let status = adapter.reconcile(
        true,
        &[page],
        &[
            group_definition(10, "Front", "front"),
            definition(20, "Cues", true),
        ],
        &HashMap::from([
            (
                10,
                PlaybackValue::new(0.5, true).with_color(Some(group_color)),
            ),
            (20, PlaybackValue::new(0.5, true)),
        ]),
    );

    assert_eq!(status.lights[0].kind, MatterLightKind::Color);
    assert!(status.lights[0].color.is_some());
    assert_eq!(status.lights[1].kind, MatterLightKind::Dimmable);
    assert_eq!(status.lights[1].color, None);

    let color_write = MatterColorWrite::ColorTemperature { mireds: 250 };
    let resolved = adapter
        .resolve_write(
            endpoint_id(1, 1).unwrap(),
            MatterPlaybackWrite {
                on: None,
                level: None,
                color: Some(color_write),
            },
        )
        .unwrap();
    assert_eq!(resolved.playback_number, 10);
    assert_eq!(resolved.color, Some(color_write));
    assert_eq!(resolved.level, 0.5);
    assert_eq!(
        adapter.resolve_write(
            endpoint_id(1, 2).unwrap(),
            MatterPlaybackWrite {
                on: None,
                level: None,
                color: Some(color_write),
            }
        ),
        Err(MatterBridgeError::EndpointNotExposed(
            endpoint_id(1, 2).unwrap()
        ))
    );
}

#[test]
fn controller_color_write_and_authoritative_reconcile_share_one_revision() {
    let adapter = MatterBridgeAdapter::default();
    let page = PlaybackPage {
        number: 1,
        name: "Groups".into(),
        slots: HashMap::from([(1, 10)]),
        virtual_playbacks: HashMap::new(),
    };
    let definition = group_definition(10, "Front", "front");
    let initial = adapter.reconcile(
        true,
        std::slice::from_ref(&page),
        std::slice::from_ref(&definition),
        &HashMap::from([(10, PlaybackValue::new(0.5, true))]),
    );
    let write = MatterColorWrite::ColorTemperature { mireds: 250 };
    adapter.apply_color_write(endpoint_id(1, 1).unwrap(), write);
    let after_write = adapter.status();
    assert_eq!(after_write.revision, initial.revision + 1);
    assert_eq!(
        after_write.lights[0].color.unwrap().mode,
        MatterColorMode::ColorTemperature
    );

    let reconciled = adapter.reconcile(
        true,
        &[page],
        &[definition],
        &HashMap::from([(
            10,
            PlaybackValue::new(0.5, true).with_color(Some(write.xyz())),
        )]),
    );
    assert_eq!(reconciled.revision, after_write.revision);
    assert_eq!(
        reconciled.lights[0].color.unwrap().mode,
        MatterColorMode::ColorTemperature
    );
}
