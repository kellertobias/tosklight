use ts_rs::{Config, TS};

use crate::v2::command_line::*;
use crate::v2::events::*;
use crate::v2::patch::*;
use crate::v2::playback::*;
use crate::v2::preload_playback_queue::*;
use crate::v2::preload_values::*;
use crate::v2::preset_recording::*;
use crate::v2::programmer_lifecycle::*;
use crate::v2::programming::*;
use crate::v2::selective_import::*;

pub(super) fn all(config: &Config) -> Vec<String> {
    let mut declarations = command_line(config);
    declarations.extend(event_subscription(config));
    declarations.extend(programming(config));
    declarations.extend(playback_projection(config));
    declarations.extend(event_payload(config));
    declarations.extend(playback_transport(config));
    declarations.extend(patch(config));
    declarations.extend(selective_import(config));
    declarations.extend(interaction(config));
    declarations
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
        PresetRecordingFamily::decl(config),
        PresetRecordingAddress::decl(config),
        PresetRecordingMode::decl(config),
        PresetRecordRequest::decl(config),
        RecordedPresetProjection::decl(config),
        PresetRecordOutcome::decl(config),
        PresetRecordErrorKind::decl(config),
        PresetRecordErrorResponse::decl(config),
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
        PlaybackActionOutcome::decl(config),
        PlaybackErrorKind::decl(config),
        PlaybackErrorResponse::decl(config),
        PlaybackRuntimeSnapshotRequest::decl(config),
        PlaybackRuntimeSnapshot::decl(config),
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
