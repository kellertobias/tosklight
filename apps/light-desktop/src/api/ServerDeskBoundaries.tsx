import { type PropsWithChildren, useCallback } from "react";
import { ConfigurationActionsProvider } from "../features/configuration/ConfigurationActionsProvider";
import { ConfigurationStateProvider } from "../features/configuration/ConfigurationState";
import { DeskLockActionsProvider } from "../features/deskLock/DeskLockActionsProvider";
import { DeskLockStateProvider } from "../features/deskLock/DeskLockState";
import { CommandHistoryStateProvider } from "../features/commandHistory/CommandHistoryState";
import { DeskSnapshotStateProvider } from "../features/deskSnapshot/DeskSnapshotState";
import { ShellStatusStateProvider } from "../features/shellStatus/ShellStatusState";
import type { useServerState } from "../features/server/useServerState";
import { StageLayoutStateProvider } from "../features/stageLayout/StageLayoutState";
import type { ConfigurationUpdateResult } from "./client/deskManagement";
import { PoolPresentationLegacyMigration } from "../features/poolPresentation/PoolPresentationLegacyMigration";

type ServerState = ReturnType<typeof useServerState>;

/**
 * Desk-installation capability boundaries: authoritative configuration and stage layout, each with
 * its own scoped store and action provider so readers stay off the broad server-context path.
 */
export function ServerDeskBoundaries({
	children,
	state,
}: PropsWithChildren<{ state: ServerState }>) {
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
					{children}
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
