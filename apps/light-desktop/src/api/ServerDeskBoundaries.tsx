import { type PropsWithChildren, useCallback } from "react";
import { CommandHistoryStateProvider } from "../features/commandHistory/CommandHistoryState";
import { CueThumbnailActionsProvider } from "../features/cueThumbnails/CueThumbnailActions";
import { ConfigurationActionsProvider } from "../features/configuration/ConfigurationActionsProvider";
import { ConfigurationStateProvider } from "../features/configuration/ConfigurationState";
import { DeskLockActionsProvider } from "../features/deskLock/DeskLockActionsProvider";
import { DeskLockStateProvider } from "../features/deskLock/DeskLockState";
import { DeskSnapshotStateProvider } from "../features/deskSnapshot/DeskSnapshotState";
import { PoolPresentationLegacyMigration } from "../features/poolPresentation/PoolPresentationLegacyMigration";
import type { useServerState } from "../features/server/useServerState";
import type { SessionRole } from "../features/session/ownership";
import { ShellStatusStateProvider } from "../features/shellStatus/ShellStatusState";
import { StageLayoutActionsProvider } from "../features/stageLayout/StageLayoutActions";
import { StageLayoutStateProvider } from "../features/stageLayout/StageLayoutState";
import type { ConfigurationUpdateResult } from "./client/deskManagement";

type ServerState = ReturnType<typeof useServerState>;

/**
 * Desk-installation capability boundaries: authoritative configuration and stage layout, each with
 * its own scoped store and action provider so readers stay off the broad server-context path.
 */
export function ServerDeskBoundaries({
	children,
	state,
	sessionRole,
}: PropsWithChildren<{ state: ServerState; sessionRole: SessionRole }>) {
	const applyConfigurationUpdate = useCallback(
		(result: ConfigurationUpdateResult) => {
			state.setConfiguration(result.configuration);
			state.setMatter(result.matter);
		},
		[state.setConfiguration, state.setMatter],
	);
	const configureDeskLock = useCallback(
		(...args: Parameters<typeof state.api.desk.configureDeskLock>) =>
			state.api.desk.configureDeskLock(...args),
		[state.api],
	);
	const lockDesk = useCallback(
		(...args: Parameters<typeof state.api.desk.lockDesk>) =>
			state.api.desk.lockDesk(...args),
		[state.api],
	);
	const unlockDesk = useCallback(
		(...args: Parameters<typeof state.api.desk.unlockDesk>) =>
			state.api.desk.unlockDesk(...args),
		[state.api],
	);
	const updateConfiguration = useCallback(
		(...args: Parameters<typeof state.api.desk.updateConfiguration>) =>
			state.api.desk.updateConfiguration(...args),
		[state.api],
	);
	const updatePoolPresentation = useCallback(
		(...args: Parameters<typeof state.api.desk.updatePoolPresentation>) =>
			state.api.desk.updatePoolPresentation(...args),
		[state.api],
	);
	return (
		<DeskSnapshotStateProvider store={state.deskSnapshotStore}>
			<CommandHistoryStateProvider store={state.commandHistoryStore}>
				<ShellStatusStateProvider store={state.shellStatusStore}>
					<DeskLockStateProvider store={state.deskLockStore}>
						<DeskLockActionsProvider
							store={state.deskLockStore}
							configure={configureDeskLock}
							lock={lockDesk}
							unlock={unlockDesk}
							onError={state.setError}
						>
							<ConfigurationStateProvider store={state.configurationStore}>
								<ConfigurationActionsProvider
									store={state.configurationStore}
									updateConfiguration={updateConfiguration}
									updatePoolPresentation={updatePoolPresentation}
									onApplied={applyConfigurationUpdate}
									onError={state.setError}
								>
									<PoolPresentationLegacyMigration />
									<StageLayoutStateProvider store={state.stageLayoutStore}>
										<StageLayoutActionsProvider
											client={state.api.stageLayout}
											showId={state.bootstrap?.active_show?.id ?? null}
											canWrite={
												sessionRole === "primary" &&
												state.status === "connected"
											}
										>
											<CueThumbnailActionsProvider
												client={state.api.cueThumbnails}
												showId={state.bootstrap?.active_show?.id ?? null}
												canWrite={
													sessionRole === "primary" &&
													state.status === "connected"
												}
											>
												{children}
											</CueThumbnailActionsProvider>
										</StageLayoutActionsProvider>
									</StageLayoutStateProvider>
								</ConfigurationActionsProvider>
							</ConfigurationStateProvider>
						</DeskLockActionsProvider>
					</DeskLockStateProvider>
				</ShellStatusStateProvider>
			</CommandHistoryStateProvider>
		</DeskSnapshotStateProvider>
	);
}
