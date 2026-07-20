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
import type { ProgrammerPreloadPlaybackQueueProjection } from "./contracts";
import {
	ProgrammerPreloadPlaybackQueueSession,
	type ProgrammerPreloadPlaybackQueueSessionOptions,
} from "./session";
import {
	type ProgrammerPreloadPlaybackQueueState,
	ProgrammerPreloadPlaybackQueueStore,
} from "./store";
import type { ProgrammerPreloadPlaybackQueueEventTransport } from "./transport";

export interface ProgrammerPreloadPlaybackQueueViewProviderProps {
	showId: string | null;
	userId: string | null;
	authorityKey: string | null;
	store: ProgrammerPreloadPlaybackQueueStore;
	transport: ProgrammerPreloadPlaybackQueueEventTransport | null;
	loadSnapshot: ProgrammerPreloadPlaybackQueueSessionOptions["loadSnapshot"];
	onSessionError?: (error: Error | null) => void;
}

const StoreContext = createContext<ProgrammerPreloadPlaybackQueueStore | null>(
	null,
);
const SessionContext =
	createContext<ProgrammerPreloadPlaybackQueueSession | null>(null);
const fallbackStore = new ProgrammerPreloadPlaybackQueueStore();

export function ProgrammerPreloadPlaybackQueueViewProvider({
	children,
	showId,
	userId,
	authorityKey,
	store,
	transport,
	loadSnapshot,
	onSessionError,
}: PropsWithChildren<ProgrammerPreloadPlaybackQueueViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && userId && authorityKey
				? new ProgrammerPreloadPlaybackQueueSession({
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
	useLayoutEffect(() => {
		store.reset(showId, userId, authorityKey);
	}, [authorityKey, showId, store, userId]);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				{children}
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammerPreloadPlaybackQueueView(enabled = true) {
	return useProgrammerPreloadPlaybackQueueSelector(
		useCallback(
			(state: ProgrammerPreloadPlaybackQueueState) => state.projection,
			[],
		),
		Object.is,
		enabled,
	);
}

export function useProgrammerPreloadPlaybackQueueSelector<T>(
	selector: (state: ProgrammerPreloadPlaybackQueueState) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T | null {
	useQueueViewActivation(enabled);
	const scopedSelector = useCallback(
		(state: ProgrammerPreloadPlaybackQueueState) =>
			enabled ? selector(state) : null,
		[enabled, selector],
	);
	return useExternalSelection(
		useProgrammerPreloadPlaybackQueueStore(),
		scopedSelector,
		useMemo(() => equalNullable(equal), [equal]),
	);
}

export function useProgrammerPreloadPlaybackQueueStatus() {
	return useExternalSelection(
		useProgrammerPreloadPlaybackQueueStore(),
		selectStatus,
		equalStatus,
	);
}

export function useProgrammerPreloadPlaybackQueueStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useQueueViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate();
	}, [enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerPreloadPlaybackQueueStore,
	selector: (state: ProgrammerPreloadPlaybackQueueState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{
		state: ProgrammerPreloadPlaybackQueueState | null;
		selector: ((state: ProgrammerPreloadPlaybackQueueState) => T) | null;
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
			cache.current = { ...cache.current, state, selector, equal };
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

function selectStatus(state: ProgrammerPreloadPlaybackQueueState) {
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

export type { ProgrammerPreloadPlaybackQueueProjection };
