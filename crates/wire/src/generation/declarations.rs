use ts_rs::{Config, TS};

use crate::v2::command_line::*;
use crate::v2::cue_recording::*;
use crate::v2::cue_transfer::*;
use crate::v2::events::*;
use crate::v2::group_recording::*;
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
use crate::v2::selective_import::*;

pub(super) fn all(config: &Config) -> Vec<String> {
    let mut declarations = command_line(config);
    declarations.extend(event_subscription(config));
    declarations.extend(programming(config));
    declarations.extend(programming_update(config));
    declarations.extend(playback_projection(config));
    declarations.extend(output_runtime_transport(config));
    declarations.extend(event_payload(config));
    declarations.extend(playback_transport(config));
    declarations.extend(playback_topology(config));
    declarations.extend(patch(config));
    declarations.extend(selective_import(config));
    declarations.extend(interaction(config));
    declarations
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
        ProgrammingCaptureModeProjection::decl(config),
        ProgrammingCaptureModeChange::decl(config),
        ProgrammingCaptureModeSnapshot::decl(config),
        ProgrammingValuesProjection::decl(config),
        ProgrammingValuesChange::decl(config),
        ProgrammingValuesSnapshot::decl(config),
        ProgrammingValueTiming::decl(config),
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
        PresetRecallActionState::decl(config),
        PresetRecallOutcome::decl(config),
        PresetRecallErrorKind::decl(config),
        PresetRecallErrorResponse::decl(config),
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
        SpeedGroupRuntimeProjection::decl(config),
        GrandMasterRuntimeProjection::decl(config),
        PlaybackTargetProjection::decl(config),
        PlaybackRuntimeProjection::decl(config),
        PlaybackDeskProjection::decl(config),
        PlaybackTransitionCause::decl(config),
        PlaybackCueTransition::decl(config),
        PlaybackRuntimeChange::decl(config),
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
        EventPayload::decl(config),
        EventEnvelope::decl(config),
        EventClientMessage::decl(config),
        EventServerMessage::decl(config),
    ]
}

fn playback_transport(config: &Config) -> Vec<String> {
    vec![
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
        PatchFixturesRequest::decl(config),
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

fn selective_import(config: &Config) -> Vec<String> {
    vec![
        SelectiveImportObjectKey::decl(config),
        SelectiveImportConflictResolution::decl(config),
        SelectiveImportConflictChoice::decl(config),
        SelectiveImportProfileKey::decl(config),
        SelectiveImportProfileConflictResolution::decl(config),
        SelectiveImportProfileConflictChoice::decl(config),
        SelectiveImportSelection::decl(config),
        SelectiveImportApplyRequest::decl(config),
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
