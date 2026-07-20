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
	GroupRecordingActions,
	GroupRecordingTransport,
} from "./contracts";
import { GroupRecordingWriter } from "./writer";

interface GroupRecordingProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	transport: GroupRecordingTransport | null;
	loadGroup(
		showId: string,
		objectId: string,
	): Promise<ShowObject<"group"> | null>;
	onError?: (error: Error | null) => void;
}

const GroupRecordingContext = createContext<GroupRecordingActions | null>(null);

/** Mounting this action boundary performs no reads or subscriptions. */
export function GroupRecordingProvider({
	children,
	showId,
	store,
	transport,
	loadGroup,
	onError,
}: PropsWithChildren<GroupRecordingProviderProps>) {
	const writer = useMemo(
		() =>
			showId && transport
				? new GroupRecordingWriter({
						showId,
						store,
						transport,
						loadGroup,
						onError,
					})
				: null,
		[loadGroup, onError, showId, store, transport],
	);
	const actions = useMemo<GroupRecordingActions>(
		() =>
			writer ?? {
				record: async () => {
					onError?.(new Error("Group recording is unavailable"));
					return null;
				},
			},
		[onError, writer],
	);
	useEffect(() => () => writer?.stop(), [writer]);
	return (
		<GroupRecordingContext.Provider value={actions}>
			{children}
		</GroupRecordingContext.Provider>
	);
}

export function useGroupRecording() {
	return useContext(GroupRecordingContext);
}
