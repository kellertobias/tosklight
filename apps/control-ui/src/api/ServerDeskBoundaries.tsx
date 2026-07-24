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
	return (
		<DeskSnapshotStateProvider store={state.deskSnapshotStore}>
		<CommandHistoryStateProvider store={state.commandHistoryStore}>
		<ShellStatusStateProvider store={state.shellStatusStore}>
		<DeskLockStateProvider store={state.deskLockStore}>
		<DeskLockActionsProvider
			store={state.deskLockStore}
			configure={state.client.configureDeskLock}
			lock={state.client.lockDesk}
			unlock={state.client.unlockDesk}
			onError={state.setError}
		>
		<ConfigurationStateProvider store={state.configurationStore}>
			<ConfigurationActionsProvider
				store={state.configurationStore}
				updateConfiguration={state.client.updateConfiguration}
				onApplied={applyConfigurationUpdate}
				onError={state.setError}
			>
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
