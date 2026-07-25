import {
	createContext,
	type PropsWithChildren,
	useContext,
} from "react";
import { browserDesktopBridge } from "./browserDesktopBridge";
import type { DesktopBridge } from "./types";

const DesktopContext = createContext<DesktopBridge>(browserDesktopBridge);

export function DesktopProvider({
	bridge,
	children,
}: PropsWithChildren<{ bridge: DesktopBridge }>) {
	return (
		<DesktopContext.Provider value={bridge}>{children}</DesktopContext.Provider>
	);
}

export function useDesktopBridge() {
	return useContext(DesktopContext);
}
