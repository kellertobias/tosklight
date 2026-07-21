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
	SpeedGroupAuthorityProjection,
	SpeedGroupRuntimeActions,
	SpeedGroupRuntimeScope,
} from "./contracts";
import { SpeedGroupRuntimeSession } from "./session";
import { SpeedGroupRuntimeStore } from "./store";
import type { SpeedGroupRuntimeState } from "./storeState";
import type { SpeedGroupRuntimeTransport } from "./transport";
import { SpeedGroupRuntimeWriter } from "./writer";

export interface SpeedGroupRuntimeProviderProps {
	deskId: string | null;
	authorityKey: string;
	store: SpeedGroupRuntimeStore;
	transport: SpeedGroupRuntimeTransport | null;
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

export interface SpeedGroupRuntimeView {
	projection: SpeedGroupAuthorityProjection | null;
	status: SpeedGroupRuntimeState["status"];
	error: Error | null;
	repairRequired: boolean;
	pending: boolean;
	ready: boolean;
}

const StoreContext = createContext<SpeedGroupRuntimeStore | null>(null);
const SessionContext = createContext<SpeedGroupRuntimeSession | null>(null);
const ActionsContext = createContext<SpeedGroupRuntimeActions | null>(null);
const fallbackStore = new SpeedGroupRuntimeStore();
const NO_SUBSCRIPTION = () => () => undefined;
const DISABLED_VIEW: SpeedGroupRuntimeView = {
	projection: null,
	status: "idle",
	error: null,
	repairRequired: false,
	pending: false,
	ready: false,
};

export function SpeedGroupRuntimeProvider({
	children,
	deskId,
	authorityKey,
	store,
	transport,
	onSessionError,
	onMutationError,
}: PropsWithChildren<SpeedGroupRuntimeProviderProps>) {
	const scope = useMemo<SpeedGroupRuntimeScope | null>(
		() => (deskId ? { deskId } : null),
		[deskId],
	);
	const session = useMemo(
		() =>
			scope && transport
				? new SpeedGroupRuntimeSession({
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
				? new SpeedGroupRuntimeWriter({
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
		store.reset(deskId, authorityKey);
	}, [authorityKey, deskId, store]);
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

/** Activates only the manual Speed Group projection needed by its caller. */
export function useSpeedGroupRuntimeView(enabled = true) {
	useSpeedGroupActivation(enabled);
	return useSpeedGroupSelector(
		useCallback(
			(state: SpeedGroupRuntimeState) =>
				enabled ? selectView(state) : DISABLED_VIEW,
			[enabled],
		),
		equalView,
		enabled,
	);
}

/** Action-only controls activate the authority needed for revision checks. */
export function useSpeedGroupRuntimeActions(enabled = true) {
	useSpeedGroupActivation(enabled);
	const actions = useContext(ActionsContext);
	return enabled ? actions : null;
}

export function useSpeedGroupRuntimeStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useSpeedGroupActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function useSpeedGroupSelector<T>(
	selector: (state: SpeedGroupRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
	enabled: boolean,
) {
	const store = useSpeedGroupRuntimeStore();
	const cache = useRef<{
		state: SpeedGroupRuntimeState | null;
		selector: ((state: SpeedGroupRuntimeState) => T) | null;
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

function selectView(state: SpeedGroupRuntimeState): SpeedGroupRuntimeView {
	return {
		projection: state.projection,
		status: state.status,
		error: state.error,
		repairRequired: state.repairRequired,
		pending: state.pendingRequestIds.length > 0,
		ready: state.status === "ready" && state.projection !== null,
	};
}

function equalView(left: SpeedGroupRuntimeView, right: SpeedGroupRuntimeView) {
	return (
		left.projection === right.projection &&
		left.status === right.status &&
		left.error === right.error &&
		left.repairRequired === right.repairRequired &&
		left.pending === right.pending &&
		left.ready === right.ready
	);
}
