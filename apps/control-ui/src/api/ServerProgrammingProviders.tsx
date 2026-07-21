import { type PropsWithChildren, useCallback } from "react";
import { CueTransferProvider } from "../features/cueTransfer/CueTransferProvider";
import { PlaybackRuntimeViewProvider } from "../features/playbackRuntime/PlaybackRuntimeView";
import { ProgrammerCaptureModeViewProvider } from "../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerLifecycleViewProvider } from "../features/programmerLifecycle/ProgrammerLifecycleView";
import { ProgrammerPreloadPlaybackQueueViewProvider } from "../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import { ProgrammerPreloadValuesViewProvider } from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { ProgrammerValuesViewProvider } from "../features/programmerValues/ProgrammerValuesView";
import type { CommandExecutionRequest } from "../features/programmingInteraction/commandExecution";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingUpdateProvider } from "../features/programmingUpdate/ProgrammingUpdateProvider";
import type { ServerContextValue } from "../features/server/ServerContextValue";
import { useSelectedGroupMembership } from "../features/server/useSelectedGroupMembership";
import type { useServerState } from "../features/server/useServerState";
import { usePortableGroups } from "../features/showObjects/ShowObjectsState";
import { ShowObjectDetailSubscription } from "../features/showObjects/ShowObjectsView";
import type { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";

interface ServerProgrammingProvidersProps {
	state: ReturnType<typeof useServerState>;
	boundaries: ReturnType<typeof useServerFeatureBoundaries>;
	value: ServerContextValue;
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

export function ServerProgrammingProviders({
	children,
	state,
	boundaries,
	value,
}: PropsWithChildren<ServerProgrammingProvidersProps>) {
	const showId = state.bootstrap?.active_show?.id ?? null;
	const userId = state.session?.user.id ?? null;
	// The command surface owns no transport. Execution is injected here as one
	// narrow feature action over the existing command-execution path.
	const executeCommand = useCallback(
		({ command, target, pristine }: CommandExecutionRequest) =>
			value.executeCommandLine(command, { target, pristine }),
		[value.executeCommandLine],
	);
	return (
		<ProgrammingUpdateProvider
			showId={showId}
			deskId={state.session?.desk.id ?? null}
			userId={userId}
			initialShowRevision={state.bootstrap?.active_show?.revision ?? null}
			authorityKey={boundaries.programmingUpdateAuthorityKey}
			store={state.showObjectsStore}
			transport={boundaries.programmingUpdateTransport}
			loadObject={boundaries.loadShowObject}
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
						userId={userId}
						authorityKey={boundaries.programmerCaptureModeAuthorityKey}
						store={state.programmerCaptureModeStore}
						transport={boundaries.programmerCaptureModeTransport}
						loadSnapshot={boundaries.loadProgrammerCaptureModeSnapshot}
						onSessionError={boundaries.reportProgrammerCaptureModeSessionError}
					>
						<CueTransferProvider
							showId={showId}
							deskId={state.session?.desk.id ?? null}
							userId={userId}
							authorityKey={boundaries.cueTransferAuthorityKey}
							showStore={state.showObjectsStore}
							programmingStore={state.programmingInteractionStore}
							transport={boundaries.cueTransferTransport}
							repair={boundaries.cueTransferConflictRepair}
							onError={boundaries.reportCueTransferError}
						>
							<ProgrammerValuesViewProvider
								showId={showId}
								userId={userId}
								authorityKey={boundaries.programmerValuesAuthorityKey}
								store={state.programmerValuesStore}
								transport={boundaries.programmerValuesTransport}
								loadSnapshot={boundaries.loadProgrammerValuesSnapshot}
								applyAction={boundaries.applyProgrammerValuesAction}
								onSessionError={boundaries.reportProgrammerValuesSessionError}
								onMutationError={boundaries.reportProgrammerValuesMutationError}
							>
								<ProgrammerPreloadValuesViewProvider
									showId={showId}
									userId={userId}
									authorityKey={boundaries.programmerPreloadValuesAuthorityKey}
									store={state.programmerPreloadValuesStore}
									transport={boundaries.programmerPreloadValuesTransport}
									loadSnapshot={boundaries.loadProgrammerPreloadValuesSnapshot}
									applyAction={boundaries.applyProgrammerPreloadValuesAction}
									onSessionError={
										boundaries.reportProgrammerPreloadValuesSessionError
									}
									onMutationError={
										boundaries.reportProgrammerPreloadValuesMutationError
									}
								>
									<ProgrammerPreloadPlaybackQueueViewProvider
										showId={showId}
										userId={userId}
										authorityKey={
											boundaries.programmerPreloadPlaybackQueueAuthorityKey
										}
										store={state.programmerPreloadPlaybackQueueStore}
										transport={
											boundaries.programmerPreloadPlaybackQueueTransport
										}
										loadSnapshot={
											boundaries.loadProgrammerPreloadPlaybackQueueSnapshot
										}
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
											loadSnapshot={
												boundaries.loadProgrammingInteractionSnapshot
											}
											replaceCommandLine={
												state.client.replaceProgrammingCommandLine
											}
											executeCommand={executeCommand}
											applySelection={state.client.applyProgrammingSelection}
											onSessionError={boundaries.reportProgrammingSessionError}
											onMutationError={
												boundaries.reportProgrammingMutationError
											}
										>
											<SelectedGroupMembershipSync state={state} />
											<ShowObjectDetailSubscription
												kind="group"
												objectId={value.selectedGroupId}
											/>
											{children}
										</ProgrammingInteractionViewProvider>
									</ProgrammerPreloadPlaybackQueueViewProvider>
								</ProgrammerPreloadValuesViewProvider>
							</ProgrammerValuesViewProvider>
						</CueTransferProvider>
					</ProgrammerCaptureModeViewProvider>
				</PlaybackRuntimeViewProvider>
			</ProgrammerLifecycleViewProvider>
		</ProgrammingUpdateProvider>
	);
}
