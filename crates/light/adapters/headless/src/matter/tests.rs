use super::*;
use light_playback::{
    FlashReleaseMode, PlaybackButtonAction, PlaybackDefinition, PlaybackFaderMode, PlaybackTarget,
};

fn address(page: u8, position: u16) -> PlaybackAddress {
    PlaybackAddress::new(page, position).unwrap()
}
fn definition(name: &str) -> PlaybackDefinition {
    PlaybackDefinition {
        name: name.into(),
        target: PlaybackTarget::CueList {
            cue_list_id: light_core::CueListId::new(),
        },
        buttons: [PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
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
fn page(number: u8, assignments: Vec<(u16, PlaybackDefinition)>) -> PlaybackPage {
    PlaybackPage {
        number,
        name: format!("Page {number}"),
        assignments: assignments.into_iter().collect(),
    }
}
fn group_definition() -> PlaybackDefinition {
    PlaybackDefinition {
        target: PlaybackTarget::Group {
            group_id: "front".into(),
            initial_master: None,
        },
        ..definition("Front")
    }
}

#[test]
fn disabled_bridge_exposes_nothing_and_does_not_churn_revision() {
    let adapter = MatterBridgeAdapter::default();
    let first = adapter.reconcile(false, &[], &HashMap::new());
    let second = adapter.reconcile(false, &[], &HashMap::new());
    assert_eq!(first, MatterBridgeStatus::default());
    assert_eq!(first, second);
    assert_eq!(
        adapter.resolve_write(
            1,
            MatterPlaybackWrite {
                on: Some(true),
                ..Default::default()
            }
        ),
        Err(MatterBridgeError::Disabled)
    );
}

#[test]
fn sparse_extreme_addresses_have_distinct_endpoints_and_values() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [
        page(99, vec![(1000, definition("Last"))]),
        page(
            1,
            vec![(1000, definition("First")), (1, definition("Unexposed"))],
        ),
    ];
    let values = HashMap::from([
        (address(99, 1000), PlaybackValue::new(1.0, true)),
        (address(1, 1000), PlaybackValue::new(0.5, true)),
    ]);
    let status = adapter.reconcile(true, &pages, &values);
    assert_eq!(status.lights.len(), 2);
    assert_eq!(status.lights[0].address, address(1, 1000));
    assert_eq!(status.lights[0].level, 127);
    assert_eq!(status.lights[1].address, address(99, 1000));
    assert_eq!(status.lights[1].level, 254);
    assert_ne!(status.lights[0].endpoint_id, status.lights[1].endpoint_id);
    assert_eq!(
        adapter.reconcile(true, &pages, &values).revision,
        status.revision
    );
}

#[test]
fn removed_addresses_keep_reservations_across_restart_and_readd() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(99, vec![(1000, definition("Look"))])];
    let values = HashMap::from([(address(99, 1000), PlaybackValue::new(0.5, true))]);
    let mut persisted = String::new();
    let first = adapter.reconcile_persisted(true, &pages, &values, None, |value| {
        persisted = value.into();
        Ok(())
    });
    let original = first.lights[0].endpoint_id;
    let saved = persisted.clone();
    adapter.reconcile_persisted(true, &[], &HashMap::new(), Some(&saved), |_| {
        panic!("reservation removal must not rewrite")
    });
    let restarted = MatterBridgeAdapter::default();
    let second = restarted.reconcile_persisted(true, &pages, &values, Some(&saved), |_| {
        panic!("existing reservation must not rewrite")
    });
    assert_eq!(second.lights[0].endpoint_id, original);
    let extra = [page(1, vec![(1, definition("New"))])];
    let new_values = HashMap::from([(address(1, 1), PlaybackValue::new(1.0, true))]);
    let third = restarted.reconcile_persisted(true, &extra, &new_values, Some(&saved), |_| Ok(()));
    assert_ne!(third.lights[0].endpoint_id, original);
}

#[test]
fn duplicate_or_reserved_saved_endpoints_fail_without_exposure() {
    let pages = [page(1, vec![(1, definition("Look"))])];
    let values = HashMap::from([(address(1, 1), PlaybackValue::new(1.0, true))]);
    for entries in [
        vec![MatterEndpointAllocation {
            address: address(1, 1),
            endpoint_id: 0,
        }],
        vec![MatterEndpointAllocation {
            address: address(1, 1),
            endpoint_id: 0xfffe,
        }],
        vec![
            MatterEndpointAllocation {
                address: address(1, 1),
                endpoint_id: 1,
            },
            MatterEndpointAllocation {
                address: address(2, 1),
                endpoint_id: 1,
            },
        ],
        vec![
            MatterEndpointAllocation {
                address: address(1, 1),
                endpoint_id: 1,
            },
            MatterEndpointAllocation {
                address: address(1, 1),
                endpoint_id: 2,
            },
        ],
    ] {
        let adapter = MatterBridgeAdapter::default();
        let saved = serde_json::to_string(&entries).unwrap();
        let status = adapter.reconcile_persisted(true, &pages, &values, Some(&saved), |_| {
            panic!("invalid saved mapping must not be overwritten")
        });
        assert_eq!(status.transport, MatterTransportState::Failed);
        assert!(status.lights.is_empty());
        assert!(status.limitation.unwrap().contains("duplicate"));
    }
}

#[test]
fn capacity_exhaustion_never_wraps_or_steals_a_reserved_endpoint() {
    let saved = (1..0xfffe)
        .map(|id| MatterEndpointAllocation {
            address: address(((id - 1) / 1000 + 1) as u8, (id - 1) % 1000 + 1),
            endpoint_id: id,
        })
        .collect::<Vec<_>>();
    let pages = [page(99, vec![(1000, definition("New"))])];
    let values = HashMap::from([(address(99, 1000), PlaybackValue::new(1.0, true))]);
    assert!(
        allocate_endpoints(&pages, &values, &saved)
            .unwrap_err()
            .contains("capacity")
    );
}

#[test]
fn failed_persistence_exposes_nothing_and_can_recover() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(1, vec![(1, definition("Look"))])];
    let values = HashMap::from([(address(1, 1), PlaybackValue::new(1.0, true))]);
    let failure = adapter.reconcile_persisted(true, &pages, &values, None, |_| {
        Err("disk unavailable".into())
    });
    assert!(failure.lights.is_empty());
    assert_eq!(failure.limitation.as_deref(), Some("disk unavailable"));
    let recovered = adapter.reconcile_persisted(true, &pages, &values, None, |_| Ok(()));
    assert_eq!(recovered.transport, MatterTransportState::AdapterReady);
    assert_eq!(recovered.lights.len(), 1);
}

#[test]
fn values_are_clamped_nonfinite_and_inactive_values_are_off() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(
        1,
        (1..=4)
            .map(|position| (position, definition("Look")))
            .collect(),
    )];
    let values = HashMap::from([
        (address(1, 1), PlaybackValue::new(2.0, true)),
        (address(1, 2), PlaybackValue::new(-1.0, true)),
        (address(1, 3), PlaybackValue::new(f32::NAN, true)),
        (address(1, 4), PlaybackValue::new(1.0, false)),
    ]);
    let status = adapter.reconcile(true, &pages, &values);
    assert_eq!(
        status
            .lights
            .iter()
            .map(|light| light.level)
            .collect::<Vec<_>>(),
        vec![254, 0, 0, 0]
    );
}

#[test]
fn writes_use_exact_address_off_wins_and_reserved_level_is_rejected() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(4, vec![(1000, definition("Look"))])];
    let values = HashMap::from([(address(4, 1000), PlaybackValue::new(0.5, true))]);
    let status = adapter.reconcile(true, &pages, &values);
    let endpoint = status.lights[0].endpoint_id;
    let half = adapter
        .resolve_write(
            endpoint,
            MatterPlaybackWrite {
                level: Some(127),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(half.address, address(4, 1000));
    assert!((half.level - 0.5).abs() < 0.001);
    assert_eq!(
        adapter
            .resolve_write(
                endpoint,
                MatterPlaybackWrite {
                    on: Some(false),
                    level: Some(254),
                    color: None
                }
            )
            .unwrap()
            .level,
        0.0
    );
    assert_eq!(
        adapter.resolve_write(
            endpoint,
            MatterPlaybackWrite {
                level: Some(255),
                ..Default::default()
            }
        ),
        Err(MatterBridgeError::ReservedLevel)
    );
    assert_eq!(
        adapter.resolve_write(endpoint, MatterPlaybackWrite::default()),
        Err(MatterBridgeError::MissingValue)
    );
}

#[test]
fn on_without_level_retains_nonzero_level_or_activates_full() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(
        1,
        vec![(1, definition("Active")), (2, definition("Off"))],
    )];
    let values = HashMap::from([
        (address(1, 1), PlaybackValue::new(0.5, true)),
        (address(1, 2), PlaybackValue::default()),
    ]);
    let status = adapter.reconcile(true, &pages, &values);
    assert_eq!(
        adapter
            .resolve_write(
                status.lights[0].endpoint_id,
                MatterPlaybackWrite {
                    on: Some(true),
                    ..Default::default()
                }
            )
            .unwrap()
            .level,
        0.5
    );
    assert_eq!(
        adapter
            .resolve_write(
                status.lights[1].endpoint_id,
                MatterPlaybackWrite {
                    on: Some(true),
                    ..Default::default()
                }
            )
            .unwrap()
            .level,
        1.0
    );
}

#[test]
fn color_writes_are_only_valid_on_group_master_endpoints() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(
        1,
        vec![(1, group_definition()), (2, definition("Cues"))],
    )];
    let values = HashMap::from([
        (address(1, 1), PlaybackValue::new(0.5, true)),
        (address(1, 2), PlaybackValue::new(0.5, true)),
    ]);
    let status = adapter.reconcile(true, &pages, &values);
    let color = MatterColorWrite::ColorTemperature { mireds: 250 };
    assert_eq!(status.lights[0].kind, MatterLightKind::Color);
    assert_eq!(status.lights[1].color, None);
    let write = MatterPlaybackWrite {
        color: Some(color),
        ..Default::default()
    };
    assert_eq!(
        adapter
            .resolve_write(status.lights[0].endpoint_id, write)
            .unwrap()
            .color,
        Some(color)
    );
    assert_eq!(
        adapter.resolve_write(status.lights[1].endpoint_id, write),
        Err(MatterBridgeError::EndpointNotExposed(
            status.lights[1].endpoint_id
        ))
    );
}

#[test]
fn controller_color_mode_and_authoritative_reconcile_share_one_revision() {
    let adapter = MatterBridgeAdapter::default();
    let pages = [page(1, vec![(1, group_definition())])];
    let values = HashMap::from([(address(1, 1), PlaybackValue::new(0.5, true))]);
    let initial = adapter.reconcile(true, &pages, &values);
    let endpoint = initial.lights[0].endpoint_id;
    let write = MatterColorWrite::ColorTemperature { mireds: 250 };
    adapter.apply_color_write(endpoint, write);
    let changed = adapter.status();
    assert_eq!(changed.revision, initial.revision + 1);
    let values = HashMap::from([(
        address(1, 1),
        PlaybackValue::new(0.5, true).with_color(Some(write.xyz())),
    )]);
    let reconciled = adapter.reconcile(true, &pages, &values);
    assert_eq!(reconciled.revision, changed.revision);
    assert_eq!(
        reconciled.lights[0].color.unwrap().mode,
        MatterColorMode::ColorTemperature
    );
}

#[test]
fn stale_concurrent_settings_do_not_reassign_cached_endpoints() {
    let adapter = MatterBridgeAdapter::default();
    let first_pages = [page(99, vec![(1000, definition("First"))])];
    let first_values = HashMap::from([(address(99, 1000), PlaybackValue::new(1.0, true))]);
    let first = adapter.reconcile_persisted(true, &first_pages, &first_values, None, |_| Ok(()));
    let second_pages = [page(1, vec![(1, definition("Second"))])];
    let second_values = HashMap::from([(address(1, 1), PlaybackValue::new(1.0, true))]);
    let mut saved = String::new();
    let second = adapter.reconcile_persisted(true, &second_pages, &second_values, None, |value| {
        saved = value.into();
        Ok(())
    });
    assert_ne!(first.lights[0].endpoint_id, second.lights[0].endpoint_id);
    let entries: Vec<MatterEndpointAllocation> = serde_json::from_str(&saved).unwrap();
    assert_eq!(entries.len(), 2);
}
