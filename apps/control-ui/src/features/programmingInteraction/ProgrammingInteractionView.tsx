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
import {
	type CommandExecutionOutcome,
	createCommandLineExecution,
	type ExecuteCommandLine,
} from "./commandExecution";
import {
	type CommandLineExecutionResult,
	ProgrammingCommandLineWriter,
	type ProgrammingCommandLineWriterOptions,
} from "./commandLineWriter";
import type {
	CommandLinePatch,
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

export interface ProgrammingCommandLineActions {
	replace(text: string): Promise<boolean>;
	reset(): Promise<boolean>;
	flush(): Promise<boolean>;
	/** Settles pending edits, then runs the command against the scoped authority. */
	execute(value?: string): Promise<CommandExecutionOutcome>;
	executeAfterPendingWrites(
		execute: () => Promise<boolean>,
		optimisticReset: CommandLinePatch,
	): Promise<CommandLineExecutionResult>;
}

export interface ProgrammingSelectionActions {
	replace(intent: ProgrammingSelectionReplacementIntent): Promise<SelectionActionOutcome | null>;
	gesture(intent: ProgrammingSelectionGestureIntent): Promise<SelectionActionOutcome | null>;
	selectGroup(intent: ProgrammingGroupSelectionIntent): Promise<SelectionActionOutcome | null>;
	applyRule(rule: SelectionRule): Promise<SelectionActionOutcome | null>;
}

const StoreContext = createContext<ProgrammingInteractionStore | null>(null);
const SessionContext = createContext<ProgrammingInteractionSession | null>(null);
const CommandLineActionsContext =
	createContext<ProgrammingCommandLineActions | null>(null);
const SelectionActionsContext =
	createContext<ProgrammingSelectionActions | null>(null);
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
	const commandLineActions = useMemo<ProgrammingCommandLineActions | null>(() => {
		if (!commandLineWriter) return null;
		const executeAfterPendingWrites: ProgrammingCommandLineActions["executeAfterPendingWrites"] =
			(execute, optimisticReset) => {
				const run = () =>
					commandLineWriter.executeAfterPendingWrites(execute, optimisticReset);
				return selectionWriter
					? selectionWriter.runAfterPendingWrites(run, "write_failed")
					: run();
			};
		return {
			replace: (text) => commandLineWriter.replace(text),
			reset: () => commandLineWriter.replace(""),
			flush: () => commandLineWriter.flush(),
			execute: createCommandLineExecution({
				store,
				// Selection writes share this barrier, so Enter still settles every
				// pending desk edit before the command runs.
				executeAfterPendingWrites,
				execute: executeCommand,
			}),
			executeAfterPendingWrites,
		};
	}, [commandLineWriter, executeCommand, selectionWriter, store]);
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
	useLayoutEffect(() => {
		store.reset(showId, deskId, authorityKey);
		return () => session?.stop();
	}, [authorityKey, deskId, session, showId, store]);
	useEffect(() => () => commandLineWriter?.stop(), [commandLineWriter]);
	useEffect(() => () => selectionWriter?.stop(), [selectionWriter]);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<CommandLineActionsContext.Provider value={commandLineActions}>
					<SelectionActionsContext.Provider value={selectionActions}>
						{children}
					</SelectionActionsContext.Provider>
				</CommandLineActionsContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function useProgrammingCommandLineView(
	enabled = true,
	observe = true,
) {
	useProgrammingCapabilityView("commandLine", enabled);
	return useProgrammingSelector(
		useCallback(
			(state: ProgrammingInteractionState) =>
				enabled && observe ? state.commandLine : null,
			[enabled, observe],
		),
		Object.is,
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

export function useProgrammingCommandLineActions() {
	return useContext(CommandLineActionsContext);
}

export function useProgrammingSelectionActions(enabled = true) {
	const actions = useContext(SelectionActionsContext);
	useProgrammingCapabilityView("selection", enabled && actions !== null);
	return enabled ? actions : null;
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
