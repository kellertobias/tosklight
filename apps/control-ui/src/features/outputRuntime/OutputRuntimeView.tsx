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
import type {
	OutputRuntimeActions,
	OutputRuntimeProjection,
	OutputRuntimeScope,
} from "./contracts";
import { OutputRuntimeSession } from "./session";
import { type OutputRuntimeState, OutputRuntimeStore } from "./store";
import type { OutputRuntimeTransport } from "./transport";
import { OutputRuntimeWriter } from "./writer";

export interface OutputRuntimeProviderProps {
	showId: string | null;
	deskId: string | null;
	authorityKey: string;
	store: OutputRuntimeStore;
	transport: OutputRuntimeTransport | null;
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

export interface OutputRuntimeView {
	projection: OutputRuntimeProjection | null;
	status: OutputRuntimeState["status"];
	error: Error | null;
	repairRequired: boolean;
	pending: boolean;
	ready: boolean;
}

const StoreContext = createContext<OutputRuntimeStore | null>(null);
const SessionContext = createContext<OutputRuntimeSession | null>(null);
const ActionsContext = createContext<OutputRuntimeActions | null>(null);
const fallbackStore = new OutputRuntimeStore();
const NO_SUBSCRIPTION = () => () => undefined;
const DISABLED_VIEW: OutputRuntimeView = {
	projection: null,
	status: "idle",
	error: null,
	repairRequired: false,
	pending: false,
	ready: false,
};

export function OutputRuntimeProvider({
	children,
	showId,
	deskId,
	authorityKey,
	store,
	transport,
	onSessionError,
	onMutationError,
}: PropsWithChildren<OutputRuntimeProviderProps>) {
	const scope = useMemo<OutputRuntimeScope | null>(
		() => (showId && deskId ? { showId, deskId } : null),
		[deskId, showId],
	);
	const session = useMemo(
		() =>
			scope && transport
				? new OutputRuntimeSession({
						scope,
						authorityKey,
						store,
						transport,
						onError: onSessionError,
					})
				: null,
		[authorityKey, onSessionError, scope, store, transport],
	);
	const writer = useMemo(
		() =>
			scope && transport && session
				? new OutputRuntimeWriter({
						scope,
						store,
						transport,
						repair: (error) => session.repairAuthority(error),
						onError: onMutationError,
					})
				: null,
		[onMutationError, scope, session, store, transport],
	);
	useLayoutEffect(() => {
		store.reset(showId, deskId, authorityKey);
	}, [authorityKey, deskId, showId, store]);
	useStrictModeSafeStop(session);
	useStrictModeSafeStop(writer);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<ActionsContext.Provider value={writer}>
					{children}
				</ActionsContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

/** Activates and observes only the current Show's global output object. */
export function useOutputRuntimeView(enabled = true) {
	useOutputRuntimeActivation(enabled);
	return useOutputRuntimeSelector(
		useCallback(
			(state: OutputRuntimeState) =>
				enabled ? selectView(state) : DISABLED_VIEW,
			[enabled],
		),
		equalView,
		enabled,
	);
}

/** Action-only controls activate the authority needed for revision checks. */
export function useOutputRuntimeActions(enabled = true) {
	useOutputRuntimeActivation(enabled);
	const actions = useContext(ActionsContext);
	return enabled ? actions : null;
}

export function useOutputRuntimeStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useOutputRuntimeActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function useOutputRuntimeSelector<T>(
	selector: (state: OutputRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
	enabled: boolean,
) {
	const store = useOutputRuntimeStore();
	const cache = useRef<{
		state: OutputRuntimeState | null;
		selector: ((state: OutputRuntimeState) => T) | null;
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
		if (
			cache.current.state &&
			cache.current.selector === selector &&
			cache.current.equal === equal &&
			equal(cache.current.value as T, value)
		) {
			cache.current.state = state;
			return cache.current.value as T;
		}
		cache.current = { state, selector, equal, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

function selectView(state: OutputRuntimeState): OutputRuntimeView {
	return {
		projection: state.projection,
		status: state.status,
		error: state.error,
		repairRequired: state.repairRequired,
		pending: state.pendingRequestIds.length > 0,
		ready: state.status === "ready" && state.projection !== null,
	};
}

function equalView(left: OutputRuntimeView, right: OutputRuntimeView) {
	return (
		left.projection === right.projection &&
		left.status === right.status &&
		left.error === right.error &&
		left.repairRequired === right.repairRequired &&
		left.pending === right.pending &&
		left.ready === right.ready
	);
}
