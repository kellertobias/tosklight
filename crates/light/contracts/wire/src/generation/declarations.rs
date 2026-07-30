use ts_rs::{Config, TS};

use crate::v2::command_line::*;
use crate::v2::control_desk_configuration::*;
use crate::v2::cue_deletion::*;
use crate::v2::cue_recording::*;
use crate::v2::cue_transfer::*;
use crate::v2::desk_management::*;
use crate::v2::dynamics::*;
use crate::v2::events::*;
use crate::v2::files::*;
use crate::v2::fixture_library::*;
use crate::v2::group_management::*;
use crate::v2::group_recording::*;
use crate::v2::live_action::*;
use crate::v2::output_control::*;
use crate::v2::output_runtime::*;
use crate::v2::patch::*;
use crate::v2::playback::*;
use crate::v2::playback_topology::*;
use crate::v2::preload_lifecycle::*;
use crate::v2::preload_playback_queue::*;
use crate::v2::preload_values::*;
use crate::v2::preset_recall::*;
use crate::v2::preset_recording::*;
use crate::v2::programmer_lifecycle::*;
use crate::v2::programmer_priority::*;
use crate::v2::programming::*;
use crate::v2::programming_update::*;
use crate::v2::runtime::*;
use crate::v2::schedules::*;
use crate::v2::screen_configuration::*;
use crate::v2::selective_import::*;
use crate::v2::show_library::*;
use crate::v2::show_objects::*;
use crate::v2::speed_group::*;
use crate::v2::stage_layout::*;
use crate::v2::virtual_playback_zones::*;
use crate::v2::visualization::*;

pub(super) fn all(config: &Config) -> Vec<String> {
    let mut declarations = command_line(config);
    declarations.extend(control_desk_configuration(config));
    declarations.extend(desk_management(config));
    declarations.extend(dynamics(config));
    declarations.extend(event_subscription(config));
    declarations.extend(files(config));
    declarations.extend(programming(config));
    declarations.extend(programming_update(config));
    declarations.extend(playback_projection(config));
    declarations.extend(output_runtime_transport(config));
    declarations.extend(output_control(config));
    declarations.extend(speed_group_transport(config));
    declarations.extend(event_payload(config));
    declarations.extend(fixture_library(config));
    declarations.extend(playback_transport(config));
    declarations.extend(playback_topology(config));
    declarations.extend(patch(config));
    declarations.extend(stage_layout(config));
    declarations.extend(runtime(config));
    declarations.extend(schedules(config));
    declarations.extend(screen_configuration(config));
    declarations.extend(virtual_playback_zones(config));
    declarations.extend(visualization(config));
    declarations.extend(selective_import(config));
    declarations.extend(show_library(config));
    declarations.extend(interaction(config));
    declarations.extend(live_actions(config));
    declarations
}

fn schedules(config: &Config) -> Vec<String> {
    vec![
        ScheduleDefinition::decl(config),
        ScheduleTrigger::decl(config),
        ScheduleCalendarRule::decl(config),
        ScheduleWeekday::decl(config),
        ScheduleMonthWeekOrdinal::decl(config),
        ScheduleTarget::decl(config),
        SchedulePlaybackAction::decl(config),
        ScheduleMasterTransition::decl(config),
        ScheduleOccurrenceProjection::decl(config),
        ScheduleOccurrenceStatus::decl(config),
        ScheduleOccurrenceResult::decl(config),
        ScheduleProjection::decl(config),
        ScheduleSnapshot::decl(config),
        SchedulePreviewRequest::decl(config),
        SchedulePreview::decl(config),
        ScheduleCreateDefinition::decl(config),
        ScheduleCreateRequest::decl(config),
        ScheduleUpdateRequest::decl(config),
        SchedulePatch::decl(config),
        ScheduleDuplicateRequest::decl(config),
        ScheduleDeleteRequest::decl(config),
        ScheduleMutationOutcome::decl(config),
        ScheduleRuntimeChange::decl(config),
    ]
}

fn visualization(config: &Config) -> Vec<String> {
    vec![
        VisualizationScope::decl(config),
        VisualizationLane::decl(config),
        VisualizationClientMessage::decl(config),
        VisualizationValue::decl(config),
        VisualizationValueKey::decl(config),
        VisualizationStackEntryType::decl(config),
        VisualizationDynamicStackEntry::decl(config),
        VisualizationLaneSnapshot::decl(config),
        VisualizationLaneDelta::decl(config),
        VisualizationServerMessage::decl(config),
    ]
}

fn dynamics(config: &Config) -> Vec<String> {
    vec![
        DynamicDefinitionProjection::decl(config),
        DynamicTargetBindingProjection::decl(config),
        DynamicLaneProjection::decl(config),
        DynamicLaneModeProjection::decl(config),
        DynamicPhaseSpreadModeProjection::decl(config),
        DynamicKeyframeConfigurationProjection::decl(config),
        DynamicKeyframeProjection::decl(config),
        DynamicMaxMinConfigurationProjection::decl(config),
        DynamicMiddleAmplitudeConfigurationProjection::decl(config),
        DynamicScalarSourceProjection::decl(config),
        DynamicTargetScalarFallbackProjection::decl(config),
        DynamicScalarInterpolationProjection::decl(config),
        DynamicPeriodicFunctionProjection::decl(config),
        DynamicPwmShapeProjection::decl(config),
        DynamicRandomGroupProjection::decl(config),
        DynamicPhaseDistributionProjection::decl(config),
        DynamicPhaseOrderingProjection::decl(config),
        DynamicSpeedProjection::decl(config),
        DynamicSpeedGroupProjection::decl(config),
        DynamicRationalProjection::decl(config),
        DynamicRunModeProjection::decl(config),
        DynamicActivationPolicyProjection::decl(config),
        DynamicActivationBoundaryProjection::decl(config),
        DynamicReferenceProjection::decl(config),
        DynamicValueTimingProjection::decl(config),
        DynamicInstanceOverridesProjection::decl(config),
        DynamicStartActionRequest::decl(config),
        DynamicOffActionRequest::decl(config),
        DynamicControllerValueActionRequest::decl(config),
        DynamicFixAtActionRequest::decl(config),
        DynamicInstanceActionOutcome::decl(config),
        DynamicControllerActionOutcome::decl(config),
        DynamicRuntimeSnapshotProjection::decl(config),
        DynamicDefinitionStatusProjection::decl(config),
        DynamicRuntimeInstanceProjection::decl(config),
        DynamicRuntimeControllerProjection::decl(config),
        DynamicStartLiveActionRequest::decl(config),
        DynamicOffLiveActionRequest::decl(config),
        DynamicControllerLiveActionRequest::decl(config),
        DynamicCreateActionRequest::decl(config),
        DynamicPoolActionRequest::decl(config),
        DynamicDeleteActionRequest::decl(config),
        DynamicUpdateActionRequest::decl(config),
        DynamicUpdateIntent::decl(config),
    ]
}

fn live_actions(config: &Config) -> Vec<String> {
    vec![
        LiveActionMessageType::decl(config),
        PresetRecallLiveActionRequest::decl(config),
        CommandLineReplaceLiveActionRequest::decl(config),
        CommandLineSetLiveActionRequest::decl(config),
        CommandTargetLiveActionRequest::decl(config),
        CommandLineExecuteLiveActionRequest::decl(config),
        CommandTargetHttpActionRequest::decl(config),
        CommandTargetHttpActionOutcome::decl(config),
        ProgrammerUndoHttpActionOutcome::decl(config),
        ProgrammerCaptureModeLiveActionRequest::decl(config),
        ProgrammerCaptureModeHttpActionRequest::decl(config),
        ProgrammerCaptureModeOutcome::decl(config),
        ProgrammingAlignMode::decl(config),
        ProgrammingAlignLiveActionRequest::decl(config),
        ProgrammingAlignHttpActionRequest::decl(config),
        ProgrammingAlignOutcome::decl(config),
        FixtureControlLiveActionRequest::decl(config),
        FixtureControlHttpActionRequest::decl(config),
        FixtureControlKind::decl(config),
        FixtureControlOutcome::decl(config),
        GenerateFixturePresetsRequest::decl(config),
        GeneratedFixturePreset::decl(config),
        GenerateFixturePresetsOutcome::decl(config),
        LiveAction::decl(config),
        LiveActionFrame::decl(config),
    ]
}

fn files(config: &Config) -> Vec<String> {
    vec![
        FileInputAction::decl(config),
        FileInputOrigin::decl(config),
        FileInputClaimRequest::decl(config),
        FileInputReleaseRequest::decl(config),
        NativeNoteUpdateRequest::decl(config),
        TextDocumentUpdateRequest::decl(config),
        FileOperationKind::decl(config),
        FileConflictChoice::decl(config),
        FileOperationRequest::decl(config),
    ]
}

fn desk_management(config: &Config) -> Vec<String> {
    vec![
        ConfigurationUpdateRequest::decl(config),
        ConfigurationPatch::decl(config),
        PoolPresentationConfiguration::decl(config),
        PoolColorPalette::decl(config),
        PresetPoolColorPalette::decl(config),
        PoolColorMode::decl(config),
        PoolItemPresentation::decl(config),
        TimecodeSourceConfiguration::decl(config),
        OscTimecodeConfiguration::decl(config),
        FileManagerRoot::decl(config),
        SpeedGroupSettingsUpdateRequest::decl(config),
        SpeedGroupSource::decl(config),
        SoundToLightConfiguration::decl(config),
        SoundAnalysisMode::decl(config),
        FrequencySelection::decl(config),
        FrequencyPreset::decl(config),
        SpeedGroupLiveActionRequest::decl(config),
        OutputMasterActionRequest::decl(config),
        SpeedGroupLiveAction::decl(config),
        crate::v2::desk_management::SoundObservation::decl(config),
        DeskLockConfigurationUpdateRequest::decl(config),
        DeskUnlockMode::decl(config),
        DeskUnlockRequest::decl(config),
        UserCreateRequest::decl(config),
    ]
}

fn output_control(config: &Config) -> Vec<String> {
    vec![
        DmxOverrideRequest::decl(config),
        HighlightAction::decl(config),
        HighlightActionRequest::decl(config),
        PatchPreviewHighlightRequest::decl(config),
        MediaThumbnailRefreshRequest::decl(config),
        MediaPreviewRefreshRequest::decl(config),
    ]
}

fn fixture_library(config: &Config) -> Vec<String> {
    vec![
        FixtureDefinitionsSnapshot::decl(config),
        FixtureProfilesSnapshot::decl(config),
        FixtureLibraryWarningsSnapshot::decl(config),
        FixtureProfileRevisionsSnapshot::decl(config),
        FixtureLibraryActionRequest::decl(config),
        FixtureLibraryAction::decl(config),
        FixtureLibraryActionOutcome::decl(config),
        FixtureLibraryActionResult::decl(config),
        FixtureLibraryResource::decl(config),
    ]
}

fn control_desk_configuration(config: &Config) -> Vec<String> {
    vec![
        ControlDeskConfigurationActionRequest::decl(config),
        ControlDeskConfigurationAction::decl(config),
        ControlDeskConfigurationPatch::decl(config),
        ControlDeskConfigurationActionOutcome::decl(config),
    ]
}

fn screen_configuration(config: &Config) -> Vec<String> {
    vec![
        FixedScreenFixtureIncludedHeads::decl(config),
        FixedScreenFixtureOrder::decl(config),
        FixedScreenFixtureColumn::decl(config),
        FixedScreenStageRenderQuality::decl(config),
        FixedScreenTextMode::decl(config),
        FixedScreenPane::decl(config),
        ScreenContent::decl(config),
        ScreenPlaybackSurfaceRow::decl(config),
        ScreenPlaybackSurfaceLayout::decl(config),
        ScreenPageMode::decl(config),
        ScreenConfiguration::decl(config),
        ScreenConfigurationSnapshot::decl(config),
        ScreenConfigurationActionRequest::decl(config),
        ScreenConfigurationCreateRequest::decl(config),
        ScreenConfigurationUpdateRequest::decl(config),
        ScreenConfigurationDeleteRequest::decl(config),
        ScreenConfigurationAction::decl(config),
        ScreenConfigurationPatch::decl(config),
        ScreenConfigurationActionOutcome::decl(config),
    ]
}

fn show_library(config: &Config) -> Vec<String> {
    vec![
        ShowLibrarySnapshot::decl(config),
        ShowLibraryEntry::decl(config),
        ShowLibraryRevision::decl(config),
        ShowLibraryActionRequest::decl(config),
        ShowLibraryAction::decl(config),
        ShowOpenTransition::decl(config),
        MvrImportDestination::decl(config),
        MvrImportResolution::decl(config),
        MvrImportResolutionAction::decl(config),
        ShowLibraryActionOutcome::decl(config),
        ShowLibraryActionResult::decl(config),
        MvrApplyOutcome::decl(config),
        MvrImportPreview::decl(config),
        MvrPreviewFixture::decl(config),
        MvrExportPreview::decl(config),
        ShowObjectRecord::decl(config),
        ShowObjectCollectionSnapshot::decl(config),
        ShowObjectExactSnapshot::decl(config),
        OutputRouteActionRequest::decl(config),
        OutputRouteAction::decl(config),
        OutputRoutePatch::decl(config),
        OutputRouteActionOutcome::decl(config),
        UserLayoutActionRequest::decl(config),
        UserLayoutAction::decl(config),
        UserLayoutPatch::decl(config),
        PatchLayerActionRequest::decl(config),
        PatchLayerAction::decl(config),
        PatchLayerInput::decl(config),
        PreloadRecordActionRequest::decl(config),
        PreloadRecordAction::decl(config),
        PreloadPresetMode::decl(config),
        PreloadPresetFamily::decl(config),
        ShowObjectActionOutcome::decl(config),
    ]
}

fn runtime(config: &Config) -> Vec<String> {
    vec![
        RuntimeSessionCreateRequest::decl(config),
        RuntimeDeskUser::decl(config),
        RuntimePlaybackSurfaceRow::decl(config),
        RuntimePlaybackSurfaceLayout::decl(config),
        RuntimeControlDesk::decl(config),
        RuntimeSessionResponse::decl(config),
        RuntimeRevisionCopySource::decl(config),
        RuntimeShowEntry::decl(config),
        RuntimeOutputHealth::decl(config),
        RuntimeClientSummary::decl(config),
        RuntimeAttributeDescriptor::decl(config),
        RuntimeHighlightFixture::decl(config),
        RuntimeHighlightState::decl(config),
        RuntimeBootstrapHighlightState::decl(config),
        RuntimeBootstrapSnapshot::decl(config),
        RuntimeReadinessSnapshot::decl(config),
        RuntimeVisualizationDiagnostics::decl(config),
        RuntimeDiagnosticsSnapshot::decl(config),
        RuntimePerformanceDiagnosticsSnapshot::decl(config),
    ]
}

fn virtual_playback_zones(config: &Config) -> Vec<String> {
    vec![
        VirtualPlaybackExclusionZone::decl(config),
        VirtualPlaybackExclusionSnapshot::decl(config),
        VirtualPlaybackExclusionUpdateRequest::decl(config),
        VirtualPlaybackExclusionUpdateOutcome::decl(config),
        VirtualPlaybackExclusionZonesChange::decl(config),
    ]
}

fn speed_group_transport(config: &Config) -> Vec<String> {
    vec![
        SpeedGroupId::decl(config),
        SpeedGroupProjection::decl(config),
        SpeedGroupAuthorityProjection::decl(config),
        SpeedGroupSnapshot::decl(config),
        SpeedGroupAction::decl(config),
        crate::v2::speed_group::SpeedGroupActionRequest::decl(config),
        SpeedGroupDurability::decl(config),
        SpeedGroupActionState::decl(config),
        SpeedGroupActionOutcome::decl(config),
        SpeedGroupChange::decl(config),
        SpeedGroupErrorKind::decl(config),
        SpeedGroupErrorResponse::decl(config),
    ]
}

fn output_runtime_transport(config: &Config) -> Vec<String> {
    vec![
        OutputRuntimeActionRequest::decl(config),
        OutputRuntimeDurability::decl(config),
        OutputRuntimeActionState::decl(config),
        OutputRuntimeActionOutcome::decl(config),
        OutputRuntimeErrorKind::decl(config),
        OutputRuntimeErrorResponse::decl(config),
    ]
}

fn programming_update(config: &Config) -> Vec<String> {
    vec![
        ProgrammingUpdateCueMode::decl(config),
        ProgrammingUpdateExistingContentMode::decl(config),
        ProgrammingUpdateMode::decl(config),
        ProgrammingUpdateTarget::decl(config),
        ProgrammingUpdateTargetFamily::decl(config),
        ProgrammingUpdateCueIdentity::decl(config),
        ProgrammingUpdateTargetIdentity::decl(config),
        ProgrammingUpdateObjectKind::decl(config),
        ProgrammingUpdateObjectIdentity::decl(config),
        ProgrammingUpdateTargetFilter::decl(config),
        ProgrammingUpdateAddress::decl(config),
        ProgrammingUpdateCueSource::decl(config),
        ProgrammingUpdateIgnoreReason::decl(config),
        ProgrammingUpdateItemOutcome::decl(config),
        ProgrammingUpdatePreviewItem::decl(config),
        ProgrammingUpdatePreview::decl(config),
        ProgrammingUpdatePreviewRequest::decl(config),
        ProgrammingUpdatePreviewResponse::decl(config),
        ProgrammingUpdateTargetsRequest::decl(config),
        ProgrammingUpdateTargetEntry::decl(config),
        ProgrammingUpdateTargetsResponse::decl(config),
        ProgrammingUpdateAction::decl(config),
        ProgrammingUpdateActionRequest::decl(config),
        ProgrammingUpdateProjection::decl(config),
        ProgrammingUpdateSummary::decl(config),
        ProgrammingUpdateActionOutcome::decl(config),
        ProgrammingUpdateErrorKind::decl(config),
        ProgrammingUpdateErrorResponse::decl(config),
        ProgrammingUpdateSettings::decl(config),
        ProgrammingUpdateSettingsProjection::decl(config),
        ProgrammingUpdateSettingsUpdateRequest::decl(config),
        ProgrammingUpdateSettingsUpdateOutcome::decl(config),
    ]
}

fn command_line(config: &Config) -> Vec<String> {
    vec![
        CommandTarget::decl(config),
        CommandKey::decl(config),
        CommandKeyPhase::decl(config),
        CommandAcceptedAction::decl(config),
        CommandChoiceOptionId::decl(config),
        CueTransferOperation::decl(config),
        CueMoveCopyChoiceType::decl(config),
        CommandHttpSource::decl(config),
        CommandChoiceOption::decl(config),
        CueMoveCopyChoice::decl(config),
        DynamicInstanceChoiceType::decl(config),
        DynamicInstanceChoiceOption::decl(config),
        DynamicInstanceChoice::decl(config),
        PendingCommandChoice::decl(config),
        ReplaceCommandLineRequest::decl(config),
        CommandKeyRequest::decl(config),
        ExecuteCommandLineRequest::decl(config),
        CommandLineResponse::decl(config),
        CommandOperationOutcome::decl(config),
        CommandOperationResponse::decl(config),
        CommandErrorResponse::decl(config),
        CommandLineChangedEvent::decl(config),
    ]
}

fn event_subscription(config: &Config) -> Vec<String> {
    vec![
        EventCapability::decl(config),
        EventClass::decl(config),
        EventDeliveryPolicy::decl(config),
        EventActionSource::decl(config),
        EventObject::decl(config),
        EventSubscriptionFilter::decl(config),
        EventRateLimit::decl(config),
        EventSnapshotCursor::decl(config),
        SequenceGap::decl(config),
        EventSource::decl(config),
    ]
}

fn programming(config: &Config) -> Vec<String> {
    vec![
        ProgrammingLifecycleSession::decl(config),
        ProgrammingLifecycleProgrammer::decl(config),
        ProgrammingLifecycleProjection::decl(config),
        ProgrammingLifecycleDelta::decl(config),
        ProgrammingLifecycleChange::decl(config),
        ProgrammingLifecycleSnapshot::decl(config),
        ProgrammerPriorityActionRequest::decl(config),
        ProgrammerPriorityProjection::decl(config),
        ProgrammerPriorityChange::decl(config),
        ProgrammerPrioritySnapshot::decl(config),
        ProgrammerPriorityActionState::decl(config),
        ProgrammerPriorityActionOutcome::decl(config),
        ProgrammerPriorityErrorKind::decl(config),
        ProgrammerPriorityErrorResponse::decl(config),
        ProgrammingColorXyz::decl(config),
        ProgrammingAttributeValue::decl(config),
        ProgrammingFixtureValue::decl(config),
        ProgrammingGroupValue::decl(config),
        ProgrammingDynamicSemanticValue::decl(config),
        ProgrammingDynamicValue::decl(config),
        ProgrammingCaptureModeProjection::decl(config),
        ProgrammingCaptureModeChange::decl(config),
        ProgrammingCaptureModeSnapshot::decl(config),
        ProgrammingValuesProjection::decl(config),
        ProgrammingFixtureValueAddress::decl(config),
        ProgrammingGroupValueAddress::decl(config),
        ProgrammingValuesChange::decl(config),
        ProgrammingValuesSnapshot::decl(config),
        ProgrammingPickerColor::decl(config),
        ProgrammingValueTiming::decl(config),
        ProgrammingValueOperation::decl(config),
        ProgrammingValueMutation::decl(config),
        ProgrammingValuesAction::decl(config),
        ProgrammingValuesActionRequest::decl(config),
        ProgrammingValuesActionState::decl(config),
        ProgrammingValuesActionOutcome::decl(config),
        ProgrammingValuesErrorKind::decl(config),
        ProgrammingValuesErrorResponse::decl(config),
        ProgrammingPreloadColorXyz::decl(config),
        ProgrammingPreloadAttributeValue::decl(config),
        ProgrammingPreloadFixtureValue::decl(config),
        ProgrammingPreloadGroupValue::decl(config),
        ProgrammingPreloadValuesProjection::decl(config),
        ProgrammingPreloadValuesChange::decl(config),
        ProgrammingPreloadValuesSnapshot::decl(config),
        ProgrammingPreloadValueTiming::decl(config),
        ProgrammingPreloadValueMutation::decl(config),
        ProgrammingPreloadValuesAction::decl(config),
        ProgrammingPreloadValuesActionRequest::decl(config),
        ProgrammingPreloadValuesActionState::decl(config),
        ProgrammingPreloadValuesActionOutcome::decl(config),
        ProgrammingPreloadValuesErrorKind::decl(config),
        ProgrammingPreloadValuesErrorResponse::decl(config),
        ProgrammingPreloadPlaybackAction::decl(config),
        ProgrammingPreloadPlaybackSurface::decl(config),
        ProgrammingPreloadPlaybackQueueItem::decl(config),
        ProgrammingPreloadPlaybackQueueProjection::decl(config),
        ProgrammingPreloadPlaybackQueueChange::decl(config),
        ProgrammingPreloadPlaybackQueueSnapshot::decl(config),
        ProgrammingPreloadLifecycleAction::decl(config),
        ProgrammingPreloadLifecycleRequest::decl(config),
        ProgrammingPreloadRuntimeOutcome::decl(config),
        ProgrammingPreloadCommitOutcome::decl(config),
        ProgrammingPreloadLifecycleState::decl(config),
        ProgrammingPreloadLifecycleOutcome::decl(config),
        ProgrammingPreloadLifecycleErrorKind::decl(config),
        ProgrammingPreloadLifecycleErrorResponse::decl(config),
        PresetRecordingFamily::decl(config),
        PresetRecordingAddress::decl(config),
        PresetRecordingMode::decl(config),
        PresetRecordRequest::decl(config),
        RecordedPresetProjection::decl(config),
        PresetRecordOutcome::decl(config),
        PresetRecordErrorKind::decl(config),
        PresetRecordErrorResponse::decl(config),
        PresetRecallRequest::decl(config),
        RecalledPresetProjection::decl(config),
        PresetRecallDisposition::decl(config),
        PresetRecallActionState::decl(config),
        PresetRecallOutcome::decl(config),
        PresetRecallErrorKind::decl(config),
        PresetRecallErrorResponse::decl(config),
        GroupPropertiesUpdate::decl(config),
        GroupSourceExpectation::decl(config),
        GroupManagementOperation::decl(config),
        GroupManagementRequest::decl(config),
        GroupManagementObjectProjection::decl(config),
        GroupManagementOutcome::decl(config),
        GroupManagementErrorKind::decl(config),
        GroupManagementErrorResponse::decl(config),
        GroupRecordOperation::decl(config),
        GroupRecordRequest::decl(config),
        RecordedGroupProjection::decl(config),
        RecordedStoredGroupProjection::decl(config),
        GroupRecordOutcome::decl(config),
        GroupRecordErrorKind::decl(config),
        GroupRecordErrorResponse::decl(config),
        CueRecordTarget::decl(config),
        CueRecordOperation::decl(config),
        CueRecordTiming::decl(config),
        CueRecordCapturePolicy::decl(config),
        CueRecordActivationPolicy::decl(config),
        CueRecordRequest::decl(config),
        CueRecordCapturedSource::decl(config),
        RecordedCueObjectProjection::decl(config),
        CueRecordProjections::decl(config),
        RecordedCueProjection::decl(config),
        CueRecordRuntimeOutcome::decl(config),
        CueRecordOutcome::decl(config),
        CueRecordErrorKind::decl(config),
        CueRecordErrorResponse::decl(config),
        CueDeletionAddress::decl(config),
        CueDeletionAuthority::decl(config),
        CueDeletionRequest::decl(config),
        CueDeletionObjectProjection::decl(config),
        DeletedCueProjection::decl(config),
        CueDeletionOutcome::decl(config),
        CueDeletionErrorKind::decl(config),
        CueDeletionErrorResponse::decl(config),
        CueTransferMode::decl(config),
        CueTransferRequest::decl(config),
        CueTransferObjectProjection::decl(config),
        CueTransferSummary::decl(config),
        CueTransferOutcome::decl(config),
        CueTransferErrorKind::decl(config),
        CueTransferErrorResponse::decl(config),
    ]
}

fn playback_projection(config: &Config) -> Vec<String> {
    vec![
        PlaybackSurface::decl(config),
        PlaybackAddress::decl(config),
        ResolvedPlaybackAddress::decl(config),
        PlaybackAction::decl(config),
        PendingPlaybackAction::decl(config),
        PlaybackOutcome::decl(config),
        PlaybackDurability::decl(config),
        PlaybackRuntimeIdentity::decl(config),
        PlaybackShowScope::decl(config),
        PlaybackCueReference::decl(config),
        ManualXFadeDirection::decl(config),
        SoundLossReason::decl(config),
        SpeedSource::decl(config),
        SoundStatus::decl(config),
        CueListRuntimeProjection::decl(config),
        DynamicPlaybackRuntimeState::decl(config),
        DynamicPlaybackControllerStatus::decl(config),
        DynamicPlaybackSpeedSource::decl(config),
        DynamicPlaybackRuntimeProjection::decl(config),
        SpeedGroupRuntimeProjection::decl(config),
        GrandMasterRuntimeProjection::decl(config),
        PlaybackTargetProjection::decl(config),
        PlaybackRuntimeProjection::decl(config),
        PlaybackDeskProjection::decl(config),
        PlaybackTransitionCause::decl(config),
        PlaybackCueTransition::decl(config),
        PlaybackRuntimeChange::decl(config),
        PlaybackTelemetrySample::decl(config),
        PlaybackTelemetryTick::decl(config),
        DynamicRuntimeEventKind::decl(config),
        DynamicRuntimeChange::decl(config),
    ]
}

fn event_payload(config: &Config) -> Vec<String> {
    vec![
        OutputProtocol::decl(config),
        OutputDeliveryMode::decl(config),
        OutputRoute::decl(config),
        OutputRouteChange::decl(config),
        OutputRuntimeIdentity::decl(config),
        OutputRuntimeScope::decl(config),
        OutputRuntimeProjection::decl(config),
        OutputRuntimeChange::decl(config),
        OutputRuntimeSnapshot::decl(config),
        ShowObjectKind::decl(config),
        ShowObjectChange::decl(config),
        ShowObjectsChange::decl(config),
        SelectiveImportObjectChange::decl(config),
        FixtureProfileIdentity::decl(config),
        ManagedAssetReference::decl(config),
        SelectiveImportChange::decl(config),
        NotificationRevision::decl(config),
        HardwareConnectionNotification::decl(config),
        HighlightChange::decl(config),
        ScreenNotificationKind::decl(config),
        ScreenNotification::decl(config),
        ShowLibraryNotificationKind::decl(config),
        ShowLibraryNotification::decl(config),
        FixtureLibraryNotificationKind::decl(config),
        FixtureLibraryNotification::decl(config),
        MediaNotificationKind::decl(config),
        MediaNotification::decl(config),
        DeskActionNotification::decl(config),
        FileInputNotification::decl(config),
        FileOperationItemNotification::decl(config),
        FileOperationNotification::decl(config),
        GroupConfigurationNotification::decl(config),
        UpdateTargetFamilyNotification::decl(config),
        UpdateTargetNotification::decl(config),
        UpdateWorkflowNotification::decl(config),
        OperatorNotification::decl(config),
        EventPayload::decl(config),
        EventEnvelope::decl(config),
        EventClientMessage::decl(config),
        EventServerMessage::decl(config),
    ]
}

fn playback_transport(config: &Config) -> Vec<String> {
    vec![
        PlaybackOverview::decl(config),
        PlaybackActionRequest::decl(config),
        PlaybackRelatedOutcome::decl(config),
        PlaybackActionOutcome::decl(config),
        PlaybackErrorKind::decl(config),
        PlaybackErrorResponse::decl(config),
        PlaybackRuntimeSnapshotRequest::decl(config),
        PlaybackRuntimeSnapshot::decl(config),
    ]
}

fn playback_topology(config: &Config) -> Vec<String> {
    vec![
        PlaybackTopologyTarget::decl(config),
        PlaybackTopologyButtonAction::decl(config),
        PlaybackTopologyFaderMode::decl(config),
        PlaybackTopologyFlashReleaseMode::decl(config),
        PlaybackTopologyDynamicAssignment::decl(config),
        PlaybackTopologyDynamicTargetScope::decl(config),
        PlaybackTopologyDynamicFaderMode::decl(config),
        PlaybackTopologyDynamicResumePolicy::decl(config),
        PlaybackTopologyPlaybackDefinition::decl(config),
        PlaybackTopologyAction::decl(config),
        PlaybackTopologyActionRequest::decl(config),
        PlaybackTopologyResolution::decl(config),
        PlaybackTopologyObjectProjection::decl(config),
        PlaybackTopologyActionState::decl(config),
        PlaybackTopologyActionOutcome::decl(config),
        PlaybackTopologyErrorKind::decl(config),
        PlaybackTopologyErrorResponse::decl(config),
    ]
}

fn patch(config: &Config) -> Vec<String> {
    vec![
        PatchDirectControlProtocol::decl(config),
        PatchProfilePolicy::decl(config),
        PatchSplitAssignment::decl(config),
        PatchDirectControlEndpoint::decl(config),
        PatchFixtureLocation::decl(config),
        PatchFixtureRotation::decl(config),
        PatchMultiPatchInput::decl(config),
        PatchHighlightOverrideInput::decl(config),
        PatchFixtureInput::decl(config),
        PatchOperatorAddressOverride::decl(config),
        PatchSplitPlacementMode::decl(config),
        PatchSplitPlacementIntent::decl(config),
        PatchPlacementIntent::decl(config),
        PatchFixturesRequest::decl(config),
        PatchFixtureAxis::decl(config),
        PatchFixturePolicyAction::decl(config),
        PatchFixturePolicyActionRequest::decl(config),
        PatchErrorResponse::decl(config),
        PatchLogicalHeadProjection::decl(config),
        PatchMultiPatchProjection::decl(config),
        PatchHighlightOverrideProjection::decl(config),
        PatchFixtureProjection::decl(config),
        PatchModeSplitProjection::decl(config),
        PatchModeProjection::decl(config),
        PatchProfileRevisionProjection::decl(config),
        PatchDelta::decl(config),
        PatchFixturesOutcome::decl(config),
        PatchSnapshot::decl(config),
    ]
}

fn stage_layout(config: &Config) -> Vec<String> {
    vec![
        StagePositionAxis::decl(config),
        StagePosition2d::decl(config),
        StageProjection2d::decl(config),
        StageLayoutAction::decl(config),
        StageLayoutActionRequest::decl(config),
        StageLayoutActionOutcome::decl(config),
        StageLayoutErrorResponse::decl(config),
    ]
}

fn selective_import(config: &Config) -> Vec<String> {
    vec![
        SelectiveImportObjectKey::decl(config),
        SelectiveImportLoadMode::decl(config),
        SelectiveImportConflictResolution::decl(config),
        SelectiveImportConflictChoice::decl(config),
        SelectiveImportProfileKey::decl(config),
        SelectiveImportProfileConflictResolution::decl(config),
        SelectiveImportProfileConflictChoice::decl(config),
        SelectiveImportSelection::decl(config),
        SelectiveImportApplyRequest::decl(config),
        SelectiveImportCatalogSection::decl(config),
        SelectiveImportCatalogObject::decl(config),
        SelectiveImportCatalog::decl(config),
        SelectiveImportObjectAction::decl(config),
        SelectiveImportObjectPreview::decl(config),
        SelectiveImportDependencyDisposition::decl(config),
        SelectiveImportDependency::decl(config),
        SelectiveImportConflict::decl(config),
        SelectiveImportProfileAction::decl(config),
        SelectiveImportProfilePreview::decl(config),
        SelectiveImportManagedAssetAction::decl(config),
        SelectiveImportAssetReference::decl(config),
        SelectiveImportManagedAssetPreview::decl(config),
        SelectiveImportBlocker::decl(config),
        SelectiveImportPreview::decl(config),
        SelectiveImportOutcomeObjectChange::decl(config),
        SelectiveImportProfileChange::decl(config),
        SelectiveImportOutcome::decl(config),
        SelectiveImportErrorResponse::decl(config),
    ]
}

fn interaction(config: &Config) -> Vec<String> {
    vec![
        ProgrammerSelectionRule::decl(config),
        ProgrammerSelectionReference::decl(config),
        ProgrammerSelectionExpression::decl(config),
        ProgrammerSelectionGridAxisOrigin::decl(config),
        ProgrammerSelectionGridMethod::decl(config),
        ProgrammerSelectionGridConfiguration::decl(config),
        ProgrammerRowsFirstTraversal::decl(config),
        ProgrammerColumnsFirstTraversal::decl(config),
        ProgrammerSelectionGridTraversalAxis::decl(config),
        ProgrammerSelectionGrid::decl(config),
        ProgrammerSelectionProjection::decl(config),
        ProgrammingInteractionProjection::decl(config),
        ProgrammingInteractionChange::decl(config),
        ProgrammingInteractionSnapshot::decl(config),
        ProgrammingSelectionGestureSource::decl(config),
        ProgrammingSelectionAction::decl(config),
        ProgrammingSelectionActionRequest::decl(config),
        ProgrammingSelectionAcceptedAction::decl(config),
        ProgrammingSelectionActionOutcome::decl(config),
    ]
}
