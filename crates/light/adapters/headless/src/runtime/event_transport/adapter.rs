//! Translation between transport-independent application events and v2 wire DTOs.

mod selective_import;

use light_application as application;
use light_wire::v2::events as wire;

#[cfg(test)]
use uuid::Uuid;

use super::super::Session;

pub(super) fn application_rate_limits(
    limits: Vec<wire::EventRateLimit>,
) -> Vec<application::ReplaceableEventRateLimit> {
    limits
        .into_iter()
        .map(|limit| application::ReplaceableEventRateLimit {
            capability: app_capability(limit.capability),
            class: app_class(limit.class),
            object: limit.object.map(app_object),
            min_interval: std::time::Duration::from_millis(limit.min_interval_millis),
        })
        .collect()
}

pub(super) fn application_filter(
    session: &Session,
    filter: wire::EventSubscriptionFilter,
) -> application::EventFilter {
    application::EventFilter {
        desk_id: Some(session.desk.id),
        programmer_user_id: Some(session.user.id.0),
        capabilities: filter
            .capabilities
            .into_iter()
            .map(app_capability)
            .collect(),
        classes: filter.classes.into_iter().map(app_class).collect(),
        objects: filter.objects.into_iter().map(app_object).collect(),
    }
}

fn app_object(object: wire::EventObject) -> application::EventObject {
    application::EventObject::new(app_capability(object.capability), object.id)
}

fn app_capability(capability: wire::EventCapability) -> application::EventCapability {
    use application::EventCapability as App;
    match capability {
        wire::EventCapability::Programmer => App::Programmer,
        wire::EventCapability::Playback => App::Playback,
        wire::EventCapability::Show => App::Show,
        wire::EventCapability::Desk => App::Desk,
        wire::EventCapability::Output => App::Output,
        wire::EventCapability::System => App::System,
    }
}

fn app_class(class: wire::EventClass) -> application::EventClass {
    use application::EventClass as App;
    match class {
        wire::EventClass::Transition => App::Transition,
        wire::EventClass::Projection => App::Projection,
        wire::EventClass::CommandOutcome => App::CommandOutcome,
        wire::EventClass::Error => App::Error,
        wire::EventClass::Safety => App::Safety,
        wire::EventClass::Telemetry => App::Telemetry,
    }
}

pub(super) fn wire_delivery(
    delivery: application::SubscriptionDelivery,
) -> Option<wire::EventServerMessage> {
    match delivery {
        application::SubscriptionDelivery::Event(event) => {
            wire_event(&event).map(|event| wire::EventServerMessage::Event {
                event: Box::new(event),
            })
        }
        application::SubscriptionDelivery::Gap(gap) => {
            Some(wire::EventServerMessage::Gap { gap: wire_gap(gap) })
        }
    }
}

pub(super) fn wire_gap(gap: application::SequenceGap) -> wire::SequenceGap {
    wire::SequenceGap {
        after_sequence: gap.after_sequence,
        oldest_available: gap.oldest_available,
        latest_sequence: gap.latest_sequence,
    }
}

fn wire_event(event: &application::EventEnvelope) -> Option<wire::EventEnvelope> {
    Some(wire::EventEnvelope {
        sequence: event.sequence,
        occurred_at: event.occurred_at.to_rfc3339(),
        desk_id: event.desk_id,
        class: wire_class(event.class),
        object: event.object.as_ref().map(wire_object),
        related_objects: (!event.related_objects.is_empty())
            .then(|| event.related_objects.iter().map(wire_object).collect()),
        source: wire_source(event.source),
        correlation_id: event.correlation_id,
        delivery: wire_delivery_policy(event.delivery),
        payload: wire_payload(&event.payload, event.sequence)?,
    })
}

fn wire_object(object: &application::EventObject) -> wire::EventObject {
    wire::EventObject {
        capability: wire_capability(object.capability),
        id: object.id.clone(),
    }
}

fn wire_capability(capability: application::EventCapability) -> wire::EventCapability {
    use application::EventCapability as App;
    match capability {
        App::Programmer => wire::EventCapability::Programmer,
        App::Playback => wire::EventCapability::Playback,
        App::Show => wire::EventCapability::Show,
        App::Desk => wire::EventCapability::Desk,
        App::Output => wire::EventCapability::Output,
        App::System => wire::EventCapability::System,
    }
}

fn wire_class(class: application::EventClass) -> wire::EventClass {
    use application::EventClass as App;
    match class {
        App::Transition => wire::EventClass::Transition,
        App::Projection => wire::EventClass::Projection,
        App::CommandOutcome => wire::EventClass::CommandOutcome,
        App::Error => wire::EventClass::Error,
        App::Safety => wire::EventClass::Safety,
        App::Telemetry => wire::EventClass::Telemetry,
    }
}

fn wire_delivery_policy(policy: application::DeliveryPolicy) -> wire::EventDeliveryPolicy {
    match policy {
        application::DeliveryPolicy::Lossless => wire::EventDeliveryPolicy::Lossless,
        application::DeliveryPolicy::Replaceable => wire::EventDeliveryPolicy::Replaceable,
    }
}

fn wire_source(source: application::EventSource) -> wire::EventSource {
    match source {
        application::EventSource::Runtime => wire::EventSource::Runtime,
        application::EventSource::Action(source) => wire::EventSource::Action {
            source: wire_action_source(source),
        },
    }
}

fn wire_action_source(source: application::ActionSource) -> wire::EventActionSource {
    use application::ActionSource as App;
    match source {
        App::UserInterface => wire::EventActionSource::UserInterface,
        App::Keyboard => wire::EventActionSource::Keyboard,
        App::Osc => wire::EventActionSource::Osc,
        App::Http => wire::EventActionSource::Http,
        App::Midi => wire::EventActionSource::Midi,
        App::Matter => wire::EventActionSource::Matter,
        App::Cue => wire::EventActionSource::Cue,
        App::Timecode => wire::EventActionSource::Timecode,
        App::Scheduler => wire::EventActionSource::Scheduler,
        App::Macro => wire::EventActionSource::Macro,
        App::System => wire::EventActionSource::System,
    }
}

fn wire_payload(
    payload: &application::ApplicationEvent,
    sequence: u64,
) -> Option<wire::EventPayload> {
    Some(match payload {
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::InteractionChanged(change),
        ) => wire::EventPayload::ProgrammingInteractionChanged {
            change: super::super::command_http::interaction_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::ValuesChanged(change),
        ) => wire::EventPayload::ProgrammingValuesChanged {
            change: super::super::command_http::values_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::PriorityChanged(change),
        ) => wire::EventPayload::ProgrammerPriorityChanged {
            change: super::super::command_http::priority_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::CaptureModeChanged(change),
        ) => wire::EventPayload::ProgrammingCaptureModeChanged {
            change: super::super::command_http::capture_mode_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::PreloadValuesChanged(change),
        ) => wire::EventPayload::ProgrammingPreloadValuesChanged {
            change: super::super::command_http::preload_values_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::PreloadPlaybackQueueChanged(change),
        ) => wire::EventPayload::ProgrammingPreloadPlaybackQueueChanged {
            change: super::super::command_http::preload_playback_queue_change(change),
        },
        application::ApplicationEvent::Programming(
            application::ProgrammingEvent::LifecycleChanged(change),
        ) => wire::EventPayload::ProgrammingLifecycleChanged {
            change: super::super::command_http::lifecycle_change(change),
        },
        application::ApplicationEvent::Playback(application::PlaybackEvent::RuntimeChanged(
            change,
        )) => wire::EventPayload::PlaybackRuntimeChanged {
            change: super::super::playback_v2::runtime_change(change),
        },
        application::ApplicationEvent::Playback(application::PlaybackEvent::TelemetrySampled(
            tick,
        )) => wire::EventPayload::PlaybackTelemetrySampled {
            tick: super::super::playback_v2::telemetry_tick(tick),
        },
        application::ApplicationEvent::Playback(
            application::PlaybackEvent::SpeedGroupsChanged(change),
        ) => wire::EventPayload::SpeedGroupsChanged {
            change: super::super::speed_group_v2::wire_change(change),
        },
        application::ApplicationEvent::Desk(event) => match event {
            application::DeskEvent::PlaybackViewChanged(projection) => {
                wire::EventPayload::PlaybackViewChanged {
                    projection: super::super::playback_v2::desk_projection(*projection),
                }
            }
            application::DeskEvent::ConfigurationChanged(change) => {
                wire::EventPayload::ServerConfigurationChanged {
                    change: wire_revision(*change),
                }
            }
            application::DeskEvent::ScreensChanged(change) => wire::EventPayload::ScreensChanged {
                change: wire_screen_notification(*change),
            },
            application::DeskEvent::HardwareConnectionChanged(change) => {
                wire::EventPayload::HardwareConnectionChanged {
                    change: wire::HardwareConnectionNotification {
                        revision: change.revision,
                        connected: change.connected,
                    },
                }
            }
        },
        application::ApplicationEvent::Output(event) => match event {
            application::OutputEvent::RuntimeChanged(change) => {
                wire::EventPayload::OutputRuntimeChanged {
                    change: super::super::output_runtime_v2::wire_change(*change),
                }
            }
            application::OutputEvent::DynamicRuntimeChanged(change) => {
                wire::EventPayload::DynamicRuntimeChanged {
                    change: wire_dynamic_runtime_change(change),
                }
            }
            application::OutputEvent::HighlightChanged(change) => {
                wire::EventPayload::HighlightChanged {
                    change: wire::HighlightChange {
                        revision: change.revision,
                        desk_id: change.desk_id,
                        user_id: change.user_id,
                        action: change.action.clone(),
                        source: change.source.clone(),
                        state: super::super::runtime_wire::highlight(change.state.clone()),
                    },
                }
            }
            application::OutputEvent::MediaChanged(change) => wire::EventPayload::MediaChanged {
                change: wire_media_notification(*change),
            },
        },
        application::ApplicationEvent::Show(application::ShowEvent::PatchChanged(change)) => {
            wire::EventPayload::ShowPatchChanged {
                delta: super::super::show_patch_wire::wire_delta(change, Some(sequence)),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::OutputRouteChanged(change)) => {
            wire::EventPayload::OutputRouteChanged {
                change: wire_output_route_change(change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::ObjectsChanged(change)) => {
            wire::EventPayload::ShowObjectsChanged {
                change: wire_show_objects_change(change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::SelectiveImportApplied(
            change,
        )) => wire::EventPayload::SelectiveImportApplied {
            change: Box::new(selective_import::wire_change(change)),
        },
        application::ApplicationEvent::Show(
            application::ShowEvent::VirtualPlaybackExclusionZonesChanged(change),
        ) => wire::EventPayload::VirtualPlaybackExclusionZonesChanged {
            change: light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionZonesChange {
                show_id: change.show_id.0,
                revision: change.revision,
            },
        },
        application::ApplicationEvent::Show(application::ShowEvent::ShowLibraryChanged(change)) => {
            wire::EventPayload::ShowLibraryChanged {
                change: wire_show_library_notification(*change),
            }
        }
        application::ApplicationEvent::Show(application::ShowEvent::FixtureLibraryChanged(
            change,
        )) => wire::EventPayload::FixtureLibraryChanged {
            change: wire_fixture_library_notification(*change),
        },
        application::ApplicationEvent::System(application::SystemEvent::Operator(notification)) => {
            wire::EventPayload::OperatorNotification {
                notification: wire_operator_notification(notification),
            }
        }
    })
}

fn wire_dynamic_runtime_change(
    change: &application::DynamicRuntimeChange,
) -> wire::DynamicRuntimeChange {
    use application::DynamicRuntimeEventKind as App;
    use wire::DynamicRuntimeEventKind as Wire;
    wire::DynamicRuntimeChange {
        kind: match change.kind {
            App::InstanceStarted => Wire::InstanceStarted,
            App::InstancePending => Wire::InstancePending,
            App::InstanceActive => Wire::InstanceActive,
            App::InstanceOff => Wire::InstanceOff,
            App::InstanceRelease => Wire::InstanceRelease,
            App::ControllerUpdated => Wire::ControllerUpdated,
            App::ControllerWinnerChanged => Wire::ControllerWinnerChanged,
            App::Paused => Wire::Paused,
            App::Resumed => Wire::Resumed,
            App::FailedDependency => Wire::FailedDependency,
            App::PreloadCommitted => Wire::PreloadCommitted,
            App::TransitionCompleted => Wire::TransitionCompleted,
        },
        dynamic_id: change.dynamic_id,
        runtime_instance_id: change.runtime_instance_id,
        controller_id: change.controller_id,
        winning_controller_id: change.winning_controller_id,
        occurred_at_millis: change.occurred_at_millis,
        message: change.message.clone(),
    }
}

fn wire_screen_notification(change: application::ScreenNotification) -> wire::ScreenNotification {
    wire::ScreenNotification {
        revision: change.revision,
        kind: match change.kind {
            application::ScreenNotificationKind::Configuration => {
                wire::ScreenNotificationKind::Configuration
            }
            application::ScreenNotificationKind::ScreenPage => {
                wire::ScreenNotificationKind::ScreenPage
            }
            application::ScreenNotificationKind::PlaybackPage => {
                wire::ScreenNotificationKind::PlaybackPage
            }
        },
    }
}

fn wire_show_library_notification(
    change: application::ShowLibraryNotification,
) -> wire::ShowLibraryNotification {
    wire::ShowLibraryNotification {
        revision: change.revision,
        kind: match change.kind {
            application::ShowLibraryNotificationKind::ShowOpened => {
                wire::ShowLibraryNotificationKind::ShowOpened
            }
            application::ShowLibraryNotificationKind::ShowRenamed => {
                wire::ShowLibraryNotificationKind::ShowRenamed
            }
            application::ShowLibraryNotificationKind::ShowRolledBack => {
                wire::ShowLibraryNotificationKind::ShowRolledBack
            }
            application::ShowLibraryNotificationKind::ShowUploaded => {
                wire::ShowLibraryNotificationKind::ShowUploaded
            }
            application::ShowLibraryNotificationKind::ShowDeleted => {
                wire::ShowLibraryNotificationKind::ShowDeleted
            }
        },
    }
}

fn wire_fixture_library_notification(
    change: application::FixtureLibraryNotification,
) -> wire::FixtureLibraryNotification {
    wire::FixtureLibraryNotification {
        revision: change.revision,
        kind: match change.kind {
            application::FixtureLibraryNotificationKind::Library => {
                wire::FixtureLibraryNotificationKind::Library
            }
            application::FixtureLibraryNotificationKind::Profile => {
                wire::FixtureLibraryNotificationKind::Profile
            }
        },
    }
}

fn wire_media_notification(change: application::MediaNotification) -> wire::MediaNotification {
    wire::MediaNotification {
        revision: change.revision,
        kind: match change.kind {
            application::MediaNotificationKind::ThumbnailsRefreshed => {
                wire::MediaNotificationKind::ThumbnailsRefreshed
            }
            application::MediaNotificationKind::PreviewRefreshed => {
                wire::MediaNotificationKind::PreviewRefreshed
            }
            application::MediaNotificationKind::ServerOffline => {
                wire::MediaNotificationKind::ServerOffline
            }
        },
    }
}

fn wire_revision(change: application::NotificationRevision) -> wire::NotificationRevision {
    wire::NotificationRevision {
        revision: change.revision,
    }
}

fn wire_operator_notification(
    notification: &application::OperatorNotification,
) -> wire::OperatorNotification {
    use application::OperatorNotification as App;
    match notification {
        App::DeskAction {
            revision,
            notification,
        } => wire::OperatorNotification::DeskAction {
            revision: *revision,
            notification: wire::DeskActionNotification {
                action: notification.action.clone(),
                control: notification.control.clone(),
                value: notification.value.clone(),
                request_id: notification.request_id.clone(),
                session_id: notification.session_id.clone(),
                desk_id: notification.desk_id.clone(),
                desk_alias: notification.desk_alias.clone(),
            },
        },
        App::FileInput {
            revision,
            notification,
        } => wire::OperatorNotification::FileInput {
            revision: *revision,
            notification: wire::FileInputNotification {
                action: notification.action.clone(),
                instance_id: notification.instance_id.clone(),
                session_id: notification.session_id.clone(),
                source_session_id: notification.source_session_id.clone(),
                desk_id: notification.desk_id.clone(),
                operation: notification.operation.clone(),
                source: notification.source.clone(),
            },
        },
        App::FileOperation {
            revision,
            notification,
        } => wire::OperatorNotification::FileOperation {
            revision: *revision,
            notification: wire::FileOperationNotification {
                operation: notification.operation.clone(),
                items: notification
                    .items
                    .iter()
                    .map(|item| wire::FileOperationItemNotification {
                        source_root_id: item.source_root_id.clone(),
                        source: item.source.clone(),
                        destination_root_id: item.destination_root_id.clone(),
                        destination: item.destination.clone(),
                        status: item.status.clone(),
                        error: item.error.clone(),
                    })
                    .collect(),
            },
        },
        App::GroupConfiguration {
            revision,
            notification,
        } => wire::OperatorNotification::GroupConfiguration {
            revision: *revision,
            notification: wire::GroupConfigurationNotification {
                group_id: notification.group_id.clone(),
                desk_id: notification.desk_id.clone(),
            },
        },
        App::UpdateWorkflow {
            revision,
            notification,
        } => wire::OperatorNotification::UpdateWorkflow {
            revision: *revision,
            notification: wire_update_workflow(notification),
        },
        App::CommandHistoryChanged { revision, desk_id } => {
            wire::OperatorNotification::CommandHistoryChanged {
                revision: *revision,
                desk_id: desk_id.clone(),
            }
        }
    }
}

fn wire_update_workflow(
    notification: &application::UpdateWorkflowNotification,
) -> wire::UpdateWorkflowNotification {
    use application::UpdateWorkflowNotification as App;
    match notification {
        App::Armed { desk_id, armed } => wire::UpdateWorkflowNotification::Armed {
            desk_id: desk_id.clone(),
            armed: *armed,
        },
        App::TargetRequested { desk_id, target } => {
            wire::UpdateWorkflowNotification::TargetRequested {
                desk_id: desk_id.clone(),
                target: wire::UpdateTargetNotification {
                    family: match target.family {
                        application::UpdateTargetFamilyNotification::Cue => {
                            wire::UpdateTargetFamilyNotification::Cue
                        }
                        application::UpdateTargetFamilyNotification::Preset => {
                            wire::UpdateTargetFamilyNotification::Preset
                        }
                        application::UpdateTargetFamilyNotification::Group => {
                            wire::UpdateTargetFamilyNotification::Group
                        }
                    },
                    object_id: target.object_id.clone(),
                    playback_number: target.playback_number,
                    cue_id: target.cue_id.clone(),
                    cue_number: target.cue_number,
                    validate_active_context: target.validate_active_context,
                },
            }
        }
        App::TargetRejected { desk_id, error } => {
            wire::UpdateWorkflowNotification::TargetRejected {
                desk_id: desk_id.clone(),
                error: error.clone(),
            }
        }
        App::TargetsRequested { desk_id } => wire::UpdateWorkflowNotification::TargetsRequested {
            desk_id: desk_id.clone(),
        },
        App::SettingsRequested { desk_id } => wire::UpdateWorkflowNotification::SettingsRequested {
            desk_id: desk_id.clone(),
        },
    }
}

fn wire_show_objects_change(
    change: &application::ActiveShowObjectsChange,
) -> wire::ShowObjectsChange {
    wire::ShowObjectsChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        changes: change.changes.iter().map(wire_show_object_change).collect(),
    }
}

fn wire_show_object_change(change: &application::ActiveShowObjectChange) -> wire::ShowObjectChange {
    macro_rules! variant {
        ($variant:ident) => {
            wire::ShowObjectChange::$variant {
                object_id: change.object_id.clone(),
                object_revision: change.object_revision,
                body: change
                    .body
                    .as_ref()
                    .map(application::ActiveShowObjectBody::encode),
                deleted: change.deleted,
            }
        };
    }
    match change.kind {
        application::ActiveShowObjectKind::CueList => variant!(CueList),
        application::ActiveShowObjectKind::Dynamic => {
            let body = change
                .body
                .as_ref()
                .map(application::ActiveShowObjectBody::encode);
            let validation_error = body
                .as_ref()
                .and_then(super::super::show_objects_v2::dynamic_validation_error);
            wire::ShowObjectChange::Dynamic {
                object_id: change.object_id.clone(),
                object_revision: change.object_revision,
                body,
                validation_error,
                deleted: change.deleted,
            }
        }
        application::ActiveShowObjectKind::Group => variant!(Group),
        application::ActiveShowObjectKind::PatchLayer => variant!(PatchLayer),
        application::ActiveShowObjectKind::Playback => variant!(Playback),
        application::ActiveShowObjectKind::PlaybackPage => variant!(PlaybackPage),
        application::ActiveShowObjectKind::Preset => variant!(Preset),
        application::ActiveShowObjectKind::StageLayout => variant!(StageLayout),
        application::ActiveShowObjectKind::UserLayout => variant!(UserLayout),
    }
}

fn wire_output_route_change(change: &application::OutputRouteChange) -> wire::OutputRouteChange {
    wire::OutputRouteChange {
        show_id: change.show_id.0,
        show_revision: change.show_revision.value(),
        route_id: change.route_id.clone(),
        object_revision: change.object_revision,
        route: change.route.as_ref().map(wire_output_route),
        deleted: change.deleted,
    }
}

fn wire_output_route(route: &light_output::OutputRoute) -> wire::OutputRoute {
    wire::OutputRoute {
        protocol: match route.protocol {
            light_output::Protocol::ArtNet => wire::OutputProtocol::ArtNet,
            light_output::Protocol::Sacn => wire::OutputProtocol::Sacn,
        },
        logical_universe: route.logical_universe,
        destination_universe: route.destination_universe,
        delivery_mode: match route.resolved_delivery_mode() {
            light_output::DeliveryMode::Broadcast => wire::OutputDeliveryMode::Broadcast,
            light_output::DeliveryMode::Multicast => wire::OutputDeliveryMode::Multicast,
            light_output::DeliveryMode::Unicast => wire::OutputDeliveryMode::Unicast,
        },
        destination: route.destination.map(|destination| destination.to_string()),
        enabled: route.enabled,
        minimum_slots: route.minimum_slots,
    }
}

#[cfg(test)]
mod tests;
