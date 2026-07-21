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
import type { ProgrammerLifecycleProjection } from "./contracts";
import {
	ProgrammerLifecycleSession,
	type ProgrammerLifecycleSessionOptions,
} from "./session";
import {
	type ProgrammerLifecycleState,
	ProgrammerLifecycleStore,
} from "./store";
import type { ProgrammerLifecycleEventTransport } from "./transport";

export interface ProgrammerLifecycleViewProviderProps {
	authorityKey: string | null;
	store: ProgrammerLifecycleStore;
	transport: ProgrammerLifecycleEventTransport | null;
	loadSnapshot: ProgrammerLifecycleSessionOptions["loadSnapshot"];
	onSessionError?: (error: Error | null) => void;
}

export interface ProgrammerLifecycleAuthority {
	store: ProgrammerLifecycleStore;
	activate(): () => void;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<ProgrammerLifecycleStore | null>(null);
const SessionContext = createContext<ProgrammerLifecycleSession | null>(null);
const AuthorityContext = createContext<ProgrammerLifecycleAuthority | null>(null);
const fallbackStore = new ProgrammerLifecycleStore();

export function ProgrammerLifecycleViewProvider({
	children,
	authorityKey,
	store,
	transport,
	loadSnapshot,
	onSessionError,
}: PropsWithChildren<ProgrammerLifecycleViewProviderProps>) {
	const session = useMemo(
		() =>
			authorityKey
				? new ProgrammerLifecycleSession({
						authorityKey,
						store,
						transport,
						loadSnapshot,
						onError: onSessionError,
					})
				: null,
		[authorityKey, loadSnapshot, onSessionError, store, transport],
	);
	const authority = useMemo<ProgrammerLifecycleAuthority | null>(
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
		store.reset(authorityKey);
	}, [authorityKey, store]);
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

export function useProgrammerLifecycleView(enabled = true) {
	return useProgrammerLifecycleSelector(
		useCallback((state: ProgrammerLifecycleState) => state.projection, []),
		Object.is,
		enabled,
	);
}

export function useProgrammerLifecycleSelector<T>(
	selector: (state: ProgrammerLifecycleState) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T | null {
	useLifecycleViewActivation(enabled);
	const scopedSelector = useCallback(
		(state: ProgrammerLifecycleState) => (enabled ? selector(state) : null),
		[enabled, selector],
	);
	return useExternalSelection(
		useProgrammerLifecycleStore(),
		scopedSelector,
		useMemo(() => equalNullable(equal), [equal]),
	);
}

export function useProgrammerLifecycleStatus() {
	return useExternalSelection(
		useProgrammerLifecycleStore(),
		selectStatus,
		equalStatus,
	);
}

export function useProgrammerLifecycleStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useProgrammerLifecycleAuthority() {
	return useContext(AuthorityContext);
}

function useLifecycleViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate();
	}, [enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerLifecycleStore,
	selector: (state: ProgrammerLifecycleState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{
		state: ProgrammerLifecycleState | null;
		selector: ((state: ProgrammerLifecycleState) => T) | null;
		value?: T;
	}>({ state: null, selector: null });
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.state === state && cache.current.selector === selector)
			return cache.current.value as T;
		const value = selector(state);
		if (cache.current.state && equal(cache.current.value as T, value)) {
			cache.current = { state, selector, value: cache.current.value };
			return cache.current.value as T;
		}
		cache.current = { state, selector, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}

function equalNullable<T>(equal: (left: T, right: T) => boolean) {
	return (left: T | null, right: T | null) =>
		left === null || right === null ? left === right : equal(left, right);
}

function selectStatus(state: ProgrammerLifecycleState) {
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

export type { ProgrammerLifecycleProjection };
