import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
} from "react";
import type { AttributeConfigurationApiClient } from "../../api/client/attributeConfiguration";
import type {
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
} from "../../api/generated/light-wire";

export interface AttributeConfigurationActions {
	canWrite: boolean;
	load(): Promise<AttributeConfigurationSnapshot>;
	update(
		snapshot: AttributeConfigurationSnapshot,
		patch: AttributeConfigurationPatch,
	): Promise<AttributeConfigurationSnapshot>;
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
		) => {
			if (!canWrite || !showId)
				throw new Error(
					"The primary desk is not ready to edit show attributes.",
				);
			const outcome = await client.update(showId, snapshot, patch);
			await onApplied();
			return outcome.snapshot;
		},
		[canWrite, client, onApplied, showId],
	);
	const actions = useMemo(
		() => ({
			canWrite: canWrite && Boolean(showId),
			load,
			update,
		}),
		[canWrite, load, showId, update],
	);
	return (
		<AttributeConfigurationActionsContext.Provider value={actions}>
			{children}
		</AttributeConfigurationActionsContext.Provider>
	);
}

export function useAttributeConfigurationActions() {
	return useContext(AttributeConfigurationActionsContext);
}
