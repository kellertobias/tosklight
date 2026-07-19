import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useSyncExternalStore,
} from "react";
import type { ProgrammingCapability } from "./contracts";
import {
	ProgrammingInteractionSession,
	type ProgrammingInteractionSessionOptions,
} from "./session";
import {
	type ProgrammingInteractionState,
	ProgrammingInteractionStore,
} from "./store";
import type { ProgrammingEventTransport } from "./transport";

interface ProgrammingInteractionViewProviderProps {
	showId: string | null;
	deskId: string | null;
	store: ProgrammingInteractionStore;
	transport: ProgrammingEventTransport | null;
	loadSnapshot: ProgrammingInteractionSessionOptions["loadSnapshot"];
	onError?: (error: Error | null) => void;
}

const StoreContext = createContext<ProgrammingInteractionStore | null>(null);
const SessionContext = createContext<ProgrammingInteractionSession | null>(null);
const fallbackStore = new ProgrammingInteractionStore();

export function ProgrammingInteractionViewProvider({
	children,
	showId,
	deskId,
	store,
	transport,
	loadSnapshot,
	onError,
}: PropsWithChildren<ProgrammingInteractionViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && deskId
				? new ProgrammingInteractionSession({
						showId,
						deskId,
						store,
						transport,
						loadSnapshot,
						onError,
					})
				: null,
		[deskId, loadSnapshot, onError, showId, store, transport],
	);
	useEffect(() => {
		if (!session) store.reset(showId, deskId);
		return () => session?.stop();
	}, [deskId, session, showId, store]);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				{children}
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammingCommandLineView(enabled = true) {
	useProgrammingCapabilityView("commandLine", enabled);
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled ? state.commandLine : null,
			[enabled],
		),
		Object.is,
	);
}

export function useProgrammingSelectionView(enabled = true) {
	useProgrammingCapabilityView("selection", enabled);
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled ? state.selection : null,
			[enabled],
		),
		Object.is,
	);
}

export function useProgrammingInteractionStatus() {
	return useProgrammingSelector(selectStatus, equalStatus);
}

export function useProgrammingInteractionStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

function useProgrammingCapabilityView(
	capability: ProgrammingCapability,
	enabled: boolean,
) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate(capability);
	}, [capability, enabled, session]);
}

function useProgrammingSelector<T>(
	selector: (state: ProgrammingInteractionState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const store = useProgrammingInteractionStore();
	const cache = useRef<{ state: ProgrammingInteractionState | null; value?: T }>({
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

function selectStatus(state: ProgrammingInteractionState) {
	return { status: state.status, error: state.error };
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return left.status === right.status && left.error === right.error;
}
