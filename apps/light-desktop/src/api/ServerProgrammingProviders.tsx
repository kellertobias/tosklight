import { type PropsWithChildren, useCallback } from "react";
import { CueTransferProvider } from "../features/cueTransfer/CueTransferProvider";
import { FrontendWarmupBoundary } from "../features/frontendWarmup/FrontendWarmupBoundary";
import { GroupManagementProvider } from "../features/groupManagement/GroupManagementProvider";
import { OutputRuntimeProvider } from "../features/outputRuntime/OutputRuntimeView";
import { PlaybackRuntimeViewProvider } from "../features/playbackRuntime/PlaybackRuntimeView";
import { PresetRecallProvider } from "../features/presetRecall/PresetRecallProvider";
import { ProgrammerCaptureModeViewProvider } from "../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerLifecycleViewProvider } from "../features/programmerLifecycle/ProgrammerLifecycleView";
import { ProgrammerPreloadLifecycleProvider } from "../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { ProgrammerPreloadPlaybackQueueViewProvider } from "../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import { ProgrammerPreloadValuesViewProvider } from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { ProgrammerPriorityProvider } from "../features/programmerPriority/ProgrammerPriorityView";
import { ProgrammerValuesViewProvider } from "../features/programmerValues/ProgrammerValuesView";
import type { CommandExecutionRequest } from "../features/programmingInteraction/commandExecution";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingUpdateProvider } from "../features/programmingUpdate/ProgrammingUpdateProvider";
import type { ServerCapabilities } from "../features/server/capabilityContracts";
import { useSelectedGroupMembership } from "../features/server/useSelectedGroupMembership";
import type { useServerState } from "../features/server/useServerState";
import { usePortableGroups } from "../features/showObjects/ShowObjectsState";
import { ShowObjectDetailSubscription } from "../features/showObjects/ShowObjectsView";
import { SpeedGroupRuntimeProvider } from "../features/speedGroupRuntime/SpeedGroupRuntimeView";
import type { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";

interface ServerProgrammingProvidersProps {
	state: ReturnType<typeof useServerState>;
	boundaries: ReturnType<typeof useServerFeatureBoundaries>;
	value: ServerCapabilities;
}

function SelectedGroupMembershipSync({
	state,
}: {
	state: ReturnType<typeof useServerState>;
}) {
	const groups = usePortableGroups(state.selectedGroupId !== null);
	useSelectedGroupMembership(
		groups,
		state.selectedGroupId,
		state.setSelectedGroupId,
		state.setSelectedFixtures,
	);
	return null;
}

function PresetRecallBoundary({
	children,
	showId,
	sessionId,
	state,
	boundaries,
}: PropsWithChildren<
	Pick<ServerProgrammingProvidersProps, "state" | "boundaries"> & {
		showId: string | null;
		sessionId: string | null;
	}
>) {
	const loadPreset = useCallback(
		(show: string, objectId: string) =>
			boundaries.loadShowObjectSnapshot(show, "preset", objectId),
		[boundaries.loadShowObjectSnapshot],
	);
	return (
		<PresetRecallProvider
			showId={showId}
			sessionId={sessionId}
			deskId={state.session?.desk.id ?? null}
			authorityKey={boundaries.presetRecallAuthorityKey}
			showStore={state.showObjectsStore}
			transport={boundaries.presetRecallTransport}
			loadPreset={loadPreset}
			onError={boundaries.reportPresetRecallError}
		>
			{children}
		</PresetRecallProvider>
	);
}

function useCommandExecution(value: ServerCapabilities) {
	return useCallback(
		({ command, target, pristine }: CommandExecutionRequest) =>
			value.executeCommandLine(command, { target, pristine }),
		[value.executeCommandLine],
	);
}

export function ServerProgrammingProviders(
	props: PropsWithChildren<ServerProgrammingProvidersProps>,
) {
	const { state, boundaries } = props;
	return (
		<ServerSpeedGroupRuntimeBoundary state={state} boundaries={boundaries}>
			<ServerOutputRuntimeBoundary state={state} boundaries={boundaries}>
				<ProgrammerPriorityProvider
					sessionId={state.session?.session_id ?? null}
					authorityKey={boundaries.programmerPriorityAuthorityKey}
					store={state.programmerPriorityStore}
					transport={boundaries.programmerPriorityTransport}
					onSessionError={boundaries.reportProgrammerPrioritySessionError}
					onMutationError={boundaries.reportProgrammerPriorityMutationError}
				>
					<ServerShowProgrammingProviders {...props} />
				</ProgrammerPriorityProvider>
			</ServerOutputRuntimeBoundary>
		</ServerSpeedGroupRuntimeBoundary>
	);
}

export function ServerSpeedGroupRuntimeBoundary({
	children,
	state,
	boundaries,
}: PropsWithChildren<
	Pick<ServerProgrammingProvidersProps, "state" | "boundaries">
>) {
	return (
		<SpeedGroupRuntimeProvider
			deskId={state.session?.desk.id ?? null}
			authorityKey={boundaries.speedGroupRuntimeAuthorityKey}
			store={state.speedGroupRuntimeStore}
			transport={boundaries.speedGroupRuntimeTransport}
			onSessionError={boundaries.reportSpeedGroupSessionError}
			onMutationError={boundaries.reportSpeedGroupMutationError}
		>
			{children}
		</SpeedGroupRuntimeProvider>
	);
}

export function ServerOutputRuntimeBoundary({
	children,
	state,
	boundaries,
}: PropsWithChildren<
	Pick<ServerProgrammingProvidersProps, "state" | "boundaries">
>) {
	const showId = state.bootstrap?.active_show?.id ?? null;
	const deskId = state.session?.desk.id ?? null;
	return (
		<OutputRuntimeProvider
			showId={showId}
			deskId={deskId}
			authorityKey={boundaries.outputRuntimeAuthorityKey}
			store={state.outputRuntimeStore}
			transport={boundaries.outputRuntimeTransport}
			onSessionError={boundaries.reportOutputRuntimeSessionError}
			onMutationError={boundaries.reportOutputRuntimeMutationError}
		>
			{children}
		</OutputRuntimeProvider>
	);
}

function GroupManagementBoundary({
	children,
	showId,
	state,
	boundaries,
}: PropsWithChildren<
	Pick<ServerProgrammingProvidersProps, "state" | "boundaries"> & {
		showId: string | null;
	}
>) {
	const loadGroup = useCallback(
		(show: string, objectId: string) =>
			boundaries.loadShowObject(show, "group", objectId),
		[boundaries.loadShowObject],
	);
	return (
		<GroupManagementProvider
			showId={showId}
			store={state.showObjectsStore}
			transport={boundaries.groupManagementTransport}
			loadGroup={loadGroup}
			onError={boundaries.reportGroupManagementError}
		>
			{children}
		</GroupManagementProvider>
	);
}

function ProgrammingUpdateBoundary({
	children,
	showId,
	sessionId,
	state,
	boundaries,
}: PropsWithChildren<
	Pick<ServerProgrammingProvidersProps, "state" | "boundaries"> & {
		showId: string | null;
		sessionId: string | null;
	}
>) {
	return (
		<ProgrammingUpdateProvider
			showId={showId}
			deskId={state.session?.desk.id ?? null}
			sessionId={sessionId}
			initialShowRevision={state.bootstrap?.active_show?.revision ?? null}
			authorityKey={boundaries.programmingUpdateAuthorityKey}
			store={state.showObjectsStore}
			transport={boundaries.programmingUpdateTransport}
			loadObject={boundaries.loadShowObject}
		>
			{children}
		</ProgrammingUpdateProvider>
	);
}

function PreloadProgrammingProviders({
	children,
	showId,
	sessionId,
	state,
	boundaries,
	value,
}: PropsWithChildren<
	ServerProgrammingProvidersProps & {
		showId: string | null;
		sessionId: string | null;
	}
>) {
	const executeCommand = useCommandExecution(value);
	const replaceCommandLine = useCallback(
		(deskId: string, text: string, expectedRevision: number) =>
			state.api.programming.replaceProgrammingCommandLine(
				deskId,
				text,
				expectedRevision,
			),
		[state.api],
	);
	const applySelection = useCallback(
		(
			deskId: string,
			request: Parameters<
				typeof state.api.programming.applyProgrammingSelection
			>[1],
		) => state.api.programming.applyProgrammingSelection(deskId, request),
		[state.api],
	);
	return (
		<ProgrammerPreloadValuesViewProvider
			showId={showId}
			sessionId={sessionId}
			authorityKey={boundaries.programmerPreloadValuesAuthorityKey}
			store={state.programmerPreloadValuesStore}
			transport={boundaries.programmerPreloadValuesTransport}
			loadSnapshot={boundaries.loadProgrammerPreloadValuesSnapshot}
			applyAction={boundaries.applyProgrammerPreloadValuesAction}
			onSessionError={boundaries.reportProgrammerPreloadValuesSessionError}
			onMutationError={boundaries.reportProgrammerPreloadValuesMutationError}
		>
			<ProgrammerPreloadPlaybackQueueViewProvider
				showId={showId}
				sessionId={sessionId}
				authorityKey={boundaries.programmerPreloadPlaybackQueueAuthorityKey}
				store={state.programmerPreloadPlaybackQueueStore}
				transport={boundaries.programmerPreloadPlaybackQueueTransport}
				loadSnapshot={boundaries.loadProgrammerPreloadPlaybackQueueSnapshot}
				onSessionError={
					boundaries.reportProgrammerPreloadPlaybackQueueSessionError
				}
			>
				<ProgrammingInteractionViewProvider
					showId={showId}
					deskId={state.session?.desk.id ?? null}
					authorityKey={boundaries.programmingAuthorityKey}
					store={state.programmingInteractionStore}
					transport={boundaries.programmingTransport}
					loadSnapshot={boundaries.loadProgrammingInteractionSnapshot}
					replaceCommandLine={replaceCommandLine}
					executeCommand={executeCommand}
					applySelection={applySelection}
					onSessionError={boundaries.reportProgrammingSessionError}
					onMutationError={boundaries.reportProgrammingMutationError}
				>
					<ProgrammerPreloadLifecycleProvider
						showId={showId}
						sessionId={sessionId}
						deskId={state.session?.desk.id ?? null}
						authorityKey={boundaries.programmerPreloadLifecycleAuthorityKey}
						lifecycleAuthorityKey={boundaries.programmerLifecycleAuthorityKey}
						showStore={state.showObjectsStore}
						store={state.programmerPreloadLifecycleStore}
						transport={boundaries.programmerPreloadLifecycleTransport}
						onError={boundaries.reportProgrammerPreloadLifecycleMutationError}
					>
						{children}
					</ProgrammerPreloadLifecycleProvider>
				</ProgrammingInteractionViewProvider>
			</ProgrammerPreloadPlaybackQueueViewProvider>
		</ProgrammerPreloadValuesViewProvider>
	);
}

function ServerShowProgrammingProviders({
	children,
	state,
	boundaries,
	value,
}: PropsWithChildren<ServerProgrammingProvidersProps>) {
	const showId = state.bootstrap?.active_show?.id ?? null;
	const sessionId = state.session?.session_id ?? null;
	return (
		<GroupManagementBoundary
			showId={showId}
			state={state}
			boundaries={boundaries}
		>
			<ProgrammingUpdateBoundary
				showId={showId}
				sessionId={sessionId}
				state={state}
				boundaries={boundaries}
			>
				<ProgrammerLifecycleViewProvider
					authorityKey={boundaries.programmerLifecycleAuthorityKey}
					store={state.programmerLifecycleStore}
					transport={boundaries.programmerLifecycleTransport}
					loadSnapshot={boundaries.loadProgrammerLifecycleSnapshot}
					onSessionError={boundaries.reportProgrammerLifecycleSessionError}
				>
					<PlaybackRuntimeViewProvider
						showId={showId}
						deskId={state.session?.desk.id ?? null}
						authorityKey={boundaries.playbackAuthorityKey}
						store={state.playbackRuntimeStore}
						transport={boundaries.playbackTransport}
						loadSnapshot={boundaries.loadPlaybackSnapshot}
						applyAction={boundaries.applyPlaybackRuntimeAction}
						applyDeskPage={boundaries.applyPlaybackDeskPage}
						onError={boundaries.reportPlaybackError}
					>
						<ProgrammerCaptureModeViewProvider
							showId={showId}
							sessionId={sessionId}
							authorityKey={boundaries.programmerCaptureModeAuthorityKey}
							store={state.programmerCaptureModeStore}
							transport={boundaries.programmerCaptureModeTransport}
							loadSnapshot={boundaries.loadProgrammerCaptureModeSnapshot}
							onSessionError={
								boundaries.reportProgrammerCaptureModeSessionError
							}
						>
							<CueTransferProvider
								showId={showId}
								deskId={state.session?.desk.id ?? null}
								sessionId={sessionId}
								authorityKey={boundaries.cueTransferAuthorityKey}
								showStore={state.showObjectsStore}
								programmingStore={state.programmingInteractionStore}
								transport={boundaries.cueTransferTransport}
								repair={boundaries.cueTransferConflictRepair}
								onError={boundaries.reportCueTransferError}
							>
								<ProgrammerValuesViewProvider
									showId={showId}
									sessionId={sessionId}
									authorityKey={boundaries.programmerValuesAuthorityKey}
									store={state.programmerValuesStore}
									transport={boundaries.programmerValuesTransport}
									loadSnapshot={boundaries.loadProgrammerValuesSnapshot}
									applyAction={boundaries.applyProgrammerValuesAction}
									onSessionError={boundaries.reportProgrammerValuesSessionError}
									onMutationError={
										boundaries.reportProgrammerValuesMutationError
									}
								>
									<PreloadProgrammingProviders
										showId={showId}
										sessionId={sessionId}
										state={state}
										boundaries={boundaries}
										value={value}
									>
										<PresetRecallBoundary
											showId={showId}
											sessionId={sessionId}
											state={state}
											boundaries={boundaries}
										>
											<FrontendWarmupBoundary showId={showId} state={state} />
											<SelectedGroupMembershipSync state={state} />
											<ShowObjectDetailSubscription
												kind="group"
												objectId={value.selectedGroupId}
											/>
											{children}
										</PresetRecallBoundary>
									</PreloadProgrammingProviders>
								</ProgrammerValuesViewProvider>
							</CueTransferProvider>
						</ProgrammerCaptureModeViewProvider>
					</PlaybackRuntimeViewProvider>
				</ProgrammerLifecycleViewProvider>
			</ProgrammingUpdateBoundary>
		</GroupManagementBoundary>
	);
}
