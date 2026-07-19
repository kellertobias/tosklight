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
import type {
	ProgrammerValuesActions,
	ProgrammerValuesProjection,
} from "./contracts";
import {
	ProgrammerValuesSession,
	type ProgrammerValuesSessionOptions,
} from "./session";
import {
	type ProgrammerValuesState,
	ProgrammerValuesStore,
} from "./store";
import type { ProgrammerValuesEventTransport } from "./transport";

export interface ProgrammerValuesViewProviderProps {
	showId: string | null;
	userId: string | null;
	store: ProgrammerValuesStore;
	transport: ProgrammerValuesEventTransport | null;
	loadSnapshot: ProgrammerValuesSessionOptions["loadSnapshot"];
	actions?: ProgrammerValuesActions | null;
	onSessionError?: (error: Error | null) => void;
}

const StoreContext = createContext<ProgrammerValuesStore | null>(null);
const SessionContext = createContext<ProgrammerValuesSession | null>(null);
const ActionsContext = createContext<ProgrammerValuesActions | null>(null);
const fallbackStore = new ProgrammerValuesStore();

export function ProgrammerValuesViewProvider({
	children,
	showId,
	userId,
	store,
	transport,
	loadSnapshot,
	actions = null,
	onSessionError,
}: PropsWithChildren<ProgrammerValuesViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && userId
				? new ProgrammerValuesSession({
						showId,
						userId,
						store,
						transport,
						loadSnapshot,
						onError: onSessionError,
					})
				: null,
		[loadSnapshot, onSessionError, showId, store, transport, userId],
	);
	useLayoutEffect(() => {
		store.reset(showId, userId);
		return () => session?.stop();
	}, [session, showId, store, userId]);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<ActionsContext.Provider value={actions}>
					{children}
				</ActionsContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammerValuesView(enabled = true) {
	return useProgrammerValuesSelector(
		useCallback(
			(state: ProgrammerValuesState) => state.projection,
			[],
		),
		Object.is,
		enabled,
	);
}

export function useProgrammerValuesSelector<T>(
	selector: (state: ProgrammerValuesState) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T | null {
	useValuesViewActivation(enabled);
	const store = useProgrammerValuesStore();
	const scopedSelector = useCallback(
		(state: ProgrammerValuesState) => (enabled ? selector(state) : null),
		[enabled, selector],
	);
	return useExternalSelection(store, scopedSelector, equalNullable(equal));
}

export function useProgrammerValuesStatus() {
	return useExternalSelection(
		useProgrammerValuesStore(),
		selectStatus,
		equalStatus,
	);
}

export function useProgrammerValuesActions() {
	return useContext(ActionsContext);
}

export function useProgrammerValuesStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useValuesViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerValuesStore,
	selector: (state: ProgrammerValuesState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{ state: ProgrammerValuesState | null; value?: T }>({
		state: null,
	});
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.state === state) return cache.current.value as T;
		const value = selector(state);
		if (cache.current.state && equal(cache.current.value as T, value)) {
			cache.current.state = state;
			return cache.current.value as T;
		}
		cache.current = { state, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}

function equalNullable<T>(equal: (left: T, right: T) => boolean) {
	return (left: T | null, right: T | null) =>
		left === null || right === null ? left === right : equal(left, right);
}

function selectStatus(state: ProgrammerValuesState) {
	return {
		status: state.status,
		error: state.error,
		repairRequired: state.repairRequired,
		pendingRequestIds: state.pendingRequestIds,
	};
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return (
		left.status === right.status &&
		left.error === right.error &&
		left.repairRequired === right.repairRequired &&
		left.pendingRequestIds === right.pendingRequestIds
	);
}

export type { ProgrammerValuesProjection };
