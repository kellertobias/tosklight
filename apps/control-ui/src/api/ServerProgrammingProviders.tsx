import type { PropsWithChildren } from "react";
import { PlaybackRuntimeViewProvider } from "../features/playbackRuntime/PlaybackRuntimeView";
import { ProgrammerCaptureModeViewProvider } from "../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerLifecycleViewProvider } from "../features/programmerLifecycle/ProgrammerLifecycleView";
import { ProgrammerPreloadValuesViewProvider } from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { ProgrammerValuesViewProvider } from "../features/programmerValues/ProgrammerValuesView";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import type { ServerContextValue } from "../features/server/ServerContextValue";
import { useSelectedGroupMembership } from "../features/server/useSelectedGroupMembership";
import type { useServerState } from "../features/server/useServerState";
import { useGroups } from "../features/server/useShowObjectsState";
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
	const groups = useGroups(state.playbacks);
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
	return (
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
				store={state.playbackRuntimeStore}
				transport={boundaries.playbackTransport}
				loadSnapshot={boundaries.loadPlaybackSnapshot}
				initialDesk={
					state.playbacks
						? {
								activePage: state.playbacks.active_page,
								selectedPlayback: state.playbacks.selected_playback ?? null,
							}
						: null
				}
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
							<ProgrammingInteractionViewProvider
								showId={showId}
								deskId={state.session?.desk.id ?? null}
								store={state.programmingInteractionStore}
								transport={boundaries.programmingTransport}
								loadSnapshot={boundaries.loadProgrammingInteractionSnapshot}
								replaceCommandLine={state.client.replaceProgrammingCommandLine}
								applySelection={state.client.applyProgrammingSelection}
								onSessionError={boundaries.reportProgrammingSessionError}
								onMutationError={boundaries.reportProgrammingMutationError}
							>
								<SelectedGroupMembershipSync state={state} />
								<ShowObjectDetailSubscription
									kind="group"
									objectId={value.selectedGroupId}
								/>
								{children}
							</ProgrammingInteractionViewProvider>
						</ProgrammerPreloadValuesViewProvider>
					</ProgrammerValuesViewProvider>
				</ProgrammerCaptureModeViewProvider>
			</PlaybackRuntimeViewProvider>
		</ProgrammerLifecycleViewProvider>
	);
}
