import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
} from "react";
import type {
	AttributeConfigurationApiClient,
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
	ColorIntentReport,
	ColorModelImpact,
	ColorProgrammingModel,
} from "../../api/client/attributeConfiguration";

export interface AttributeConfigurationActions {
	canWrite: boolean;
	load(): Promise<AttributeConfigurationSnapshot>;
	update(
		snapshot: AttributeConfigurationSnapshot,
		patch: AttributeConfigurationPatch,
		options?: { acknowledgeColorModelImpact?: boolean },
	): Promise<AttributeConfigurationSnapshot>;
	colorModelImpact(model: ColorProgrammingModel): Promise<ColorModelImpact>;
	colorIntentReport(fixtureIds: readonly string[]): Promise<ColorIntentReport>;
}

const AttributeConfigurationActionsContext =
	createContext<AttributeConfigurationActions | null>(null);

export function AttributeConfigurationActionsProvider({
	children,
	client,
	showId,
	canWrite,
	onApplied,
}: PropsWithChildren<{
	client: AttributeConfigurationApiClient;
	showId: string | null;
	canWrite: boolean;
	onApplied(): Promise<void>;
}>) {
	const load = useCallback(() => {
		if (!showId)
			return Promise.reject(
				new Error("Attribute configuration requires an active show."),
			);
		return client.snapshot(showId);
	}, [client, showId]);
	const update = useCallback(
		async (
			snapshot: AttributeConfigurationSnapshot,
			patch: AttributeConfigurationPatch,
			options?: { acknowledgeColorModelImpact?: boolean },
		) => {
			if (!canWrite || !showId)
				throw new Error(
					"The primary desk is not ready to edit show attributes.",
				);
			const outcome = await client.update(showId, snapshot, patch, options);
			await onApplied();
			return outcome.snapshot;
		},
		[canWrite, client, onApplied, showId],
	);
	const colorModelImpact = useCallback(
		(model: ColorProgrammingModel) => {
			if (!showId)
				return Promise.reject(
					new Error("The colour model requires an active show."),
				);
			return client.colorModelImpact(showId, model);
		},
		[client, showId],
	);
	const colorIntentReport = useCallback(
		(fixtureIds: readonly string[]) => {
			if (!showId)
				return Promise.reject(
					new Error("The colour report requires an active show."),
				);
			return client.colorIntentReport(showId, fixtureIds);
		},
		[client, showId],
	);
	const actions = useMemo(
		() => ({
			canWrite: canWrite && Boolean(showId),
			load,
			update,
			colorModelImpact,
			colorIntentReport,
		}),
		[canWrite, colorIntentReport, colorModelImpact, load, showId, update],
	);
	return (
		<AttributeConfigurationActionsContext.Provider value={actions}>
			{children}
		</AttributeConfigurationActionsContext.Provider>
	);
}

/** Supplies stubbed actions to a component under test. */
export const AttributeConfigurationActionsContextForTest =
	AttributeConfigurationActionsContext.Provider;

export function useAttributeConfigurationActions() {
	return useContext(AttributeConfigurationActionsContext);
}
