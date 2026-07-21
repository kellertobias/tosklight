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
import type { ExecuteCommandLine } from "./commandExecution";
import {
	type ProgrammingCommandLineActions,
	useProgrammingCommandLineActionsValue,
} from "./commandLineActionsValue";
import {
	ProgrammingCommandLineWriter,
	type ProgrammingCommandLineWriterOptions,
} from "./commandLineWriter";
import type {
	ProgrammingCapability,
	SelectionActionOutcome,
	SelectionRule,
} from "./contracts";
import {
	type ProgrammingGroupSelectionIntent,
	type ProgrammingSelectionGestureIntent,
	type ProgrammingSelectionReplacementIntent,
	ProgrammingSelectionWriter,
	type ProgrammingSelectionWriterOptions,
} from "./selectionWriter";
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
	authorityKey?: string;
	store: ProgrammingInteractionStore;
	transport: ProgrammingEventTransport | null;
	loadSnapshot: ProgrammingInteractionSessionOptions["loadSnapshot"];
	replaceCommandLine?: ProgrammingCommandLineWriterOptions["replace"];
	executeCommand?: ExecuteCommandLine;
	applySelection?: ProgrammingSelectionWriterOptions["apply"];
	onSessionError?: (error: Error | null) => void;
	onMutationError?: (error: Error | null) => void;
}

export interface ProgrammingSelectionActions {
	replace(
		intent: ProgrammingSelectionReplacementIntent,
	): Promise<SelectionActionOutcome | null>;
	gesture(
		intent: ProgrammingSelectionGestureIntent,
	): Promise<SelectionActionOutcome | null>;
	selectGroup(
		intent: ProgrammingGroupSelectionIntent,
	): Promise<SelectionActionOutcome | null>;
	applyRule(rule: SelectionRule): Promise<SelectionActionOutcome | null>;
}

/** Stable lifecycle seam for action-only features that depend on selection. */
export interface ProgrammingSelectionAuthority {
	store: ProgrammingInteractionStore;
	activate(): () => void;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<ProgrammingInteractionStore | null>(null);
const SessionContext = createContext<ProgrammingInteractionSession | null>(
	null,
);
const CommandLineActionsContext =
	createContext<ProgrammingCommandLineActions | null>(null);
const SelectionActionsContext =
	createContext<ProgrammingSelectionActions | null>(null);
const SelectionAuthorityContext =
	createContext<ProgrammingSelectionAuthority | null>(null);
const fallbackStore = new ProgrammingInteractionStore();

export function ProgrammingInteractionViewProvider({
	children,
	showId,
	deskId,
	authorityKey = "",
	store,
	transport,
	loadSnapshot,
	replaceCommandLine,
	executeCommand,
	applySelection,
	onSessionError,
	onMutationError,
}: PropsWithChildren<ProgrammingInteractionViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && deskId
				? new ProgrammingInteractionSession({
						showId,
						deskId,
						authorityKey,
						store,
						transport,
						loadSnapshot,
						onError: onSessionError,
					})
				: null,
		[
			authorityKey,
			deskId,
			loadSnapshot,
			onSessionError,
			showId,
			store,
			transport,
		],
	);
	const commandLineWriter = useMemo(
		() =>
			showId && deskId && replaceCommandLine
				? new ProgrammingCommandLineWriter({
						showId,
						deskId,
						store,
						replace: replaceCommandLine,
						loadSnapshot,
						onError: onMutationError,
					})
				: null,
		[
			authorityKey,
			deskId,
			loadSnapshot,
			onMutationError,
			replaceCommandLine,
			showId,
			store,
		],
	);
	const selectionWriter = useMemo(
		() =>
			showId && deskId && applySelection
				? new ProgrammingSelectionWriter({
						showId,
						deskId,
						store,
						apply: applySelection,
						loadSnapshot,
						onError: onMutationError,
					})
				: null,
		[
			authorityKey,
			applySelection,
			deskId,
			loadSnapshot,
			onMutationError,
			showId,
			store,
		],
	);
	const commandLineActions = useProgrammingCommandLineActionsValue(
		commandLineWriter,
		selectionWriter,
		store,
		executeCommand,
	);
	const selectionActions = useMemo<ProgrammingSelectionActions | null>(
		() =>
			selectionWriter
				? {
						replace: (intent) => selectionWriter.replace(intent),
						gesture: (intent) => selectionWriter.gesture(intent),
						selectGroup: (intent) => selectionWriter.selectGroup(intent),
						applyRule: (rule) => selectionWriter.applyRule(rule),
					}
				: null,
		[selectionWriter],
	);
	const selectionAuthority = useMemo<ProgrammingSelectionAuthority | null>(
		() =>
			session
				? {
						store,
						activate: () => session.activate("selection"),
						repairAuthority: (error) =>
							session.repairAuthority("selection", error),
					}
				: null,
		[session, store],
	);
	useLayoutEffect(() => {
		store.reset(showId, deskId, authorityKey);
		return () => session?.stop();
	}, [authorityKey, deskId, session, showId, store]);
	useEffect(() => () => commandLineWriter?.stop(), [commandLineWriter]);
	useEffect(() => () => selectionWriter?.stop(), [selectionWriter]);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<SelectionAuthorityContext.Provider value={selectionAuthority}>
					<CommandLineActionsContext.Provider value={commandLineActions}>
						<SelectionActionsContext.Provider value={selectionActions}>
							{children}
						</SelectionActionsContext.Provider>
					</CommandLineActionsContext.Provider>
				</SelectionAuthorityContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammingCommandLineView(enabled = true, observe = true) {
	useProgrammingCapabilityView("commandLine", enabled);
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled && observe ? state.commandLine : null,
			[enabled, observe],
		),
		Object.is,
		enabled,
	);
}

/**
 * Whether scoped command-line authority is installed for the current scope.
 *
 * This is a scalar selector so action-only consumers observe readiness without
 * rerendering for ordinary command text. It intentionally does not activate the
 * capability; the view or action hook that needs the data owns that.
 */
export function useProgrammingCommandLineReady(enabled = true) {
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled && state.commandLine !== null,
			[enabled],
		),
		Object.is,
		enabled,
	);
}

export function useProgrammingPendingCommandChoiceView(enabled = true) {
	useProgrammingCapabilityView("commandLine", enabled);
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled ? (state.commandLine?.pendingChoice ?? null) : null,
			[enabled],
		),
		Object.is,
		enabled,
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
		enabled,
	);
}

export function useProgrammingInteractionStatus() {
	return useProgrammingSelector(selectStatus, equalStatus);
}

export function useProgrammingInteractionStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useProgrammingCommandLineActions() {
	return useContext(CommandLineActionsContext);
}

export function useProgrammingSelectionActions(enabled = true) {
	const actions = useContext(SelectionActionsContext);
	useProgrammingCapabilityView("selection", enabled && actions !== null);
	return enabled ? actions : null;
}

export function useProgrammingSelectionAuthority() {
	return useContext(SelectionAuthorityContext);
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
	enabled = true,
) {
	const store = useProgrammingInteractionStore();
	const cache = useRef<{
		state: ProgrammingInteractionState | null;
		selector: ((state: ProgrammingInteractionState) => T) | null;
		equal: ((left: T, right: T) => boolean) | null;
		value?: T;
	}>({
		state: null,
		selector: null,
		equal: null,
	});
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
			cache.current.selector === selector &&
			cache.current.equal === equal &&
			cache.current.state &&
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

const NO_SUBSCRIPTION = () => () => undefined;

function selectStatus(state: ProgrammingInteractionState) {
	return { status: state.status, error: state.error };
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return left.status === right.status && left.error === right.error;
}
