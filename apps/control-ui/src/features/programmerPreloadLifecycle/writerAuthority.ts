import type { ProgrammerPreloadLifecycleAction } from "./contracts";
import type { ProgrammerPreloadLifecycleMutation } from "./store";
import type {
	LifecycleAuthority,
	ProgrammerPreloadLifecycleWriterOptions,
} from "./writer";

export interface LifecycleAuthorityGuard {
	storeScope: number;
	captureModeScope: number;
	valuesScope: number;
	queueScope: number;
	selectionScope: number;
	lifecycleScope: number;
	showGeneration: number;
}

export function captureLifecycleGuard(
	options: ProgrammerPreloadLifecycleWriterOptions,
): LifecycleAuthorityGuard {
	const guard = {
		storeScope: options.store.captureScope(),
		captureModeScope: options.captureModeStore.captureScope(),
		valuesScope: options.valuesStore.captureScope(),
		queueScope: options.queueStore.captureScope(),
		selectionScope: options.selectionStore.captureScope(),
		lifecycleScope: options.lifecycleStore.captureScope(),
		showGeneration: options.showStore.getSnapshot().authorityGeneration,
	};
	readyCapture(options, guard.captureModeScope);
	readyValues(options, guard.valuesScope);
	readyQueue(options, guard.queueScope);
	readySelection(options, guard.selectionScope);
	if (options.showStore.getSnapshot().showId !== options.scope.showId)
		throw new Error("The active Show authority is unavailable");
	if (options.readPreloadActive() === null)
		throw new Error("Authoritative Preload lifecycle status is unavailable");
	return guard;
}

export function matchesLifecycleGuard(
	authority: LifecycleAuthority,
	guard: LifecycleAuthorityGuard,
) {
	return (
		authority.storeScope === guard.storeScope &&
		authority.captureModeScope === guard.captureModeScope &&
		authority.valuesScope === guard.valuesScope &&
		authority.queueScope === guard.queueScope &&
		authority.selectionScope === guard.selectionScope &&
		authority.lifecycleScope === guard.lifecycleScope &&
		authority.showGeneration === guard.showGeneration
	);
}

export function captureLifecycleAuthority(
	options: ProgrammerPreloadLifecycleWriterOptions,
	action: ProgrammerPreloadLifecycleMutation,
	requestId: string,
): LifecycleAuthority {
	const storeScope = options.store.captureScope();
	const captureModeScope = options.captureModeStore.captureScope();
	const valuesScope = options.valuesStore.captureScope();
	const queueScope = options.queueStore.captureScope();
	const selectionScope = options.selectionStore.captureScope();
	const lifecycleScope = options.lifecycleStore.captureScope();
	const capture = readyCapture(options, captureModeScope);
	const values = readyValues(options, valuesScope);
	const queue = readyQueue(options, queueScope);
	const selectionRevision = readySelection(options, selectionScope);
	const show = options.showStore.getSnapshot();
	if (show.showId !== options.scope.showId)
		throw new Error("The active Show authority is unavailable");
	const runtimeScope = action === "go" ? options.runtimeStore.captureScope() : null;
	const runtime = runtimeScope === null ? null : readyRuntime(options, runtimeScope);
	if (options.readPreloadActive() === null)
		throw new Error("Authoritative Preload lifecycle status is unavailable");
	return {
		storeScope,
		captureModeScope,
		valuesScope,
		queueScope,
		selectionScope,
		lifecycleScope,
		runtimeScope,
		showGeneration: show.authorityGeneration,
		request: {
			requestId,
			expectedCaptureModeRevision: capture.revision,
			expectedValuesRevision: values.revision,
			expectedQueueRevision: queue.revision,
			expectedSelectionRevision: selectionRevision,
			action: requestAction(action, options.scope.showId, runtime),
		},
	};
}

function readyCapture(
	options: ProgrammerPreloadLifecycleWriterOptions,
	scope: number,
) {
	const state = options.captureModeStore.getSnapshot();
	if (
		!options.captureModeStore.isScopeCurrent(scope) ||
		state.showId !== options.scope.showId ||
		state.userId !== options.scope.userId ||
		state.status !== "ready" ||
		state.repairRequired ||
		!state.projection
	)
		throw new Error("Authoritative Programmer capture mode is unavailable");
	return state.projection;
}

function readyValues(
	options: ProgrammerPreloadLifecycleWriterOptions,
	scope: number,
) {
	const state = options.valuesStore.getSnapshot();
	if (
		!options.valuesStore.isScopeCurrent(scope) ||
		state.showId !== options.scope.showId ||
		state.userId !== options.scope.userId ||
		state.status !== "ready" ||
		state.repairRequired ||
		!state.projection
	)
		throw new Error("Authoritative Preload values are unavailable");
	return state.projection;
}

function readyQueue(
	options: ProgrammerPreloadLifecycleWriterOptions,
	scope: number,
) {
	const state = options.queueStore.getSnapshot();
	if (
		!options.queueStore.isScopeCurrent(scope) ||
		state.showId !== options.scope.showId ||
		state.userId !== options.scope.userId ||
		state.status !== "ready" ||
		state.repairRequired ||
		!state.projection
	)
		throw new Error("Authoritative Preload playback queue is unavailable");
	return state.projection;
}

function readySelection(
	options: ProgrammerPreloadLifecycleWriterOptions,
	scope: number,
) {
	const state = options.selectionStore.getSnapshot();
	const revision = options.selectionStore.authoritativeSelectionRevision(scope);
	if (
		state.showId !== options.scope.showId ||
		state.deskId !== options.scope.deskId ||
		state.status !== "ready" ||
		revision === null
	)
		throw new Error("Authoritative Programmer selection is unavailable");
	return revision;
}

function readyRuntime(
	options: ProgrammerPreloadLifecycleWriterOptions,
	scope: number,
) {
	const state = options.runtimeStore.getSnapshot();
	if (
		!options.runtimeStore.isScopeCurrent(scope) ||
		state.showId !== options.scope.showId ||
		state.deskId !== options.scope.deskId ||
		state.status !== "ready" ||
		state.showRevision === null ||
		state.eventSequence === null ||
		!state.desk
	)
		throw new Error("Authoritative Playback runtime is unavailable");
	return state;
}

function requestAction(
	action: ProgrammerPreloadLifecycleMutation,
	showId: string,
	runtime: { showRevision: number | null; eventSequence: number | null } | null,
): ProgrammerPreloadLifecycleAction {
	if (action !== "go") return { type: action };
	if (runtime?.showRevision == null || runtime.eventSequence == null)
		throw new Error("Authoritative Playback runtime is unavailable");
	return {
			type: "go",
			showId,
			expectedShowRevision: runtime.showRevision,
			expectedPlaybackEventSequence: runtime.eventSequence,
		};
}
