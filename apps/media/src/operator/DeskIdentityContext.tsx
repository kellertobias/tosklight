import { createContext, type ReactNode, useContext } from "react";

const DeskIdentityContext = createContext<string | undefined>(undefined);

export function DeskIdentityProvider({
	showName,
	children,
}: {
	showName?: string;
	children: ReactNode;
}) {
	return (
		<DeskIdentityContext.Provider value={showName}>
			{children}
		</DeskIdentityContext.Provider>
	);
}

export function useDeskShowName() {
	return useContext(DeskIdentityContext);
}
