use super::*;
use light_application::{
    ActionContext, ActionSource, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, EventBus, EventDraft, HardwareConnectionNotification, HighlightChange,
    OutputRuntimeChange, OutputRuntimeIdentity, OutputRuntimeProjection, OutputRuntimeScope,
    PatchChange, ProgrammingCaptureModeChange, ProgrammingCaptureModeProjection,
    ProgrammingInteractionChange, ProgrammingValuesChange, ProgrammingValuesProjection,
    SelectiveShowImportChange, SelectiveShowObjectChange, SpeedGroupChange, SpeedGroupId,
    SpeedGroupProjection, VisualizerConnectionNotification,
};
use light_core::{AttributeKey, AttributeValue, ShowId, UserId};
use light_show::PortableShowObjectKey;

#[test]
fn patch_event_delta_uses_the_authoritative_envelope_sequence() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Http);
    let show_id = ShowId(Uuid::from_u128(4));
    let event = bus.publish(EventDraft::patch_changed(
        &context,
        PatchChange {
            show_id,
            show_revision: Default::default(),
            patch_revision: Default::default(),
            fixtures: Vec::new(),
            removed_fixture_ids: Vec::new(),
            profile_revisions: Vec::new(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    let wire::EventPayload::ShowPatchChanged { delta } = event.payload else {
        panic!("expected a show Patch event");
    };
    assert_eq!(event.sequence, 1);
    assert_eq!(delta.show_id, show_id.0);
    assert_eq!(delta.event_sequence, Some(event.sequence));
}

#[test]
fn fixture_library_notification_keeps_audit_revision_and_typed_kind() {
    let bus = EventBus::new(4);
    let event = bus.publish(EventDraft::fixture_library_changed(
        application::FixtureLibraryNotification {
            revision: 17,
            kind: application::FixtureLibraryNotificationKind::Profile,
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a fixture-library delivery");
    };
    assert_eq!(event.sequence, 1);
    assert_eq!(event.desk_id, None);
    assert_eq!(event.class, wire::EventClass::Projection);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Show,
            id: "fixture-library".into(),
        })
    );
    let wire::EventPayload::FixtureLibraryChanged { change } = event.payload else {
        panic!("expected a fixture-library payload");
    };
    assert_eq!(change.revision, 17);
    assert_eq!(change.kind, wire::FixtureLibraryNotificationKind::Profile);
}

#[test]
fn hardware_connection_event_carries_authoritative_state_and_desk_route() {
    let bus = EventBus::new(4);
    let event = bus.publish(EventDraft::hardware_connection_changed(
        HardwareConnectionNotification {
            revision: 18,
            connected: true,
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a hardware-connection delivery");
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: "hardware-connections".into(),
        })
    );
    let wire::EventPayload::HardwareConnectionChanged { change } = event.payload else {
        panic!("expected a hardware-connection payload");
    };
    assert_eq!(change.revision, 18);
    assert!(change.connected);
}

#[test]
fn visualizer_connection_event_carries_authoritative_state() {
    let bus = EventBus::new(4);
    let event = bus.publish(EventDraft::visualizer_connection_changed(
        VisualizerConnectionNotification { connected: true },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a visualizer-connection delivery");
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: "visualizer-connections".into(),
        })
    );
    assert_eq!(event.desk_id, None);
    let wire::EventPayload::VisualizerConnectionChanged { change } = event.payload else {
        panic!("expected a visualizer-connection payload");
    };
    assert!(change.connected);
}

#[test]
fn highlight_event_carries_the_authoritative_state_and_output_route() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let desk_id = Uuid::from_u128(8);
    let event = bus.publish(EventDraft::highlight_changed(
        &context,
        HighlightChange {
            revision: 19,
            desk_id,
            action: Some("next".into()),
            source: Some("osc".into()),
            state: light_programmer::HighlightState {
                active: true,
                mode: light_programmer::HighlightMode::Step,
                output_enabled: true,
                capture_only: false,
                remembered: Vec::new(),
                active_index: None,
                active_fixture: None,
                can_previous: false,
                can_next: true,
                message: None,
            },
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a highlight delivery");
    };
    assert_eq!(event.desk_id, Some(desk_id));
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Output,
            id: "highlight".into(),
        })
    );
    let wire::EventPayload::HighlightChanged { change } = event.payload else {
        panic!("expected a highlight payload");
    };
    assert_eq!(change.revision, 19);
    assert_eq!(change.action.as_deref(), Some("next"));
    assert_eq!(change.source.as_deref(), Some("osc"));
    assert!(change.state.active);
    assert!(change.state.can_next);
}

#[test]
fn show_object_batch_keeps_one_event_and_targeted_raw_deltas() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let show_id = ShowId(Uuid::from_u128(4));
    let object_id = Uuid::from_u128(7);
    let event = bus.publish(EventDraft::active_show_objects_changed(
        &context,
        ActiveShowObjectsChange {
            show_id,
            show_revision: Default::default(),
            changes: vec![
                ActiveShowObjectChange::present(
                    ActiveShowObjectKind::Group,
                    object_id.to_string(),
                    3,
                    serde_json::json!({
                        "id": object_id,
                        "name": "Group",
                        "fixtures": [],
                        "future": {"kept": true}
                    }),
                )
                .unwrap(),
            ],
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    let wire::EventPayload::ShowObjectsChanged { change } = event.payload else {
        panic!("expected a show-object event");
    };
    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.source,
        wire::EventSource::Action {
            source: wire::EventActionSource::Osc
        }
    );
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(
        event.related_objects,
        Some(vec![
            wire::EventObject {
                capability: wire::EventCapability::Show,
                id: format!("objects:{}:kind:group", show_id.0),
            },
            wire::EventObject {
                capability: wire::EventCapability::Show,
                id: format!("objects:{}:kind:group:object:{object_id}", show_id.0),
            },
        ])
    );
    assert_eq!(change.show_id, show_id.0);
    assert_eq!(change.show_revision, 0);
    let wire::ShowObjectChange::Group {
        object_id: wire_object_id,
        body,
        ..
    } = &change.changes[0]
    else {
        panic!("expected a typed group change");
    };
    assert_eq!(wire_object_id, &object_id.to_string());
    assert_eq!(body.as_ref().unwrap()["future"]["kept"], true);
}

#[test]
fn selective_import_maps_exact_raw_changes_and_related_routes() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Http);
    let show_id = ShowId(Uuid::from_u128(4));
    let event = bus.publish(EventDraft::selective_import_applied(
        &context,
        SelectiveShowImportChange {
            show_id,
            show_revision: Default::default(),
            outcomes: Vec::new(),
            objects: vec![SelectiveShowObjectChange {
                key: PortableShowObjectKey::new("group", "1"),
                object_revision: 7,
                body: serde_json::json!({"fixtures":["fixture-1"]}),
            }],
            profiles: Vec::new(),
            managed_assets: Vec::new(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Show,
            id: format!("objects:{}", show_id.0),
        })
    );
    assert!(event.related_objects.as_ref().is_some_and(|objects| {
        objects
            .iter()
            .any(|object| object.id == format!("objects:{}:kind:group:object:1", show_id.0))
    }));
    let wire::EventPayload::SelectiveImportApplied { change } = event.payload else {
        panic!("expected a selective-import event");
    };
    assert_eq!(change.show_id, show_id.0);
    assert_eq!(change.objects[0].kind, "group");
    assert_eq!(change.objects[0].object_revision, 7);
    assert_eq!(change.objects[0].body["fixtures"][0], "fixture-1");
}

#[test]
fn global_output_change_keeps_identity_source_and_correlation() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Extension);
    let event = bus.publish(EventDraft::output_runtime_changed(
        &context,
        OutputRuntimeChange {
            projection: OutputRuntimeProjection {
                scope: OutputRuntimeScope {
                    show_id: Uuid::from_u128(40),
                },
                identity: OutputRuntimeIdentity::GlobalMaster,
                grand_master: 0.6,
                blackout: true,
                revision: 7,
            },
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected an event delivery");
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Output,
            id: "runtime:global-master".into(),
        })
    );
    assert_eq!(
        event.source,
        wire::EventSource::Action {
            source: wire::EventActionSource::Extension
        }
    );
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    let wire::EventPayload::OutputRuntimeChanged { change } = event.payload else {
        panic!("expected an output-runtime event");
    };
    assert_eq!(
        change.projection.identity,
        wire::OutputRuntimeIdentity::GlobalMaster
    );
    assert_eq!(change.projection.revision, 7);
    assert_eq!(change.projection.grand_master, 0.6);
    assert!(change.projection.blackout);
}

#[test]
fn speed_group_change_keeps_exact_object_order_timestamp_and_correlation() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Keyboard);
    let event = bus.publish(EventDraft::speed_groups_changed(
        &context,
        SpeedGroupChange {
            authority_id: Uuid::from_u128(40),
            revision: 7,
            applied_at_millis: 123,
            groups: vec![SpeedGroupProjection {
                group: SpeedGroupId::new(2).unwrap(),
                manual_bpm: 128.5,
                paused: false,
                speed_master_scale: 1.0,
                synchronized_with: Some(SpeedGroupId::new(1).unwrap()),
                phase_origin_millis: 99,
            }],
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Speed Group delivery");
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Playback,
            id: "speed-groups:manual".into(),
        })
    );
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
    let wire::EventPayload::SpeedGroupsChanged { change } = event.payload else {
        panic!("expected a Speed Group event");
    };
    assert_eq!(change.authority_id, Uuid::from_u128(40));
    assert_eq!(change.revision, 7);
    assert_eq!(change.applied_at_millis, 123);
    assert_eq!(
        change.groups[0].group,
        light_wire::v2::speed_group::SpeedGroupId::B
    );
    assert_eq!(
        change.groups[0].synchronized_with,
        Some(light_wire::v2::speed_group::SpeedGroupId::A)
    );
}

#[test]
fn programming_interaction_keeps_exact_desk_scope_and_sparse_payload() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::UserInterface);
    let event = bus.publish(EventDraft::programming_interaction_changed(
        &context,
        ProgrammingInteractionChange::from_components(
            context.desk_id,
            Some(Default::default()),
            None,
        )
        .unwrap(),
    ));
    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programming interaction delivery")
    };
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: format!("programming-command-line:{}", context.desk_id),
        })
    );
    let wire::EventPayload::ProgrammingInteractionChanged { change } = event.payload else {
        panic!("expected a Programming interaction payload")
    };
    let light_wire::v2::command_line::ProgrammingInteractionChange::CommandLine {
        desk_id,
        command_line,
    } = change
    else {
        panic!("expected a command-line-only change")
    };
    assert_eq!(desk_id, context.desk_id);
    assert_eq!(command_line.text, "FIXTURE");
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
}

#[test]
fn combined_programming_change_routes_once_through_both_exact_objects() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let event = bus.publish(EventDraft::programming_interaction_changed(
        &context,
        ProgrammingInteractionChange::from_components(
            context.desk_id,
            Some(Default::default()),
            Some(Default::default()),
        )
        .unwrap(),
    ));
    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programming interaction delivery")
    };
    assert_eq!(
        event.object.as_ref().unwrap().id,
        format!("programming-command-line:{}", context.desk_id)
    );
    assert_eq!(
        event.related_objects.as_ref().unwrap(),
        &[wire::EventObject {
            capability: wire::EventCapability::Desk,
            id: format!("programming-selection:{}", context.desk_id),
        }]
    );
    let wire::EventPayload::ProgrammingInteractionChanged { change } = event.payload else {
        panic!("expected a Programming interaction payload")
    };
    assert!(matches!(
        change,
        light_wire::v2::command_line::ProgrammingInteractionChange::Both { .. }
    ));
}

#[test]
fn programming_values_keep_the_full_projection_and_action_identity() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Osc);
    let user_id = UserId(Uuid::from_u128(3));
    let event = bus.publish(EventDraft::programming_values_changed(
        &context,
        ProgrammingValuesChange {
            delta: light_application::ProgrammingValuesDelta {
                group_values: vec![light_programmer::ProgrammerGroupUpdate {
                    group_id: "2.1".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Normalized(0.75),
                    programmer_order: 9,
                    fade: true,
                    fade_millis: Some(1_000),
                    delay_millis: Some(250),
                }],
                ..Default::default()
            },
            projection: ProgrammingValuesProjection {
                dynamic_values: Vec::new().into(),
                user_id,
                revision: 7,
                fixture_values: Vec::new(),
                group_values: vec![light_programmer::ProgrammerGroupUpdate {
                    group_id: "2.1".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Normalized(0.75),
                    programmer_order: 9,
                    fade: true,
                    fade_millis: Some(1_000),
                    delay_millis: Some(250),
                }],
            }
            .into(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programmer values delivery")
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(event.class, wire::EventClass::Projection);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Programmer,
            id: "programming-values".into(),
        })
    );
    let wire::EventPayload::ProgrammingValuesChanged { change } = event.payload else {
        panic!("expected a Programmer values payload")
    };
    assert_eq!(change.revision, 7);
    let value = &change.group_values[0];
    assert_eq!(value.group_id, "2.1");
    assert_eq!(value.programmer_order, 9);
    assert!(value.fade);
    assert_eq!(value.fade_millis, Some(1_000));
    assert_eq!(value.delay_millis, Some(250));
}

#[test]
fn programming_capture_mode_keeps_its_projection_and_action_identity() {
    let bus = EventBus::new(4);
    let context = context(ActionSource::Http);
    let user_id = UserId(Uuid::from_u128(3));
    let event = bus.publish(EventDraft::programming_capture_mode_changed(
        &context,
        ProgrammingCaptureModeChange {
            projection: ProgrammingCaptureModeProjection {
                user_id,
                revision: 4,
                blind: true,
                preview: false,
                preload_capture_programmer: true,
            }
            .into(),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Programmer capture-mode delivery")
    };
    assert_eq!(event.desk_id, None);
    assert_eq!(event.class, wire::EventClass::Projection);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Replaceable);
    assert_eq!(event.correlation_id, Some(context.correlation_id));
    assert_eq!(
        event.object,
        Some(wire::EventObject {
            capability: wire::EventCapability::Programmer,
            id: "programming-capture-mode".into(),
        })
    );
    let wire::EventPayload::ProgrammingCaptureModeChanged { change } = event.payload else {
        panic!("expected a Programmer capture-mode payload")
    };
    assert_eq!(change.projection.revision, 4);
    assert!(change.projection.blind);
    assert!(!change.projection.preview);
    assert!(change.projection.preload_capture_programmer);
}

#[test]
fn sequence_gaps_always_forward() {
    let Some(wire::EventServerMessage::Gap { gap }) = wire_delivery(
        application::SubscriptionDelivery::Gap(application::SequenceGap {
            after_sequence: 1,
            oldest_available: 3,
            latest_sequence: 4,
        }),
    ) else {
        panic!("sequence gaps must remain visible")
    };
    assert_eq!(gap.after_sequence, 1);
    assert_eq!(gap.oldest_available, 3);
    assert_eq!(gap.latest_sequence, 4);
}

#[test]
fn macro_execution_change_is_a_typed_lossless_desk_delivery() {
    let bus = EventBus::new(4);
    let desk_id = Uuid::from_u128(71);
    let execution_id = Uuid::from_u128(72);
    let event = bus.publish(EventDraft::macro_execution_changed(
        application::CommandMacroExecutionSnapshot {
            execution_id,
            macro_id: Uuid::from_u128(73),
            macro_number: 7,
            macro_name: "Blackout".into(),
            source_revision: 2,
            desk_id,
            session_id: Uuid::from_u128(75),
            state: application::CommandMacroExecutionState::Succeeded,
            line: Some(2),
            statement: Some(2),
            command: Some("GROUP 1 AT 0".into()),
            message: None,
            trigger: application::CommandMacroTrigger::Pool,
            started_at: "2026-08-10T18:00:00Z".into(),
            finished_at: Some("2026-08-10T18:00:01Z".into()),
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Macro execution delivery")
    };
    assert_eq!(event.desk_id, Some(desk_id));
    assert_eq!(event.class, wire::EventClass::CommandOutcome);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Lossless);
    let wire::EventPayload::MacroExecutionChanged { execution } = event.payload else {
        panic!("expected a typed Macro execution payload")
    };
    assert_eq!(execution.execution_id, execution_id);
    assert_eq!(
        execution.state,
        light_wire::v2::macros::MacroExecutionState::Succeeded
    );
}

#[test]
fn timecode_tick_is_a_typed_replaceable_ordered_projection() {
    let bus = EventBus::new(4);
    let id = light_playback::TimecodeId(Uuid::from_u128(70));
    let event = bus.publish(EventDraft::timecode_runtime_changed(
        application::timeline::TimecodeRuntimeChange {
            cause: application::timeline::TimecodeRuntimeChangeCause::Tick { completed_loops: 0 },
            snapshot: application::timeline::TimecodeRuntimeSnapshot {
                timecode_id: id,
                revision: 12,
                transport: light_playback::TimecodeTransportState::Playing,
                frame: light_playback::TimecodeFrame(88),
                duration: light_playback::TimecodeFrame(440),
                reconstructed: light_playback::TimecodeReconstructedState {
                    frame: light_playback::TimecodeFrame(88),
                    cue_lists: Vec::new(),
                    speed_groups: Default::default(),
                    audio_volume: 1.0,
                    audio_players: Vec::new(),
                },
                cue_list_clips: Vec::new(),
                audio_linked: true,
            },
        },
    ));

    let Some(wire::EventServerMessage::Event { event }) =
        wire_delivery(application::SubscriptionDelivery::Event(event))
    else {
        panic!("expected a Timecode runtime delivery")
    };
    assert_eq!(event.class, wire::EventClass::Projection);
    assert_eq!(event.delivery, wire::EventDeliveryPolicy::Replaceable);
    let wire::EventPayload::TimecodeRuntimeChanged { snapshot } = event.payload else {
        panic!("expected a typed Timecode runtime payload")
    };
    assert_eq!(snapshot.timecode_id, id.0);
    assert_eq!(snapshot.revision, 12);
    assert_eq!(snapshot.frame, 88);
}

#[test]
fn dynamic_runtime_event_keeps_exact_instance_controller_and_failure_identity() {
    let dynamic_id = Uuid::from_u128(21);
    let instance_id = Uuid::from_u128(22);
    let controller_id = Uuid::from_u128(23);
    let winning_id = Uuid::from_u128(24);
    let change = wire_dynamic_runtime_change(&application::DynamicRuntimeChange {
        kind: application::DynamicRuntimeEventKind::FailedDependency,
        dynamic_id: Some(dynamic_id),
        runtime_instance_id: Some(instance_id),
        controller_id: Some(controller_id),
        winning_controller_id: Some(winning_id),
        occurred_at_millis: 1_234,
        message: Some("Preset dependency is missing".into()),
    });
    assert_eq!(change.kind, wire::DynamicRuntimeEventKind::FailedDependency);
    assert_eq!(change.dynamic_id, Some(dynamic_id));
    assert_eq!(change.runtime_instance_id, Some(instance_id));
    assert_eq!(change.controller_id, Some(controller_id));
    assert_eq!(change.winning_controller_id, Some(winning_id));
    assert_eq!(change.occurred_at_millis, 1_234);
    assert_eq!(
        change.message.as_deref(),
        Some("Preset dependency is missing")
    );
}

fn context(source: ActionSource) -> ActionContext {
    ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        source,
    )
}
