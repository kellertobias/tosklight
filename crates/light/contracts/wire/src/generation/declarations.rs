use ts_rs::{Config, TS};

use crate::v2::attribute_configuration::*;
use crate::v2::command_line::*;
use crate::v2::control_desk_configuration::*;
use crate::v2::cue_deletion::*;
use crate::v2::cue_media_previews::*;
use crate::v2::cue_recording::*;
use crate::v2::cue_thumbnails::*;
use crate::v2::cue_transfer::*;
use crate::v2::desk_management::*;
use crate::v2::discovery::*;
use crate::v2::dynamics::*;
use crate::v2::events::*;
use crate::v2::extensions::*;
use crate::v2::files::*;
use crate::v2::fixture_library::*;
use crate::v2::group_management::*;
use crate::v2::group_recording::*;
use crate::v2::internal_audio::*;
use crate::v2::live_action::*;
use crate::v2::macros::*;
use crate::v2::network_endpoints::*;
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
use crate::v2::programming_intent::*;
use crate::v2::programming_update::*;
use crate::v2::psn::*;
use crate::v2::runtime::*;
use crate::v2::schedules::*;
use crate::v2::screen_configuration::*;
use crate::v2::selective_import::*;
use crate::v2::show_library::*;
use crate::v2::show_network::*;
use crate::v2::show_objects::*;
use crate::v2::show_sync::*;
use crate::v2::speed_group::*;
use crate::v2::stage_layout::*;
use crate::v2::timecode::*;
use crate::v2::virtual_playback_zones::*;
use crate::v2::visualization::*;
use crate::v2::visualizer_view::*;

mod programming;

pub(super) use programming::programming_values;
use programming::{interaction, programming, programming_update};

pub(super) fn all(config: &Config) -> Vec<String> {
    let mut declarations = attribute_configuration(config);
    declarations.extend(command_line(config));
    declarations.extend(control_desk_configuration(config));
    declarations.extend(desk_management(config));
    declarations.extend(discovery(config));
    declarations.extend(dynamics(config));
    declarations.extend(event_subscription(config));
    declarations.extend(extensions(config));
    declarations.extend(files(config));
    declarations.extend(programming_values(config));
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
    declarations.extend(psn(config));
    declarations.extend(stage_layout(config));
    declarations.extend(runtime(config));
    declarations.extend(schedules(config));
    declarations.extend(screen_configuration(config));
    declarations.extend(virtual_playback_zones(config));
    declarations.extend(visualization(config));
    declarations.extend(visualizer_view(config));
    declarations.extend(selective_import(config));
    declarations.extend(show_library(config));
    declarations.extend(show_sync(config));
    declarations.extend(interaction(config));
    declarations.extend(internal_audio(config));
    declarations.extend(live_actions(config));
    declarations.extend(macros(config));
    declarations.extend(timecode(config));
    declarations
}

fn internal_audio(config: &Config) -> Vec<String> {
    vec![
        InternalAudioPlayerStatus::decl(config),
        InternalAudioLibraryStatus::decl(config),
        InternalAudioStatus::decl(config),
    ]
}

fn timecode(config: &Config) -> Vec<String> {
    vec![
        TimecodeFrameRate::decl(config),
        TimecodeDefinition::decl(config),
        TimecodeObjectRecord::decl(config),
        TimecodeCollectionSnapshot::decl(config),
        TimecodeAudio::decl(config),
        TimecodeAudioOutputDevices::decl(config),
        TimecodeAudioImportResult::decl(config),
        TimecodeAudioWaveform::decl(config),
        TimecodeMarker::decl(config),
        TimecodeLane::decl(config),
        TimecodeLaneContent::decl(config),
        TimecodeAudioPlayerClip::decl(config),
        TimecodeClipStart::decl(config),
        TimecodeClipEnd::decl(config),
        TimecodeCueStart::decl(config),
        TimecodeCueListClip::decl(config),
        TimecodeSpeedKeyframe::decl(config),
        TimecodeCurve::decl(config),
        TimecodeVolumeKeyframe::decl(config),
        TimecodeObjectActionRequest::decl(config),
        TimecodeObjectAction::decl(config),
        TimecodePatch::decl(config),
        TimecodeTransportAction::decl(config),
        TimecodeTransportActionRequest::decl(config),
        TimecodeTransportState::decl(config),
        TimecodeCueListClipExecutionState::decl(config),
        TimecodeCueListClipExecution::decl(config),
        TimecodeTransportSnapshot::decl(config),
    ]
}

fn macros(config: &Config) -> Vec<String> {
    vec![
        MacroPresentation::decl(config),
        MacroDefinition::decl(config),
        MacroLineStatus::decl(config),
        MacroToken::decl(config),
        MacroTokenKind::decl(config),
        MacroLineDiagnostic::decl(config),
        MacroValidationRequest::decl(config),
        MacroSuggestion::decl(config),
        MacroValidation::decl(config),
        MacroObjectActionRequest::decl(config),
        MacroObjectAction::decl(config),
        MacroPatch::decl(config),
        MacroObjectActionOutcome::decl(config),
        MacroRunActionRequest::decl(config),
        MacroRunLineActionRequest::decl(config),
        MacroLiveAction::decl(config),
        MacroCancelActionRequest::decl(config),
        MacroRunLineUndoOutcome::decl(config),
        MacroTrigger::decl(config),
        MacroExecutionState::decl(config),
        MacroExecutionSnapshot::decl(config),
        MacroRuntimeSnapshot::decl(config),
    ]
}

fn extensions(config: &Config) -> Vec<String> {
    vec![
        ExtensionDiagnostic::decl(config),
        ExtensionPackageSnapshot::decl(config),
        ExtensionInstanceSnapshot::decl(config),
        ExtensionInstanceDiagnosticSnapshot::decl(config),
        ExtensionRuntimeSnapshot::decl(config),
        ExtensionRescanRequest::decl(config),
    ]
}

fn attribute_configuration(config: &Config) -> Vec<String> {
    vec![
        AttributeEncoderGroup::decl(config),
        AttributeValueType::decl(config),
        CustomAttributeLifecycle::decl(config),
        AttributeBounds::decl(config),
        CustomAttributeDescriptor::decl(config),
        AttributePlacement::decl(config),
        AttributeActivationGroup::decl(config),
        ColorProgrammingModel::decl(config),
        AttributeConfiguration::decl(config),
        ColorModelImpactKind::decl(config),
        ColorModelImpactItem::decl(config),
        ColorModelImpact::decl(config),
        ColorResolutionQuality::decl(config),
        ColorIntentEngine::decl(config),
        ColorIntentHeadReport::decl(config),
        ColorIntentReport::decl(config),
        ConfiguredAttributeDescriptor::decl(config),
        AttributeConfigurationSnapshot::decl(config),
        AttributeConfigurationPatch::decl(config),
        AttributeConfigurationUpdateRequest::decl(config),
        AttributeConfigurationUpdateOutcome::decl(config),
    ]
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
        DynamicSpatialProjectionStageProjection::decl(config),
        DynamicSpatialPosition3dProjection::decl(config),
        DynamicSpatialVector3Projection::decl(config),
        DynamicSpatialProjectionProjection::decl(config),
        DynamicSelectionShapeProjection::decl(config),
        DynamicSpatialShapeStageProjection::decl(config),
        DynamicSpatialMappingOverrideProjection::decl(config),
        DynamicSpatialPreviewBaseProjection::decl(config),
        DynamicSpatialPreviewRequest::decl(config),
        DynamicSpatialPreviewResponse::decl(config),
        DynamicTargetBindingProjection::decl(config),
        DynamicLaneProjection::decl(config),
        DynamicLaneBodyProjection::decl(config),
        DynamicLegacyScalarLaneProjection::decl(config),
        DynamicRandomRangeProjection::decl(config),
        DynamicSemanticColorBasisProjection::decl(config),
        DynamicFamilyRepresentationProjection::decl(config),
        DynamicValueAddressProjection::decl(config),
        DynamicValueProjection::decl(config),
        DynamicValueFallbackProjection::decl(config),
        DynamicPresetTemplateProjection::decl(config),
        DynamicPresetGroupTemplateProjection::decl(config),
        DynamicPresetFixtureTemplateProjection::decl(config),
        DynamicValueSourceProjection::decl(config),
        DynamicProgrammingLaneProjection::decl(config),
        DynamicProgrammingLaneConfigurationProjection::decl(config),
        DynamicProgrammingKeyframesProjection::decl(config),
        DynamicProgrammingKeyframeProjection::decl(config),
        DynamicProgrammingMaxMinProjection::decl(config),
        DynamicProgrammingMiddleAmplitudeProjection::decl(config),
        DynamicProgrammingRandomRangeProjection::decl(config),
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
        DynamicSpeedGroupTransportProjection::decl(config),
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
        FixtureFreezeFamily::decl(config),
        FixtureFreezeOperation::decl(config),
        FixtureFreezeLiveActionRequest::decl(config),
        FixtureFreezeActionOutcome::decl(config),
        ProgrammerCaptureModeLiveActionRequest::decl(config),
        ProgrammerCaptureModeHttpActionRequest::decl(config),
        ProgrammerCaptureModeOutcome::decl(config),
        ProgrammingAlignMode::decl(config),
        ProgrammingAlignAction::decl(config),
        ProgrammingAlignLiveActionRequest::decl(config),
        ProgrammingAlignHttpActionRequest::decl(config),
        ProgrammingAlignOutcome::decl(config),
        ProgrammingAlignmentProjection::decl(config),
        ProgrammingAlignmentBinding::decl(config),
        ProgrammingAlignmentLane::decl(config),
        FixtureControlLiveActionRequest::decl(config),
        FixtureControlHttpActionRequest::decl(config),
        FixtureControlTarget::decl(config),
        FixtureControlsLiveActionRequest::decl(config),
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
        HighlightLookConfiguration::decl(config),
        HighlightLookColor::decl(config),
        HighlightLookCompatibility::decl(config),
        PoolPresentationConfiguration::decl(config),
        PoolColorPalette::decl(config),
        PresetPoolColorPalette::decl(config),
        PoolColorMode::decl(config),
        PoolItemPresentation::decl(config),
        TimecodeSourceSelectionConfiguration::decl(config),
        TimecodeFrameRateConfiguration::decl(config),
        ExternalTimecodeLossPolicyConfiguration::decl(config),
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
    ]
}

fn output_control(config: &Config) -> Vec<String> {
    vec![
        OutputFrameIdentity::decl(config),
        OutputTrackingIdentity::decl(config),
        OutputDmxSnapshot::decl(config),
        OutputDmxUniverse::decl(config),
        OutputDmxOverride::decl(config),
        OutputPointPose::decl(config),
        OutputNativeLane::decl(config),
        OutputNativeInstance::decl(config),
        DmxOverrideRequest::decl(config),
        HighlightAction::decl(config),
        HighlightActionRequest::decl(config),
        PatchPreviewHighlightRequest::decl(config),
        MediaThumbnailRefreshRequest::decl(config),
        MediaPreviewRefreshRequest::decl(config),
        MediaLibraryKind::decl(config),
        MediaLibrarySelectionRequest::decl(config),
        MediaLibrarySelectionOutcome::decl(config),
        NativeMediaTextSlot::decl(config),
        NativeMediaEffectParameter::decl(config),
        NativeMediaEffectSlot::decl(config),
        NativeMediaVisualizerChannel::decl(config),
        NativeMediaSnapshot::decl(config),
        NativeMediaTextUpdateRequest::decl(config),
        NativeMediaEffectUpdateRequest::decl(config),
        DiscoveredMediaAddressUpdateRequest::decl(config),
        DiscoveredMediaOutput::decl(config),
        DiscoveredMediaServer::decl(config),
        MediaServerDiscovery::decl(config),
        NetworkEndpointDirection::decl(config),
        NetworkEndpointOrigin::decl(config),
        NetworkEndpointStatus::decl(config),
        NetworkEndpoint::decl(config),
        NetworkEndpointsSnapshot::decl(config),
    ]
}

fn fixture_library(config: &Config) -> Vec<String> {
    vec![
        FixtureGdtfPreviewRequest::decl(config),
        FixtureGdtfDiagnostic::decl(config),
        FixtureGdtfPreview::decl(config),
        FixtureGdtfImportRequest::decl(config),
        FixtureDefinitionsSnapshot::decl(config),
        FixtureProfilesSnapshot::decl(config),
        FixtureLibraryWarningsSnapshot::decl(config),
        FixtureProfileRevisionsSnapshot::decl(config),
        GelCatalogsSnapshot::decl(config),
        GelCatalog::decl(config),
        GelCatalogEntry::decl(config),
        GelCatalogImportTarget::decl(config),
        GelCatalogImportPreviewRequest::decl(config),
        GelCatalogImportConfirmRequest::decl(config),
        GelCatalogImportConfirmOutcome::decl(config),
        GelCatalogImportPreview::decl(config),
        GelCatalogImportAddition::decl(config),
        GelCatalogImportReplacement::decl(config),
        GelCatalogImportConflict::decl(config),
        GelCatalogCsvError::decl(config),
        FixtureLibraryActionRequest::decl(config),
        FixtureLibraryAction::decl(config),
        FixtureAttributeMapping::decl(config),
        FixtureSourceMapping::decl(config),
        FixtureSourceMappingsSnapshot::decl(config),
        FixtureImportRequirement::decl(config),
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
        FixedScreenFixtureCompactMode::decl(config),
        FixedScreenStageRenderQuality::decl(config),
        FixedScreenTextMode::decl(config),
        FixedScreenPane::decl(config),
        FixedScreenSide::decl(config),
        ScreenContent::decl(config),
        ScreenPlaybackSurfaceRow::decl(config),
        ScreenPlaybackSurfaceLayout::decl(config),
        ScreenPageMode::decl(config),
        ScreenConfiguration::decl(config),
        ScreenConfigurationSnapshot::decl(config),
        ProgrammerControlSurfaceConfiguration::decl(config),
        ProgrammerControlSurfacePatch::decl(config),
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
        NetworkShowCatalog::decl(config),
        NetworkShowPeer::decl(config),
        NetworkShow::decl(config),
        crate::v2::show_network::NetworkSaveFolders::decl(config),
        crate::v2::show_network::NetworkSaveRoot::decl(config),
        crate::v2::show_network::NetworkSaveEntry::decl(config),
        crate::v2::show_network::NetworkSaveEntryKind::decl(config),
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
        MvrExportSummary::decl(config),
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
        RuntimeSessionRole::decl(config),
        RuntimePlaybackSurfaceRow::decl(config),
        RuntimePlaybackSurfaceLayout::decl(config),
        RuntimeControlDesk::decl(config),
        RuntimeSessionResponse::decl(config),
        RuntimeRevisionCopySource::decl(config),
        RuntimeShowEntry::decl(config),
        RuntimeOutputHealth::decl(config),
        RuntimeChangeLeadTime::decl(config),
        RuntimeChangeLeadTimeUpdateRequest::decl(config),
        RuntimeChangeLeadTimeUpdateOutcome::decl(config),
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

fn show_sync(config: &Config) -> Vec<String> {
    vec![
        ShowSyncTransactionRequest::decl(config),
        ShowSyncOrigin::decl(config),
        ShowSyncApp::decl(config),
        ShowSyncOperation::decl(config),
        ShowSyncFieldEdit::decl(config),
        ShowSyncStatus::decl(config),
        ShowSyncConflictReason::decl(config),
        ShowSyncConflict::decl(config),
        ShowSyncAppliedObject::decl(config),
        ShowSyncTransactionOutcome::decl(config),
        ShowSyncErrorKind::decl(config),
        ShowSyncErrorResponse::decl(config),
        ShowSyncCommit::decl(config),
        ShowSyncCommittedObject::decl(config),
        ShowSyncMetadataChange::decl(config),
        ShowSyncProfileReference::decl(config),
        ShowSyncGapReason::decl(config),
        ShowSyncGap::decl(config),
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

fn command_line(config: &Config) -> Vec<String> {
    vec![
        CommandTarget::decl(config),
        CommandKey::decl(config),
        CommandKeyPhase::decl(config),
        CommandGestureKind::decl(config),
        CommandGesture::decl(config),
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
        EventTopic::decl(config),
        EventRateLimit::decl(config),
        EventSnapshotCursor::decl(config),
        SequenceGap::decl(config),
        EventSource::decl(config),
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
        DeletedCueHoldProjection::decl(config),
        CueTriggerTimingKind::decl(config),
        CueTriggerTimingProjection::decl(config),
        CueTimingRuntimeProjection::decl(config),
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
        OutputRouteTarget::decl(config),
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
        VisualizerConnectionNotification::decl(config),
        ArchitectSyncNotification::decl(config),
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
        PlaybackConfigurationNotification::decl(config),
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
        PlaybackTopologyFootprint::decl(config),
        PlaybackTopologyFlashReleaseMode::decl(config),
        PlaybackTopologyDynamicAssignment::decl(config),
        PlaybackTopologyDynamicTargetScope::decl(config),
        PlaybackTopologyDynamicFaderMode::decl(config),
        PlaybackTopologyDynamicResumePolicy::decl(config),
        PlaybackTopologyPlaybackDefinition::decl(config),
        PlaybackTopologyGroupMasterAddress::decl(config),
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
        PatchInternalFixtureBindings::decl(config),
        PatchFixtureLocation::decl(config),
        PatchFixtureRotation::decl(config),
        PatchSceneryOptions::decl(config),
        PatchChainTopEnd::decl(config),
        PatchChainBottomEnd::decl(config),
        PatchStairHandrails::decl(config),
        PatchInstalledLightSource::decl(config),
        PatchGelDefinitionSnapshot::decl(config),
        PatchGelAssignment::decl(config),
        PatchInstalledFixtureAppearance::decl(config),
        PatchCalibrationQuality::decl(config),
        PatchPositionCalibration::decl(config),
        PatchAxisOverrides::decl(config),
        PatchAxisCalibration::decl(config),
        PatchPositionCalibrationIdentity::decl(config),
        PatchColorCalibration::decl(config),
        PatchColorPathCalibration::decl(config),
        PatchNativeColorIdentity::decl(config),
        PatchEmitterCalibration::decl(config),
        PatchOpticalProvenance::decl(config),
        PatchColorRecipeMeasurement::decl(config),
        PatchNativeColorValue::decl(config),
        PatchColorXyz::decl(config),
        PatchMultiPatchInput::decl(config),
        PatchHighlightOverrideInput::decl(config),
        PatchFixtureInput::decl(config),
        PatchOperatorAddressOverride::decl(config),
        PatchSplitPlacementMode::decl(config),
        PatchSplitPlacementIntent::decl(config),
        PatchPlacementIntent::decl(config),
        PatchVectorKind::decl(config),
        PatchVectorAxis::decl(config),
        PatchVectorSpreadIntent::decl(config),
        PatchFixturesRequest::decl(config),
        PatchFixtureAxis::decl(config),
        PatchFixturePolicyAction::decl(config),
        PatchFixturePolicyActionRequest::decl(config),
        PatchFixtureUpdateAction::decl(config),
        PatchFixtureUpdateRequest::decl(config),
        PatchErrorResponse::decl(config),
        PatchLogicalHeadProjection::decl(config),
        PatchMultiPatchProjection::decl(config),
        PatchHighlightOverrideProjection::decl(config),
        PatchFixtureFreezeFamily::decl(config),
        PatchFixtureFreezeTargetProjection::decl(config),
        PatchFixtureProjection::decl(config),
        PatchModeSplitProjection::decl(config),
        PatchModeProjection::decl(config),
        PatchProfileRevisionProjection::decl(config),
        PatchDelta::decl(config),
        PatchFixturesOutcome::decl(config),
        PatchSnapshot::decl(config),
    ]
}

fn discovery(config: &Config) -> Vec<String> {
    vec![
        DiscoveredRole::decl(config),
        DiscoveredPeer::decl(config),
        DiscoverySnapshot::decl(config),
    ]
}

fn visualizer_view(config: &Config) -> Vec<String> {
    vec![
        VisualizerViewMode::decl(config),
        VisualizerRenderQuality::decl(config),
        VisualizerCamera::decl(config),
        VisualizerViewProjection::decl(config),
        VisualizerViewSnapshot::decl(config),
        VisualizerViewPatch::decl(config),
        VisualizerViewUpdateRequest::decl(config),
        VisualizerViewUpdateOutcome::decl(config),
    ]
}

fn psn(config: &Config) -> Vec<String> {
    vec![
        PsnCalibrationProjection::decl(config),
        PsnBindingProjection::decl(config),
        PsnZoneProjection::decl(config),
        PsnConfigurationProjection::decl(config),
        PsnHealthProjection::decl(config),
        PsnAcceptedSampleProjection::decl(config),
        PsnIngressDiagnosticsProjection::decl(config),
        PsnSourceProjection::decl(config),
        PsnReceiverDiagnosticsProjection::decl(config),
        PsnTrackerProjection::decl(config),
        PsnPlacementProjection::decl(config),
        PsnStatusProjection::decl(config),
        PsnPointProjection::decl(config),
        PsnMacroProjection::decl(config),
        PsnSnapshot::decl(config),
        PsnUpdateRequest::decl(config),
        PsnUpdateOutcome::decl(config),
        PsnErrorResponse::decl(config),
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
