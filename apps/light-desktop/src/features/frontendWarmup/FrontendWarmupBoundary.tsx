import { useEffect, useRef } from "react";
import {
	cueListIdentity,
	groupIdentity,
	identityKey,
	playbackIdentity,
} from "../playbackRuntime/contracts";
import { usePlaybackRuntimeAuthority } from "../playbackRuntime/PlaybackRuntimeView";
import { useProgrammerCaptureModeAuthority } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerLifecycleAuthority } from "../programmerLifecycle/ProgrammerLifecycleView";
import { useProgrammerPreloadPlaybackQueueAuthority } from "../programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import { useProgrammerPreloadValuesAuthority } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerPriorityAuthority } from "../programmerPriority/ProgrammerPriorityView";
import { useProgrammerValuesAuthority } from "../programmerValues/ProgrammerValuesView";
import { useProgrammingSelectionAuthority } from "../programmingInteraction/ProgrammingInteractionView";
import { deferredConnectionWarmupTasks } from "../server/connectionBootstrap";
import type { ServerState } from "../server/useServerState";
import type { ShowObjectKind } from "../showObjects/contracts";
import { useShowObjectsAuthority } from "../showObjects/ShowObjectsView";
import { useSpeedGroupRuntimeAuthority } from "../speedGroupRuntime/SpeedGroupRuntimeView";
import {
	FrontendWarmupCoordinator,
	type FrontendWarmupPriority,
	type FrontendWarmupTaskResult,
} from "./coordinator";
import {
	frontendPerformanceDiagnostics,
	serializedModelBytes,
} from "./diagnostics";

const SHOW_OBJECT_PRIORITIES: ReadonlyArray<{
	kind: ShowObjectKind;
	priority: "near-future" | "idle";
}> = [
	{ kind: "group", priority: "near-future" },
	{ kind: "preset", priority: "near-future" },
	{ kind: "playback_page", priority: "near-future" },
	{ kind: "dynamic", priority: "near-future" },
	{ kind: "cue_list", priority: "idle" },
	{ kind: "playback", priority: "idle" },
];
const PLAYBACK_DEPENDENCIES: readonly ShowObjectKind[] = [
	"group",
	"cue_list",
	"playback",
	"playback_page",
];
const MAX_WARM_PLAYBACK_IDENTITIES = 4096;

/**
 * Warms data prerequisites through provider authorities. It renders no hidden
 * window tree and owns all leases for exactly one show/session authority epoch.
 */
export function FrontendWarmupBoundary({
	showId,
	state,
}: {
	showId: string | null;
	state: ServerState;
}) {
	const stateRef = useRef(state);
	stateRef.current = state;
	const showObjects = useShowObjectsAuthority();
	const playback = usePlaybackRuntimeAuthority();
	const programmingSelection = useProgrammingSelectionAuthority();
	const programmerValues = useProgrammerValuesAuthority();
	const captureMode = useProgrammerCaptureModeAuthority();
	const programmerLifecycle = useProgrammerLifecycleAuthority();
	const preloadValues = useProgrammerPreloadValuesAuthority();
	const preloadPlaybackQueue = useProgrammerPreloadPlaybackQueueAuthority();
	const programmerPriority = useProgrammerPriorityAuthority();
	const speedGroupRuntime = useSpeedGroupRuntimeAuthority();

	useEffect(() => {
		const authorities = completeWarmupAuthorities({
			showObjects,
			playback,
			programmingSelection,
			programmerValues,
			captureMode,
			programmerLifecycle,
			preloadValues,
			preloadPlaybackQueue,
			programmerPriority,
			speedGroupRuntime,
		});
		if (!showId || !authorities) return;
		return startFrontendWarmup(stateRef.current, authorities);
	}, [
		captureMode,
		playback,
		preloadPlaybackQueue,
		preloadValues,
		programmerLifecycle,
		programmerPriority,
		programmerValues,
		programmingSelection,
		showId,
		showObjects,
		speedGroupRuntime,
	]);
	return null;
}

type WarmupAuthorities = {
	showObjects: NonNullable<ReturnType<typeof useShowObjectsAuthority>>;
	playback: NonNullable<ReturnType<typeof usePlaybackRuntimeAuthority>>;
	programmingSelection: NonNullable<
		ReturnType<typeof useProgrammingSelectionAuthority>
	>;
	programmerValues: NonNullable<
		ReturnType<typeof useProgrammerValuesAuthority>
	>;
	captureMode: NonNullable<ReturnType<typeof useProgrammerCaptureModeAuthority>>;
	programmerLifecycle: NonNullable<
		ReturnType<typeof useProgrammerLifecycleAuthority>
	>;
	preloadValues: NonNullable<
		ReturnType<typeof useProgrammerPreloadValuesAuthority>
	>;
	preloadPlaybackQueue: NonNullable<
		ReturnType<typeof useProgrammerPreloadPlaybackQueueAuthority>
	>;
	programmerPriority: NonNullable<
		ReturnType<typeof useProgrammerPriorityAuthority>
	>;
	speedGroupRuntime: NonNullable<
		ReturnType<typeof useSpeedGroupRuntimeAuthority>
	>;
};

type OptionalWarmupAuthorities = {
	[K in keyof WarmupAuthorities]: WarmupAuthorities[K] | null;
};

function completeWarmupAuthorities(
	authorities: OptionalWarmupAuthorities,
): WarmupAuthorities | null {
	return Object.values(authorities).every(Boolean)
		? (authorities as WarmupAuthorities)
		: null;
}

function startFrontendWarmup(
	state: ServerState,
	authorities: WarmupAuthorities,
) {
	if (frontendWarmupDisabled()) return markUsablePaintOnly();
	const coordinator = new FrontendWarmupCoordinator({
		concurrency: 2,
		onDiagnostics: (diagnostics) =>
			frontendPerformanceDiagnostics.setWarmup(diagnostics),
	});
	enqueueWarmupTasks(coordinator, state, authorities);
	let cancelled = false;
	void afterUsablePaint().then(() => {
		if (cancelled) return;
		frontendPerformanceDiagnostics.markFirstUsablePaint();
		coordinator.start();
	});
	return () => {
		cancelled = true;
		coordinator.cancel();
	};
}

function frontendWarmupDisabled() {
	return (
		typeof window !== "undefined" &&
		new URLSearchParams(window.location.search).has("frontend-warmup-disabled")
	);
}

function markUsablePaintOnly() {
	let cancelled = false;
	void afterUsablePaint().then(() => {
		if (!cancelled) frontendPerformanceDiagnostics.markFirstUsablePaint();
	});
	return () => {
		cancelled = true;
	};
}

function enqueueWarmupTasks(
	coordinator: FrontendWarmupCoordinator,
	state: ServerState,
	authorities: WarmupAuthorities,
) {
	for (const task of deferredConnectionWarmupTasks(state))
		coordinator.enqueue(task);
	enqueueStoreLease(
		coordinator,
		"programming:selection",
		"foreground",
		authorities.programmingSelection,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:capture-mode",
		"foreground",
		authorities.captureMode,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:values",
		"near-future",
		authorities.programmerValues,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:lifecycle",
		"idle",
		authorities.programmerLifecycle,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:priority",
		"idle",
		authorities.programmerPriority,
	);
	enqueueStoreLease(
		coordinator,
		"speed-group:runtime",
		"idle",
		authorities.speedGroupRuntime,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:preload-values",
		"idle",
		authorities.preloadValues,
	);
	enqueueStoreLease(
		coordinator,
		"programmer:preload-playback-queue",
		"idle",
		authorities.preloadPlaybackQueue,
	);
	for (const { kind, priority } of SHOW_OBJECT_PRIORITIES)
		coordinator.enqueue({
			key: `show-object:${kind}`,
			priority,
			run: (signal) =>
				acquireShowObjectLease(authorities.showObjects, kind, signal),
		});
	coordinator.enqueue({
		key: "playback:built-in-registry",
		priority: "idle",
		run: (signal) => acquirePlaybackRegistryLease(authorities, signal),
	});
}

function enqueueStoreLease<
	T extends {
		subscribe(listener: () => void): () => void;
		getSnapshot(): { status: string; error: Error | null };
	},
>(
	coordinator: FrontendWarmupCoordinator,
	key: string,
	priority: FrontendWarmupPriority,
	authority: { activate: () => () => void; store: T },
) {
	coordinator.enqueue({
		key,
		priority,
		run: (signal) =>
			acquireStoreLease(authority.activate, authority.store, signal),
	});
}

async function acquirePlaybackRegistryLease(
	{ showObjects, playback }: WarmupAuthorities,
	signal: AbortSignal,
): Promise<FrontendWarmupTaskResult> {
	await waitForStore(
		showObjects.store,
		(snapshot) =>
			PLAYBACK_DEPENDENCIES.every((kind) =>
				snapshot.readyCollections.has(kind),
			),
		signal,
	);
	const releases = new Map<string, () => void>();
	const synchronizeIdentities = () =>
		synchronizePlaybackIdentities(showObjects, playback, releases);
	const unsubscribe = showObjects.store.subscribe(synchronizeIdentities);
	const identities = synchronizeIdentities();
	const releaseDesk = playback.activateDeskWarm();
	try {
		await waitForStore(
			playback.store,
			(state) =>
				state.status === "ready" &&
				identities.every((identity) =>
					state.projections.has(identityKey(identity)),
				),
			signal,
		);
	} catch (reason) {
		releasePlaybackRegistry(unsubscribe, releases, releaseDesk);
		throw reason;
	}
	return {
		release: () =>
			releasePlaybackRegistry(unsubscribe, releases, releaseDesk),
		retainedBytes: serializedModelBytes(playback.store.getSnapshot()),
	};
}

function synchronizePlaybackIdentities(
	showObjects: WarmupAuthorities["showObjects"],
	playback: WarmupAuthorities["playback"],
	releases: Map<string, () => void>,
) {
	const identities = playbackIdentities(showObjects.store.getSnapshot());
	const nextKeys = new Set(identities.map(identityKey));
	for (const identity of identities) {
		const key = identityKey(identity);
		if (!releases.has(key))
			releases.set(key, playback.activateWarm(identity));
	}
	for (const [key, release] of releases) {
		if (nextKeys.has(key)) continue;
		release();
		releases.delete(key);
	}
	return identities;
}

function releasePlaybackRegistry(
	unsubscribe: () => void,
	releases: Map<string, () => void>,
	releaseDesk: () => void,
) {
	unsubscribe();
	for (const release of [...releases.values()].reverse()) release();
	releaseDesk();
}

function playbackIdentities(
	snapshot: ReturnType<
		NonNullable<
			ReturnType<typeof useShowObjectsAuthority>
		>["store"]["getSnapshot"]
	>,
) {
	return [
		...snapshot.groups.map(({ id }) => groupIdentity(id)),
		...snapshot.cueLists.map(({ id }) => cueListIdentity(id)),
		...snapshot.playbacks.map(({ body }) => playbackIdentity(body.number)),
	]
		.filter(
			(identity, index, values) =>
				values.findIndex(
					(candidate) => identityKey(candidate) === identityKey(identity),
				) === index,
		)
		.slice(0, MAX_WARM_PLAYBACK_IDENTITIES);
}

async function acquireShowObjectLease(
	authority: NonNullable<ReturnType<typeof useShowObjectsAuthority>>,
	kind: ShowObjectKind,
	signal: AbortSignal,
): Promise<FrontendWarmupTaskResult> {
	const release = authority.activate(kind);
	try {
		await waitForStore(
			authority.store,
			(snapshot) => snapshot.readyCollections.has(kind),
			signal,
		);
	} catch (reason) {
		release();
		throw reason;
	}
	return {
		release,
		retainedBytes: serializedModelBytes(
			showObjectCollection(authority.store.getSnapshot(), kind),
		),
	};
}

async function acquireStoreLease<
	T extends {
		subscribe(listener: () => void): () => void;
		getSnapshot(): { status: string; error: Error | null };
	},
>(
	activate: () => () => void,
	store: T,
	signal: AbortSignal,
): Promise<FrontendWarmupTaskResult> {
	const release = activate();
	try {
		await waitForStore(
			store,
			(snapshot) => snapshot.status === "ready",
			signal,
		);
	} catch (reason) {
		release();
		throw reason;
	}
	return {
		release,
		retainedBytes: serializedModelBytes(store.getSnapshot()),
	};
}

function waitForStore<S extends { status?: string; error?: Error | null }>(
	store: {
		subscribe(listener: () => void): () => void;
		getSnapshot(): S;
	},
	ready: (snapshot: S) => boolean,
	signal: AbortSignal,
) {
	return new Promise<void>((resolve, reject) => {
		let unsubscribe = () => {};
		const finish = (error?: Error) => {
			unsubscribe();
			signal.removeEventListener("abort", aborted);
			if (error) reject(error);
			else resolve();
		};
		const inspect = () => {
			const snapshot = store.getSnapshot();
			if (ready(snapshot)) finish();
			else if (snapshot.status === "error")
				finish(
					snapshot.error ?? new Error("Frontend warm-up authority failed"),
				);
		};
		const aborted = () =>
			finish(new DOMException("Frontend warm-up cancelled", "AbortError"));
		if (signal.aborted) return aborted();
		unsubscribe = store.subscribe(inspect);
		signal.addEventListener("abort", aborted, { once: true });
		inspect();
	});
}

function showObjectCollection(
	snapshot: ReturnType<
		NonNullable<
			ReturnType<typeof useShowObjectsAuthority>
		>["store"]["getSnapshot"]
	>,
	kind: ShowObjectKind,
) {
	if (kind === "group") return snapshot.groups;
	if (kind === "preset") return snapshot.presets;
	if (kind === "dynamic") return snapshot.dynamics;
	if (kind === "cue_list") return snapshot.cueLists;
	if (kind === "playback") return snapshot.playbacks;
	return snapshot.playbackPages;
}

function afterUsablePaint() {
	if (typeof requestAnimationFrame !== "function") return Promise.resolve();
	return new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}
