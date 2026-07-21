import { type PropsWithChildren, useCallback } from "react";
import { ConfigurationActionsProvider } from "../features/configuration/ConfigurationActionsProvider";
import { ConfigurationStateProvider } from "../features/configuration/ConfigurationState";
import type { StoredStageLayout } from "../features/server/contracts";
import type { useServerState } from "../features/server/useServerState";
import { StageLayoutActionsProvider } from "../features/stageLayout/StageLayoutActionsProvider";
import { StageLayoutStateProvider } from "../features/stageLayout/StageLayoutState";
import type { ConfigurationUpdateResult } from "./client/configuration";

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
	const putStageLayout = useCallback(
		async (
			showId: string,
			layout: StoredStageLayout,
			expectedRevision: number,
		) => {
			await state.client.putObject(
				showId,
				"stage_layout",
				"main",
				layout,
				expectedRevision,
			);
		},
		[state.client],
	);
	const readStageLayout = useCallback(
		async (showId: string) =>
			(
				await state.client.objects<StoredStageLayout>(showId, "stage_layout")
			).find((item) => item.id === "main") ?? null,
		[state.client],
	);
	return (
		<ConfigurationStateProvider store={state.configurationStore}>
			<ConfigurationActionsProvider
				store={state.configurationStore}
				updateConfiguration={state.client.updateConfiguration}
				onApplied={applyConfigurationUpdate}
				onError={state.setError}
			>
				<StageLayoutStateProvider store={state.stageLayoutStore}>
					<StageLayoutActionsProvider
						store={state.stageLayoutStore}
						showId={state.bootstrap?.active_show?.id ?? null}
						putStageLayout={putStageLayout}
						readStageLayout={readStageLayout}
						onApplied={state.setStageLayout}
						onError={state.setError}
					>
						{children}
					</StageLayoutActionsProvider>
				</StageLayoutStateProvider>
			</ConfigurationActionsProvider>
		</ConfigurationStateProvider>
	);
}
