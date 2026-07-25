import {
	createContext,
	type PropsWithChildren,
	useContext,
} from "react";
import {
	inactiveSessionHandoff,
	type SessionHandoff,
} from "./sessionHandoff";

const SessionHandoffContext = createContext<SessionHandoff>(
	inactiveSessionHandoff,
);

export function SessionHandoffProvider({
	handoff,
	children,
}: PropsWithChildren<{ handoff: SessionHandoff }>) {
	return (
		<SessionHandoffContext.Provider value={handoff}>
			{children}
		</SessionHandoffContext.Provider>
	);
}

export function useSessionHandoff() {
	return useContext(SessionHandoffContext);
}
