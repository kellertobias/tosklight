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
	ProgrammerPriorityActions,
	ProgrammerPriorityProjection,
	ProgrammerPriorityScope,
} from "./contracts";
import { ProgrammerPrioritySession } from "./session";
import { type ProgrammerPriorityState, ProgrammerPriorityStore } from "./store";
import type { ProgrammerPriorityTransport } from "./transport";
import { ProgrammerPriorityWriter } from "./writer";

export interface ProgrammerPriorityProviderProps {
	userId: string | null;
	authorityKey: string;
	store: ProgrammerPriorityStore;
	transport: ProgrammerPriorityTransport | null;
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

export interface ProgrammerPriorityView {
	projection: ProgrammerPriorityProjection | null;
	status: ProgrammerPriorityState["status"];
	error: Error | null;
	repairRequired: boolean;
	pending: boolean;
	ready: boolean;
}

const StoreContext = createContext<ProgrammerPriorityStore | null>(null);
const SessionContext = createContext<ProgrammerPrioritySession | null>(null);
const ActionsContext = createContext<ProgrammerPriorityActions | null>(null);
const fallbackStore = new ProgrammerPriorityStore();
const NO_SUBSCRIPTION = () => () => undefined;
const DISABLED_VIEW: ProgrammerPriorityView = {
	projection: null,
	status: "idle",
	error: null,
	repairRequired: false,
	pending: false,
	ready: false,
};

export function ProgrammerPriorityProvider({
	children,
	userId,
	authorityKey,
	store,
	transport,
	onSessionError,
	onMutationError,
}: PropsWithChildren<ProgrammerPriorityProviderProps>) {
	const scope = useMemo<ProgrammerPriorityScope | null>(
		() => (userId ? { userId } : null),
		[userId],
	);
	const session = useMemo(
		() =>
			scope && transport
				? new ProgrammerPrioritySession({
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
				? new ProgrammerPriorityWriter({
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
		store.reset(userId, authorityKey);
	}, [authorityKey, store, userId]);
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

/** Activates and observes only the authenticated user's lightweight priority. */
export function useProgrammerPriorityView(enabled = true) {
	usePriorityActivation(enabled);
	return usePrioritySelector(
		useCallback(
			(state: ProgrammerPriorityState) =>
				enabled ? selectView(state) : DISABLED_VIEW,
			[enabled],
		),
		equalView,
		enabled,
	);
}

/** Action-only controls still activate the exact authority needed for revision checks. */
export function useProgrammerPriorityActions(enabled = true) {
	usePriorityActivation(enabled);
	const actions = useContext(ActionsContext);
	return enabled ? actions : null;
}

export function useProgrammerPriorityStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function usePriorityActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function usePrioritySelector<T>(
	selector: (state: ProgrammerPriorityState) => T,
	equal: (left: T, right: T) => boolean,
	enabled: boolean,
) {
	const store = useProgrammerPriorityStore();
	const cache = useRef<{
		state: ProgrammerPriorityState | null;
		selector: ((state: ProgrammerPriorityState) => T) | null;
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

function selectView(state: ProgrammerPriorityState): ProgrammerPriorityView {
	return {
		projection: state.projection,
		status: state.status,
		error: state.error,
		repairRequired: state.repairRequired,
		pending: state.pendingRequestIds.length > 0,
		ready: state.status === "ready" && state.projection !== null,
	};
}

function equalView(
	left: ProgrammerPriorityView,
	right: ProgrammerPriorityView,
) {
	return (
		left.projection === right.projection &&
		left.status === right.status &&
		left.error === right.error &&
		left.repairRequired === right.repairRequired &&
		left.pending === right.pending &&
		left.ready === right.ready
	);
}
