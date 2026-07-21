import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
	useSyncExternalStore,
} from "react";
import { usePlaybackRuntimeAuthority } from "../playbackRuntime/PlaybackRuntimeView";
import { useProgrammerCaptureModeAuthority } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import type { ProgrammerCaptureModeState } from "../programmerCaptureMode/store";
import {
	useProgrammerLifecycleAuthority,
	useProgrammerLifecycleSelector,
} from "../programmerLifecycle/ProgrammerLifecycleView";
import type { ProgrammerLifecycleRow } from "../programmerLifecycle/contracts";
import type { ProgrammerLifecycleState } from "../programmerLifecycle/store";
import { useProgrammerPreloadPlaybackQueueAuthority } from "../programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import type { ProgrammerPreloadPlaybackQueueState } from "../programmerPreloadPlaybackQueue/store";
import { useProgrammerPreloadValuesAuthority } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerPreloadValuesState } from "../programmerPreloadValues/store";
import { useProgrammingSelectionAuthority } from "../programmingInteraction/ProgrammingInteractionView";
import type { ProgrammingInteractionState } from "../programmingInteraction/store";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	ProgrammerPreloadLifecycleActions,
	ProgrammerPreloadLifecycleScope,
	ProgrammerPreloadLifecycleTransport,
} from "./contracts";
import { ProgrammerPreloadLifecycleController } from "./controller";
import {
	type ProgrammerPreloadLifecycleState,
	ProgrammerPreloadLifecycleStore,
} from "./store";
import { ProgrammerPreloadLifecycleWriter } from "./writer";

interface ProgrammerPreloadLifecycleProviderProps {
	showId: string | null;
	userId: string | null;
	deskId: string | null;
	authorityKey: string;
	lifecycleAuthorityKey: string | null;
	showStore: ShowObjectsStore;
	store: ProgrammerPreloadLifecycleStore;
	transport: ProgrammerPreloadLifecycleTransport | null;
	onError?: (error: Error | null) => void;
}

export interface ProgrammerPreloadLifecycleView {
	ready: boolean;
	armed: boolean;
	active: boolean;
	pending: boolean;
	phase: "loading" | "idle" | "armed" | "active";
	error: Error | null;
	actions: ProgrammerPreloadLifecycleActions | null;
}

const StoreContext = createContext<ProgrammerPreloadLifecycleStore | null>(null);
const ActionsContext = createContext<ProgrammerPreloadLifecycleActions | null>(
	null,
);
const LifecycleAuthorityKeyContext = createContext<string | null>(null);
const fallbackStore = new ProgrammerPreloadLifecycleStore();

/** Action composition only; construction does not activate any dependent view. */
export function ProgrammerPreloadLifecycleProvider({
	children,
	showId,
	userId,
	deskId,
	authorityKey,
	lifecycleAuthorityKey,
	showStore,
	store,
	transport,
	onError,
}: PropsWithChildren<ProgrammerPreloadLifecycleProviderProps>) {
	const captureMode = useProgrammerCaptureModeAuthority();
	const values = useProgrammerPreloadValuesAuthority();
	const queue = useProgrammerPreloadPlaybackQueueAuthority();
	const selection = useProgrammingSelectionAuthority();
	const lifecycle = useProgrammerLifecycleAuthority();
	const runtime = usePlaybackRuntimeAuthority();
	const scope = useMemo<ProgrammerPreloadLifecycleScope | null>(
		() => (showId && userId && deskId ? { showId, userId, deskId } : null),
		[deskId, showId, userId],
	);
	const controller = useMemo(
		() =>
			scope && transport && captureMode && values && queue && selection && lifecycle && runtime
				? new ProgrammerPreloadLifecycleController({
						scope,
						runtime,
						onError,
						writer: new ProgrammerPreloadLifecycleWriter({
							scope,
							store,
							captureModeStore: captureMode.store,
							valuesStore: values.store,
							queueStore: queue.store,
							selectionStore: selection.store,
							lifecycleStore: lifecycle.store,
							showStore,
							runtimeStore: runtime.store,
							readPreloadActive: () =>
								readPreloadActive(
									lifecycle.store.getSnapshot(),
									scope.userId,
									lifecycleAuthorityKey,
								),
							transport,
							repair: {
								captureMode: captureMode.repairAuthority,
								values: values.repairAuthority,
								queue: queue.repairAuthority,
								selection: selection.repairAuthority,
								lifecycle: lifecycle.repairAuthority,
								runtime: runtime.repairAuthority,
							},
							onError,
						}),
					})
				: null,
		[
			authorityKey,
			captureMode,
			lifecycle,
			lifecycleAuthorityKey,
			onError,
			queue,
			runtime,
			scope,
			selection,
			showStore,
			store,
			transport,
			values,
		],
	);
	useLayoutEffect(() => {
		store.reset(showId, userId, deskId, authorityKey);
	}, [authorityKey, deskId, showId, store, userId]);
	useStrictModeSafeStop(controller);
	return (
		<StoreContext.Provider value={store}>
			<LifecycleAuthorityKeyContext.Provider value={lifecycleAuthorityKey}>
				<ActionsContext.Provider value={controller}>
					{children}
				</ActionsContext.Provider>
			</LifecycleAuthorityKeyContext.Provider>
		</StoreContext.Provider>
	);
}

/** Activates exact user/desk prerequisites only while a Preload control is mounted. */
export function useProgrammerPreloadLifecycleView(
	enabled = true,
): ProgrammerPreloadLifecycleView {
	const actions = useContext(ActionsContext);
	const lifecycleAuthorityKey = useContext(LifecycleAuthorityKeyContext);
	const store = useContext(StoreContext) ?? fallbackStore;
	const local = useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		store.getSnapshot,
		store.getSnapshot,
	);
	const showId = local.showId;
	const userId = local.userId;
	const deskId = local.deskId;
	const captureMode = useProgrammerCaptureModeAuthority();
	const values = useProgrammerPreloadValuesAuthority();
	const queue = useProgrammerPreloadPlaybackQueueAuthority();
	const selection = useProgrammingSelectionAuthority();
	useBaseActivation(enabled, captureMode, values, queue, selection);
	const captureBlind = useAuthoritySelection(
		captureMode?.store,
		useCallback(
			(state: ProgrammerCaptureModeState) =>
				captureBlindForScope(state, showId, userId),
			[showId, userId],
		),
		null,
	);
	const valuesReady = useAuthoritySelection(
		values?.store,
		useCallback(
			(state: ProgrammerPreloadValuesState) =>
				exactUserAuthorityReady(state, showId, userId),
			[showId, userId],
		),
		false,
	);
	const queueReady = useAuthoritySelection(
		queue?.store,
		useCallback(
			(state: ProgrammerPreloadPlaybackQueueState) =>
				exactUserAuthorityReady(state, showId, userId),
			[showId, userId],
		),
		false,
	);
	const selectionReady = useAuthoritySelection(
		selection?.store,
		useCallback(
			(state: ProgrammingInteractionState) =>
				state.showId === showId &&
				state.deskId === deskId &&
				state.status === "ready" &&
				state.selection !== null,
			[deskId, showId],
		),
		false,
	);
	const lifecycleRow = useProgrammerLifecycleSelector(
		useCallback(
			(state: ProgrammerLifecycleState) =>
				selectLifecycleRow(state, userId, lifecycleAuthorityKey),
			[lifecycleAuthorityKey, userId],
		),
		equalLifecycleSelection,
		enabled,
	);
	const armedAuthority = captureBlind ?? false;
	const armed = optimisticArmed(armedAuthority, local);
	const activeAuthority = lifecycleRow?.ready ? lifecycleRow.active : null;
	const active = optimisticActive(activeAuthority ?? false, local);
	const ready = Boolean(
			enabled &&
			actions &&
			lifecycleRow?.ready === true &&
			activeAuthority !== null &&
			captureBlind !== null &&
			valuesReady &&
			queueReady &&
			selectionReady,
	);
	return {
		ready,
		armed,
		active,
		pending: local.pending !== null,
		phase: !ready ? "loading" : armed ? "armed" : active ? "active" : "idle",
		error: local.error,
		actions: enabled ? actions : null,
	};
}

type Activatable = { activate(): () => void } | null;

function useBaseActivation(
	enabled: boolean,
	capture: Activatable,
	values: Activatable,
	queue: Activatable,
	selection: Activatable,
) {
	useEffect(() => {
		if (!enabled) return;
		const releases = [capture, values, queue, selection].map((authority) =>
			authority?.activate(),
		);
		return () => {
			for (const release of releases) release?.();
		};
	}, [capture, enabled, queue, selection, values]);
}

function useAuthoritySelection<T, Value>(
	store: { subscribe(listener: () => void): () => void; getSnapshot(): T } | null | undefined,
	select: (state: T) => Value,
	fallback: Value,
) {
	const getSnapshot = useCallback(
		() => (store ? select(store.getSnapshot()) : fallback),
		[fallback, select, store],
	);
	return useSyncExternalStore(
		store?.subscribe ?? NO_SUBSCRIPTION,
		getSnapshot,
		getSnapshot,
	);
}

function selectLifecycleRow(
	state: ProgrammerLifecycleState,
	userId: string | null,
	lifecycleAuthorityKey: string | null,
) {
	const row = state.projection?.programmers.find((item) => item.userId === userId);
	return {
		active: readRowPreloadActive(row),
		ready:
			row !== undefined &&
			state.authorityKey === lifecycleAuthorityKey &&
			state.status === "ready" &&
			!state.repairRequired,
	};
}

function captureBlindForScope(
	state: ProgrammerCaptureModeState,
	showId: string | null,
	userId: string | null,
) {
	return exactUserAuthorityReady(state, showId, userId)
		? (state.projection?.blind ?? null)
		: null;
}

function exactUserAuthorityReady(
	state: ProgrammerCaptureModeState | ProgrammerPreloadValuesState | ProgrammerPreloadPlaybackQueueState,
	showId: string | null,
	userId: string | null,
) {
	return Boolean(
		state.showId === showId &&
			state.userId === userId &&
			state.status === "ready" &&
			!state.repairRequired &&
			state.projection,
	);
}

function equalLifecycleSelection(
	left: ReturnType<typeof selectLifecycleRow>,
	right: ReturnType<typeof selectLifecycleRow>,
) {
	return left.active === right.active && left.ready === right.ready;
}

function readPreloadActive(
	state: ProgrammerLifecycleState,
	userId: string,
	lifecycleAuthorityKey: string | null,
) {
	if (
		state.authorityKey !== lifecycleAuthorityKey ||
		state.status !== "ready" ||
		state.repairRequired
	)
		return null;
	return readRowPreloadActive(
		state.projection?.programmers.find((row) => row.userId === userId),
	);
}

function readRowPreloadActive(row: ProgrammerLifecycleRow | undefined) {
	return row?.preloadActive ?? null;
}

function optimisticArmed(
	authoritative: boolean,
	state: ProgrammerPreloadLifecycleState,
) {
	if (state.pending?.action === "enter") return true;
	if (state.pending?.action === "go" || state.pending?.action === "release")
		return false;
	return authoritative;
}

function optimisticActive(
	authoritative: boolean,
	state: ProgrammerPreloadLifecycleState,
) {
	return state.pending?.optimisticActive ?? authoritative;
}

const NO_SUBSCRIPTION = () => () => undefined;
