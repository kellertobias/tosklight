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
import { useProgrammerCaptureModeAuthority } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type {
	ProgrammerValuesActions,
	ProgrammerValuesProjection,
} from "./contracts";
import {
	ProgrammerValuesSession,
	type ProgrammerValuesSessionOptions,
} from "./session";
import { type ProgrammerValuesState, ProgrammerValuesStore } from "./store";
import type { ProgrammerValuesEventTransport } from "./transport";
import {
	ProgrammerValuesWriter,
	type ProgrammerValuesWriterOptions,
} from "./writer";

export interface ProgrammerValuesViewProviderProps {
	showId: string | null;
	userId: string | null;
	authorityKey?: string;
	store: ProgrammerValuesStore;
	transport: ProgrammerValuesEventTransport | null;
	loadSnapshot: ProgrammerValuesSessionOptions["loadSnapshot"];
	applyAction?: ProgrammerValuesWriterOptions["applyAction"] | null;
	actions?: ProgrammerValuesActions | null;
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

/** Stable lifecycle seam for action-only features that depend on exact values. */
export interface ProgrammerValuesAuthority {
	store: ProgrammerValuesStore;
	activate(): () => void;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<ProgrammerValuesStore | null>(null);
const SessionContext = createContext<ProgrammerValuesSession | null>(null);
const ActionsContext = createContext<ProgrammerValuesActions | null>(null);
const AuthorityContext = createContext<ProgrammerValuesAuthority | null>(null);
const fallbackStore = new ProgrammerValuesStore();

export function ProgrammerValuesViewProvider({
	children,
	showId,
	userId,
	authorityKey = "",
	store,
	transport,
	loadSnapshot,
	applyAction = null,
	actions = null,
	onSessionError,
	onMutationError,
}: PropsWithChildren<ProgrammerValuesViewProviderProps>) {
	const captureModeAuthority = useProgrammerCaptureModeAuthority();
	const session = useMemo(
		() =>
			showId && userId
				? new ProgrammerValuesSession({
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
	const writer = useMemo(
		() =>
			showId && userId && session && applyAction && captureModeAuthority
				? new ProgrammerValuesWriter({
						scope: { showId, userId },
						store,
						captureModeStore: captureModeAuthority.store,
						applyAction,
						repair: (error) => session.repairAuthority(error),
						repairCaptureMode: (error) =>
							captureModeAuthority.repairAuthority(error),
						onError: onMutationError,
					})
				: null,
		[
			applyAction,
			captureModeAuthority,
			onMutationError,
			session,
			showId,
			store,
			userId,
		],
	);
	const authority = useMemo<ProgrammerValuesAuthority | null>(
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
	useStrictModeSafeStop(writer);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<AuthorityContext.Provider value={authority}>
					<ActionsContext.Provider value={actions ?? writer}>
						{children}
					</ActionsContext.Provider>
				</AuthorityContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammerValuesView(enabled = true) {
	return useProgrammerValuesSelector(
		useCallback((state: ProgrammerValuesState) => state.projection, []),
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
	const scopedEqual = useMemo(() => equalNullable(equal), [equal]);
	return useExternalSelection(store, scopedSelector, scopedEqual);
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

export function useProgrammerValuesAuthority() {
	return useContext(AuthorityContext);
}

export function useProgrammerValuesStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useValuesViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	const captureModeAuthority = useProgrammerCaptureModeAuthority();
	useEffect(() => {
		if (!enabled) return;
		const releaseValues = session?.activate();
		const releaseCaptureMode = captureModeAuthority?.activate();
		return () => {
			releaseValues?.();
			releaseCaptureMode?.();
		};
	}, [captureModeAuthority, enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerValuesStore,
	selector: (state: ProgrammerValuesState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{
		state: ProgrammerValuesState | null;
		selector: ((state: ProgrammerValuesState) => T) | null;
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
