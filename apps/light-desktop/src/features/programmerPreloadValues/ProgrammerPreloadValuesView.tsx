import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from "react";
import { capturesProgrammerWrites } from "../programmerCaptureMode/contracts";
import {
	useProgrammerCaptureModeAuthority,
	useProgrammerCaptureModeStore,
} from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type {
	ProgrammerPreloadValuesActions,
	ProgrammerPreloadValuesProjection,
} from "./contracts";
import {
	ProgrammerPreloadValuesSession,
	type ProgrammerPreloadValuesSessionOptions,
} from "./session";
import {
	type ProgrammerPreloadValuesState,
	ProgrammerPreloadValuesStore,
} from "./store";
import type { ProgrammerPreloadValuesEventTransport } from "./transport";
import {
	ProgrammerPreloadValuesWriter,
	type ProgrammerPreloadValuesWriterOptions,
} from "./writer";

export interface ProgrammerPreloadValuesViewProviderProps {
	showId: string | null;
	userId: string | null;
	authorityKey?: string;
	enabled?: boolean;
	store: ProgrammerPreloadValuesStore;
	transport: ProgrammerPreloadValuesEventTransport | null;
	loadSnapshot: ProgrammerPreloadValuesSessionOptions["loadSnapshot"];
	applyAction?: ProgrammerPreloadValuesWriterOptions["applyAction"] | null;
	actions?: ProgrammerPreloadValuesActions | null;
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

export interface ProgrammerPreloadValuesAuthority {
	store: ProgrammerPreloadValuesStore;
	activate(): () => void;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<ProgrammerPreloadValuesStore | null>(null);
const SessionContext = createContext<ProgrammerPreloadValuesSession | null>(
	null,
);
const ActionsContext = createContext<ProgrammerPreloadValuesActions | null>(
	null,
);
const AuthorityContext = createContext<ProgrammerPreloadValuesAuthority | null>(
	null,
);
const EnabledContext = createContext(false);
const fallbackStore = new ProgrammerPreloadValuesStore();

export function ProgrammerPreloadValuesViewProvider({
	children,
	showId,
	userId,
	authorityKey = "",
	enabled = true,
	store,
	transport,
	loadSnapshot,
	applyAction = null,
	actions = null,
	onSessionError,
	onMutationError,
}: PropsWithChildren<ProgrammerPreloadValuesViewProviderProps>) {
	const captureModeAuthority = useProgrammerCaptureModeAuthority();
	const captureModeStore = useProgrammerCaptureModeStore();
	const [trustedCaptureAuthority, setTrustedCaptureAuthority] =
		useState<typeof captureModeAuthority>(null);
	const captureState = useSyncExternalStore(
		captureModeStore.subscribe,
		captureModeStore.getSnapshot,
		captureModeStore.getSnapshot,
	);
	const captureEnabled =
		enabled &&
		captureModeAuthority !== null &&
		trustedCaptureAuthority === captureModeAuthority &&
		Boolean(showId) &&
		Boolean(userId) &&
		captureState.showId === showId &&
		captureState.userId === userId &&
		captureState.status === "ready" &&
		!captureState.repairRequired &&
		capturesProgrammerWrites(captureState.projection);
	const session = useMemo(
		() =>
			showId && userId
				? new ProgrammerPreloadValuesSession({
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
				? new ProgrammerPreloadValuesWriter({
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
	const authority = useMemo<ProgrammerPreloadValuesAuthority | null>(
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
	useLayoutEffect(() => {
		setTrustedCaptureAuthority(captureModeAuthority);
	}, [captureModeAuthority]);
	useStrictModeSafeStop(writer);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<AuthorityContext.Provider value={authority}>
					<EnabledContext.Provider value={captureEnabled}>
						<ActionsContext.Provider
							value={captureEnabled ? (actions ?? writer) : null}
						>
							{children}
						</ActionsContext.Provider>
					</EnabledContext.Provider>
				</AuthorityContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammerPreloadValuesView(enabled = true) {
	return useProgrammerPreloadValuesSelector(
		useCallback((state: ProgrammerPreloadValuesState) => state.projection, []),
		Object.is,
		enabled,
	);
}

export function useProgrammerPreloadValuesSelector<T>(
	selector: (state: ProgrammerPreloadValuesState) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T | null {
	const captureEnabled = useContext(EnabledContext);
	const active = enabled && captureEnabled;
	usePreloadViewActivation(active);
	const store = useProgrammerPreloadValuesStore();
	const scopedSelector = useCallback(
		(state: ProgrammerPreloadValuesState) => (active ? selector(state) : null),
		[active, selector],
	);
	const scopedEqual = useMemo(() => equalNullable(equal), [equal]);
	return useExternalSelection(store, scopedSelector, scopedEqual);
}

export function useProgrammerPreloadValuesStatus() {
	const enabled = useContext(EnabledContext);
	const status = useExternalSelection(
		useProgrammerPreloadValuesStore(),
		selectStatus,
		equalStatus,
	);
	return { ...status, enabled };
}

export function useProgrammerPreloadValuesActions() {
	return useContext(ActionsContext);
}

export function useProgrammerPreloadValuesStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useProgrammerPreloadValuesAuthority() {
	return useContext(AuthorityContext);
}

function usePreloadViewActivation(enabled: boolean) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!enabled || !session) return;
		return session.activate();
	}, [enabled, session]);
}

function useExternalSelection<T>(
	store: ProgrammerPreloadValuesStore,
	selector: (state: ProgrammerPreloadValuesState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const cache = useRef<{
		state: ProgrammerPreloadValuesState | null;
		selector: ((state: ProgrammerPreloadValuesState) => T) | null;
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

function selectStatus(state: ProgrammerPreloadValuesState) {
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

export type { ProgrammerPreloadValuesProjection };
