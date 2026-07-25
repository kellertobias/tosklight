import { createContext, type PropsWithChildren, useContext } from "react";

export interface DeskLoadingState {
	operationId: number;
	title: string;
	detail: string;
}

const DeskLoadingContext = createContext<DeskLoadingState | null>(null);

export function DeskLoadingStateProvider({
	children,
	loading,
}: PropsWithChildren<{ loading: DeskLoadingState | null }>) {
	return (
		<DeskLoadingContext.Provider value={loading}>
			{children}
		</DeskLoadingContext.Provider>
	);
}

/** A desk-wide blocking operation whose in-progress state must remain operator-visible. */
export function useDeskLoadingState(): DeskLoadingState | null {
	return useContext(DeskLoadingContext);
}
