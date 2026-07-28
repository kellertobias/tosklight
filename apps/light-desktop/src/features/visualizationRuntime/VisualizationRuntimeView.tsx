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
import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
	VisualizationRuntimeState,
	VisualizationRuntimeView,
} from "./contracts";
import { VisualizationRuntimeSession } from "./session";
import { VisualizationRuntimeStore } from "./store";
import type { VisualizationRuntimeTransport } from "./transport";

export interface VisualizationRuntimeProviderProps {
	showId: string | null;
	sessionId: string | null;
	authorityKey: string;
	transport: VisualizationRuntimeTransport | null;
	store?: VisualizationRuntimeStore;
	onError?: (error: Error | null) => void;
}

export interface VisualizationRuntimeViewOptions {
	lane?: VisualizationRuntimeLane;
	enabled?: boolean;
	intervalMillis: number;
	reconcileSnapshots?: boolean;
}

const StoreContext = createContext<VisualizationRuntimeStore | null>(null);
const SessionContext = createContext<VisualizationRuntimeSession | null>(null);
const fallbackStore = new VisualizationRuntimeStore();
const NO_SUBSCRIPTION = () => () => undefined;
const DISABLED_VIEW: VisualizationRuntimeView = {
	status: "idle",
	snapshot: null,
	error: null,
	ready: false,
};

export function VisualizationRuntimeProvider({
	children,
	showId,
	sessionId,
	authorityKey,
	transport,
	store: providedStore,
	onError,
}: PropsWithChildren<VisualizationRuntimeProviderProps>) {
	const ownedStore = useRef<VisualizationRuntimeStore | null>(null);
	if (!ownedStore.current) ownedStore.current = new VisualizationRuntimeStore();
	const store = providedStore ?? ownedStore.current;
	const scope = useMemo<VisualizationRuntimeScope | null>(
		() =>
			showId && sessionId ? { showId, sessionId, authorityKey } : null,
		[authorityKey, sessionId, showId],
	);
	const session = useMemo(
		() =>
			scope && transport
				? new VisualizationRuntimeSession({
						scope,
						store,
						transport,
						onError,
					})
				: null,
		[onError, scope, store, transport],
	);
	useLayoutEffect(() => store.reset(scope), [scope, store]);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				{children}
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

/** Claims one shared lane and observes only that lane's selected projection. */
export function useVisualizationRuntimeView({
	lane = "normal",
	enabled = true,
	intervalMillis,
	reconcileSnapshots = true,
}: VisualizationRuntimeViewOptions): VisualizationRuntimeView {
	useVisualizationRuntimeActivation(lane, enabled, intervalMillis);
	return useVisualizationRuntimeSelector(
		useCallback(
			(state: VisualizationRuntimeState) =>
				enabled ? selectLane(state, lane) : DISABLED_VIEW,
			[enabled, lane],
		),
		useCallback(
			(left: VisualizationRuntimeView, right: VisualizationRuntimeView) =>
				equalView(left, right, reconcileSnapshots),
			[reconcileSnapshots],
		),
		enabled,
	);
}

export function useVisualizationRuntimeSnapshot(
	options: VisualizationRuntimeViewOptions,
) {
	return useVisualizationRuntimeView(options).snapshot;
}

export function useVisualizationRuntimeStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useVisualizationRuntimeSnapshotSubscription(
	lane: VisualizationRuntimeLane,
	enabled: boolean,
	onSnapshot: (snapshot: VisualizationSnapshot) => void,
) {
	const store = useVisualizationRuntimeStore();
	const listener = useRef(onSnapshot);
	useLayoutEffect(() => {
		listener.current = onSnapshot;
	}, [onSnapshot]);
	useEffect(() => {
		if (!enabled) return;
		let installed: VisualizationSnapshot | null = null;
		const synchronize = () => {
			const snapshot = store.getSnapshot()[lane].snapshot;
			if (!snapshot || snapshot === installed) return;
			installed = snapshot;
			listener.current(snapshot);
		};
		synchronize();
		const unsubscribe = store.subscribe(synchronize);
		return () => {
			unsubscribe();
		};
	}, [enabled, lane, store]);
}

/**
 * Stable one-shot read through the scoped visualization transport, for consumers that
 * derive from a single authoritative snapshot (thumbnails, dialog seeding) instead of
 * observing a polling lane. Rejects while no runtime session is available.
 */
export function useVisualizationRuntimeRead(
	lane: VisualizationRuntimeLane = "normal",
) {
	const session = useContext(SessionContext);
	return useCallback((): Promise<VisualizationSnapshot> => {
		if (!session)
			return Promise.reject(
				new Error("The visualization runtime view is unavailable"),
			);
		return session.read(lane);
	}, [lane, session]);
}

function useVisualizationRuntimeActivation(
	lane: VisualizationRuntimeLane,
	enabled: boolean,
	intervalMillis: number,
) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate(lane, intervalMillis);
	}, [enabled, intervalMillis, lane, session]);
}

function useVisualizationRuntimeSelector<T>(
	selector: (state: VisualizationRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
	enabled: boolean,
) {
	const store = useVisualizationRuntimeStore();
	const cache = useRef<{
		state: VisualizationRuntimeState | null;
		selector: ((state: VisualizationRuntimeState) => T) | null;
		value?: T;
	}>({ state: null, selector: null });
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.state === state && cache.current.selector === selector)
			return cache.current.value as T;
		const value = selector(state);
		if (
			cache.current.selector === selector &&
			cache.current.value !== undefined &&
			equal(cache.current.value, value)
		) {
			cache.current.state = state;
			return cache.current.value;
		}
		cache.current = { state, selector, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

function selectLane(
	state: VisualizationRuntimeState,
	lane: VisualizationRuntimeLane,
): VisualizationRuntimeView {
	const selected = state[lane];
	const ready = selected.status === "ready" && selected.snapshot !== null;
	return {
		status: selected.status,
		snapshot: ready ? selected.snapshot : null,
		error: selected.error,
		ready,
	};
}

function equalView(
	left: VisualizationRuntimeView,
	right: VisualizationRuntimeView,
	reconcileSnapshots: boolean,
) {
	return (
		left.status === right.status &&
		(!reconcileSnapshots || left.snapshot === right.snapshot) &&
		left.error === right.error &&
		left.ready === right.ready
	);
}
