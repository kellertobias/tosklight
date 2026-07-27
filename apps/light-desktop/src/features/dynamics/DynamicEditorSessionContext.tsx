import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
	useState,
} from "react";

export type DynamicEditorTask = "curves" | "phase" | "speed";

export interface DynamicEditorSession {
	dynamicId: string;
	task: DynamicEditorTask;
	primaryLaneId: string | null;
}

interface DynamicEditorSessionValue {
	session: DynamicEditorSession | null;
	open(session: DynamicEditorSession): void;
	update(update: Partial<Omit<DynamicEditorSession, "dynamicId">>): void;
	close(dynamicId?: string): void;
}

const DynamicEditorSessionContext =
	createContext<DynamicEditorSessionValue | null>(null);

export function DynamicEditorSessionProvider({ children }: PropsWithChildren) {
	const [session, setSession] = useState<DynamicEditorSession | null>(null);
	const open = useCallback(
		(next: DynamicEditorSession) => setSession(next),
		[],
	);
	const update = useCallback(
		(next: Partial<Omit<DynamicEditorSession, "dynamicId">>) =>
			setSession((current) => (current ? { ...current, ...next } : null)),
		[],
	);
	const close = useCallback(
		(dynamicId?: string) =>
			setSession((current) =>
				!dynamicId || current?.dynamicId === dynamicId ? null : current,
			),
		[],
	);
	const value = useMemo(
		() => ({ session, open, update, close }),
		[close, open, session, update],
	);
	return (
		<DynamicEditorSessionContext.Provider value={value}>
			{children}
		</DynamicEditorSessionContext.Provider>
	);
}

export function useDynamicEditorSession() {
	const value = useContext(DynamicEditorSessionContext);
	if (!value)
		throw new Error(
			"useDynamicEditorSession must be used within DynamicEditorSessionProvider",
		);
	return value;
}
