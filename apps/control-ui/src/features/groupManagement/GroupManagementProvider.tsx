import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import type { ShowObject } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	GroupManagementActions,
	GroupManagementTransport,
} from "./contracts";
import { GroupManagementWriter } from "./writer";

interface GroupManagementProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	transport: GroupManagementTransport | null;
	loadGroup(
		showId: string,
		objectId: string,
	): Promise<ShowObject<"group"> | null>;
	onError?: (error: Error | null) => void;
}

const GroupManagementContext = createContext<GroupManagementActions | null>(
	null,
);

/**
 * Mounting this action boundary performs no reads, snapshots, or subscriptions. It stays dormant
 * until an owner invokes an action, and a Show or session replacement builds a new writer so late
 * outcomes from the previous one can no longer settle.
 */
export function GroupManagementProvider({
	children,
	showId,
	store,
	transport,
	loadGroup,
	onError,
}: PropsWithChildren<GroupManagementProviderProps>) {
	const writer = useMemo(
		() =>
			showId && transport
				? new GroupManagementWriter({
						showId,
						store,
						transport,
						loadGroup,
						onError,
					})
				: null,
		[loadGroup, onError, showId, store, transport],
	);
	const actions = useMemo<GroupManagementActions>(
		() =>
			writer ?? {
				manage: async () => {
					onError?.(new Error("Group management is unavailable"));
					return null;
				},
			},
		[onError, writer],
	);
	useEffect(() => () => writer?.stop(), [writer]);
	return (
		<GroupManagementContext.Provider value={actions}>
			{children}
		</GroupManagementContext.Provider>
	);
}

export function useGroupManagement() {
	return useContext(GroupManagementContext);
}
