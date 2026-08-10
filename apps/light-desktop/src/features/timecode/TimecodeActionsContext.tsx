import { createContext, type PropsWithChildren, useContext } from "react";
import type { TimecodesApiClient } from "../../api/client/timecodes";
import type { TimecodeTransportSnapshot } from "../../api/types/timecode";

export interface TimecodeActions {
	api: TimecodesApiClient;
	events?: {
		onRuntimeChanged(
			listener: (snapshot: TimecodeTransportSnapshot) => void,
		): () => unknown;
	};
}

const TimecodeActionsContext = createContext<TimecodeActions | null>(null);

export function TimecodeActionsProvider({
	children,
	api,
	events,
}: PropsWithChildren<TimecodeActions>) {
	return (
		<TimecodeActionsContext.Provider value={{ api, events }}>
			{children}
		</TimecodeActionsContext.Provider>
	);
}

export function useTimecodeActions(): TimecodeActions | null {
	return useContext(TimecodeActionsContext);
}
