import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useSyncExternalStore,
} from "react";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import {
	capturesProgrammerWrites,
	type ProgrammerCaptureModeProjection,
} from "./contracts";
import {
	ProgrammerCaptureModeSession,
	type ProgrammerCaptureModeSessionOptions,
} from "./session";
import {
	type ProgrammerCaptureModeState,
	ProgrammerCaptureModeStore,
} from "./store";
import type { ProgrammerCaptureModeEventTransport } from "./transport";

export interface ProgrammerCaptureModeViewProviderProps {
	showId: string | null;
	userId: string | null;
	authorityKey?: string;
	store: ProgrammerCaptureModeStore;
	transport: ProgrammerCaptureModeEventTransport | null;
	loadSnapshot: ProgrammerCaptureModeSessionOptions["loadSnapshot"];
	onSessionError?: (error: Error | null) => void;
}

/** Narrow lifecycle seam consumed by views whose writes depend on capture mode. */
export interface ProgrammerCaptureModeAuthority {
	store: ProgrammerCaptureModeStore;
	activate(): () => void;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<ProgrammerCaptureModeStore | null>(null);
const SessionContext = createContext<ProgrammerCaptureModeSession | null>(null);
const AuthorityContext = createContext<ProgrammerCaptureModeAuthority | null>(
	null,
);
const fallbackStore = new ProgrammerCaptureModeStore();

export function ProgrammerCaptureModeViewProvider({
	children,
	showId,
	userId,
	authorityKey = "",
	store,
	transport,
	loadSnapshot,
	onSessionError,
}: PropsWithChildren<ProgrammerCaptureModeViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && userId
				? new ProgrammerCaptureModeSession({
						showId,
						userId,
						authorityKey,
						store,
						transport,
						loadSnapshot,
						onError: onSessionError,
					})
				: null,
		[
			authorityKey,
			loadSnapshot,
			onSessionError,
			showId,
			store,
			transport,
			userId,
		],
	);
	const authority = useMemo<ProgrammerCaptureModeAuthority | null>(
		() =>
			session
				? {
						store,
						activate: () => session.activate(),
						repairAuthority: (error) => session.repairAuthority(error),
					}
				: null,
		[session, store],
	);
	useLayoutEffect(() => {
		store.reset(showId, userId, authorityKey);
	}, [authorityKey, showId, store, userId]);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<AuthorityContext.Provider value={authority}>
					{children}
				</AuthorityContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammerCaptureModeView(enabled = true) {
	return useProgrammerCaptureModeSelector(
		useCallback((state: ProgrammerCaptureModeState) => state.projection, []),
		Object.is,
		enabled,
	);
}

export function useProgrammerCaptureModeSelector<T>(
	selector: (state: ProgrammerCaptureModeState) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T | null {
	useCaptureModeViewActivation(enabled);
	const store = useProgrammerCaptureModeStore();
	const scopedSelector = useCallback(
		(state: ProgrammerCaptureModeState) => (enabled ? selector(state) : null),
		[enabled, selector],
	);
	const scopedEqual = useMemo(() => equalNullable(equal), [equal]);
	return useExternalSelection(store, scopedSelector, scopedEqual);
}

export function useProgrammerCaptureModeStatus() {
	return useExternalSelection(
		useProgrammerCaptureModeStore(),
		selectStatus,
		equalStatus,
	);
}

export function useProgrammerCaptureModeStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useProgrammerCaptureModeAuthority() {
	return useContext(AuthorityContext);
}

export function selectCapturesProgrammerWrites(
	state: ProgrammerCaptureModeState,
) {
	return capturesProgrammerWrites(state.projection);
}

function useCaptureModeViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerCaptureModeStore,
	selector: (state: ProgrammerCaptureModeState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{
		state: ProgrammerCaptureModeState | null;
		selector: ((state: ProgrammerCaptureModeState) => T) | null;
		equal: ((left: T, right: T) => boolean) | null;
		value?: T;
	}>({ state: null, selector: null, equal: null });
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (
			cache.current.state === state &&
			cache.current.selector === selector &&
			cache.current.equal === equal
		)
			return cache.current.value as T;
		const value = selector(state);
		if (cache.current.state && equal(cache.current.value as T, value)) {
			cache.current.state = state;
			cache.current.selector = selector;
			cache.current.equal = equal;
			return cache.current.value as T;
		}
		cache.current = { state, selector, equal, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}

function equalNullable<T>(equal: (left: T, right: T) => boolean) {
	return (left: T | null, right: T | null) =>
		left === null || right === null ? left === right : equal(left, right);
}

function selectStatus(state: ProgrammerCaptureModeState) {
	return {
		status: state.status,
		error: state.error,
		repairRequired: state.repairRequired,
	};
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return (
		left.status === right.status &&
		left.error === right.error &&
		left.repairRequired === right.repairRequired
	);
}

export type { ProgrammerCaptureModeProjection };
