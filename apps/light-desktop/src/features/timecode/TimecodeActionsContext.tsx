import { createContext, type PropsWithChildren, useContext } from "react";
import type { TimecodesApiClient } from "../../api/client/timecodes";

const TimecodeActionsContext = createContext<TimecodesApiClient | null>(null);

export function TimecodeActionsProvider({ children, api }: PropsWithChildren<{ api: TimecodesApiClient }>) {
	return <TimecodeActionsContext.Provider value={api}>{children}</TimecodeActionsContext.Provider>;
}

export function useTimecodeActions(): TimecodesApiClient | null {
	return useContext(TimecodeActionsContext);
}
