import { createContext, type PropsWithChildren, useContext } from "react";
import type { TimecodesApiClient } from "../../api/client/timecodes";
import type { EventPayload } from "../../api/generated/light-wire";

export interface TimecodeActions {
	api: TimecodesApiClient;
	events?: { onEvent(listener: (event: EventPayload) => void): () => unknown };
}

const TimecodeActionsContext = createContext<TimecodeActions | null>(null);

export function TimecodeActionsProvider({ children, api, events }: PropsWithChildren<TimecodeActions>) {
	return <TimecodeActionsContext.Provider value={{api, events}}>{children}</TimecodeActionsContext.Provider>;
}

export function useTimecodeActions(): TimecodeActions | null {
	return useContext(TimecodeActionsContext);
}
