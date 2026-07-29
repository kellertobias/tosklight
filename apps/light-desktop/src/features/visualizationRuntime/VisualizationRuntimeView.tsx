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
import type { VisualizationSnapshot } from "../../api/types";
import { desktopRuntimeAvailable } from "../../platform/desktop";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
	VisualizationRuntimeState,
	VisualizationRuntimeView,
} from "./contracts";
import { VisualizationRuntimeSession } from "./session";
import { VisualizationRuntimeStore } from "./store";
import type { VisualizationRuntimeTransport } from "./transport";

export interface VisualizationRuntimeProviderProps {
	showId: string | null;
	sessionId: string | null;
	authorityKey: string;
	transport: VisualizationRuntimeTransport | null;
	store?: VisualizationRuntimeStore;
	onError?: (error: Error | null) => void;
	desktopRole?: "owner" | "mirror";
	desktopAuthorityKey?: string;
}

export interface VisualizationRuntimeViewOptions {
	lane?: VisualizationRuntimeLane;
	enabled?: boolean;
	intervalMillis: number;
	reconcileSnapshots?: boolean;
	consumerId?: string;
}

const StoreContext = createContext<VisualizationRuntimeStore | null>(null);
const SessionContext = createContext<VisualizationRuntimeSession | null>(null);
const DesktopRuntimeRenderAckContext = createContext<(() => void) | null>(null);
const RemoteActivationContext = createContext<
	| ((
			lane: VisualizationRuntimeLane,
			intervalMillis: number,
			consumerId?: string,
	  ) => () => void)
	| null
>(null);
const fallbackStore = new VisualizationRuntimeStore();
const NO_SUBSCRIPTION = () => () => undefined;
const DISABLED_VIEW: VisualizationRuntimeView = {
	status: "idle",
	snapshot: null,
	error: null,
	ready: false,
};

export function VisualizationRuntimeProvider({
	children,
	showId,
	sessionId,
	authorityKey,
	transport,
	store: providedStore,
	onError,
	desktopRole = "owner",
	desktopAuthorityKey = authorityKey,
}: PropsWithChildren<VisualizationRuntimeProviderProps>) {
	const ownedStore = useRef<VisualizationRuntimeStore | null>(null);
	if (!ownedStore.current) ownedStore.current = new VisualizationRuntimeStore();
	const store = providedStore ?? ownedStore.current;
	const scope = useMemo<VisualizationRuntimeScope | null>(
		() => (showId && sessionId ? { showId, sessionId, authorityKey } : null),
		[authorityKey, sessionId, showId],
	);
	const session = useMemo(
		() =>
			scope && transport
				? new VisualizationRuntimeSession({
						scope,
						store,
						transport,
						onError,
					})
				: null,
		[onError, scope, store, transport],
	);
	useLayoutEffect(() => store.reset(scope), [scope, store]);
	useStrictModeSafeStop(session);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<DesktopVisualizationRuntimeBridge
					role={desktopRole}
					scope={scope}
					store={store}
					desktopAuthorityKey={desktopAuthorityKey}
					fallbackSession={session}
				>
					{children}
				</DesktopVisualizationRuntimeBridge>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

/** Claims one shared lane and observes only that lane's selected projection. */
export function useVisualizationRuntimeView({
	lane = "normal",
	enabled = true,
	intervalMillis,
	reconcileSnapshots = true,
	consumerId,
}: VisualizationRuntimeViewOptions): VisualizationRuntimeView {
	useVisualizationRuntimeActivation(lane, enabled, intervalMillis, consumerId);
	return useVisualizationRuntimeSelector(
		useCallback(
			(state: VisualizationRuntimeState) =>
				enabled ? selectLane(state, lane) : DISABLED_VIEW,
			[enabled, lane],
		),
		useCallback(
			(left: VisualizationRuntimeView, right: VisualizationRuntimeView) =>
				equalView(left, right, reconcileSnapshots),
			[reconcileSnapshots],
		),
		enabled,
	);
}

export function useVisualizationRuntimeSnapshot(
	options: VisualizationRuntimeViewOptions,
) {
	return useVisualizationRuntimeView(options).snapshot;
}

export function useVisualizationRuntimeStore() {
	return useContext(StoreContext) ?? fallbackStore;
}

export function useVisualizationRuntimeSnapshotSubscription(
	lane: VisualizationRuntimeLane,
	enabled: boolean,
	onSnapshot: (snapshot: VisualizationSnapshot) => void,
) {
	const store = useVisualizationRuntimeStore();
	const listener = useRef(onSnapshot);
	useLayoutEffect(() => {
		listener.current = onSnapshot;
	}, [onSnapshot]);
	useEffect(() => {
		if (!enabled) return;
		let installed: VisualizationSnapshot | null = null;
		const synchronize = () => {
			const snapshot = store.getSnapshot()[lane].snapshot;
			if (!snapshot || snapshot === installed) return;
			installed = snapshot;
			listener.current(snapshot);
		};
		synchronize();
		const unsubscribe = store.subscribe(synchronize);
		return () => {
			unsubscribe();
		};
	}, [enabled, lane, store]);
}

/**
 * Stable one-shot read through the scoped visualization transport, for consumers that
 * derive from a single authoritative snapshot (thumbnails, dialog seeding) instead of
 * observing a polling lane. Rejects while no runtime session is available.
 */
export function useVisualizationRuntimeRead(
	lane: VisualizationRuntimeLane = "normal",
) {
	const session = useContext(SessionContext);
	return useCallback((): Promise<VisualizationSnapshot> => {
		if (!session)
			return Promise.reject(
				new Error("The visualization runtime view is unavailable"),
			);
		return session.read(lane);
	}, [lane, session]);
}

function useVisualizationRuntimeActivation(
	lane: VisualizationRuntimeLane,
	enabled: boolean,
	intervalMillis: number,
	consumerId?: string,
) {
	const session = useContext(SessionContext);
	const remoteActivation = useContext(RemoteActivationContext);
	useEffect(() => {
		if (!enabled) return;
		if (remoteActivation)
			return remoteActivation(lane, intervalMillis, consumerId);
		if (!session) return;
		return session.activate(lane, intervalMillis, consumerId);
	}, [consumerId, enabled, intervalMillis, lane, remoteActivation, session]);
}

const DESKTOP_RUNTIME_CHANNEL = "tosklight-visualization-runtime-v1";

type DesktopRuntimeClaim = {
	type: "claim";
	showId: string;
	sessionId: string;
	authorityKey: string;
	claimId: string;
	lane: VisualizationRuntimeLane;
	intervalMillis: number;
	enabled: boolean;
	recordedAt: number;
};

type DesktopRuntimeStateMessage = {
	type: "state";
	showId: string;
	sessionId: string;
	authorityKey: string;
	state: {
		normal: DesktopRuntimeLaneStateMessage;
		preload: DesktopRuntimeLaneStateMessage;
	};
};

type DesktopRuntimeLaneStateMessage = {
	status: VisualizationRuntimeState["normal"]["status"];
	snapshot: VisualizationSnapshot | null;
	errorMessage: string | null;
};

type DesktopRuntimeMirrorRender = {
	type: "mirror-render";
	showId: string;
	sessionId: string;
	authorityKey: string;
	recordedAt: number;
};

function DesktopVisualizationRuntimeBridge({
	children,
	role,
	scope,
	store,
	desktopAuthorityKey,
	fallbackSession,
}: PropsWithChildren<{
	role: "owner" | "mirror";
	scope: VisualizationRuntimeScope | null;
	store: VisualizationRuntimeStore;
	desktopAuthorityKey: string;
	fallbackSession: VisualizationRuntimeSession | null;
}>) {
	if (!desktopBridgeAvailable())
		return (
			<DesktopRuntimeRenderAckContext.Provider value={null}>
				<RemoteActivationContext.Provider value={null}>
					{children}
				</RemoteActivationContext.Provider>
			</DesktopRuntimeRenderAckContext.Provider>
		);
	return role === "owner" ? (
		<DesktopRuntimeOwner
			scope={scope}
			store={store}
			desktopAuthorityKey={desktopAuthorityKey}
		>
			{children}
		</DesktopRuntimeOwner>
	) : (
		<DesktopRuntimeMirror
			scope={scope}
			store={store}
			desktopAuthorityKey={desktopAuthorityKey}
			fallbackSession={fallbackSession}
		>
			{children}
		</DesktopRuntimeMirror>
	);
}

function DesktopRuntimeOwner({
	children,
	scope,
	store,
	desktopAuthorityKey,
}: PropsWithChildren<{
	scope: VisualizationRuntimeScope | null;
	store: VisualizationRuntimeStore;
	desktopAuthorityKey: string;
}>) {
	const channelRef = useRef<BroadcastChannel | null>(null);
	const [claims, setClaims] = useState(new Map<string, DesktopRuntimeClaim>());
	const publish = useCallback(() => {
		if (!scope) return;
		const state = store.getSnapshot();
		channelRef.current?.postMessage({
			type: "state",
			showId: scope.showId,
			sessionId: scope.sessionId,
			authorityKey: desktopAuthorityKey,
			state: {
				normal: serializableLane(state.normal),
				preload: serializableLane(state.preload),
			},
		} satisfies DesktopRuntimeStateMessage);
	}, [desktopAuthorityKey, scope, store]);
	useEffect(() => {
		if (!scope) return;
		const channel = new BroadcastChannel(DESKTOP_RUNTIME_CHANNEL);
		channelRef.current = channel;
		const installClaim = (message: DesktopRuntimeClaim) => {
			if (
				message.showId !== scope.showId ||
				message.sessionId !== scope.sessionId ||
				message.authorityKey !== desktopAuthorityKey
			)
				return;
			setClaims((current) => {
				const next = new Map(current);
				if (message.enabled) next.set(message.claimId, message);
				else next.delete(message.claimId);
				return next;
			});
			publish();
		};
		channel.onmessage = (event) => {
			const message = event.data as Partial<
				DesktopRuntimeClaim | DesktopRuntimeMirrorRender
			>;
			if (message.type === "claim") {
				installClaim(message as DesktopRuntimeClaim);
				return;
			}
			if (
				message.type === "mirror-render" &&
				message.showId === scope.showId &&
				message.sessionId === scope.sessionId &&
				message.authorityKey === desktopAuthorityKey
			)
				frontendPerformanceDiagnostics.recordStageDesktopMirrorRender();
		};
		const unsubscribe = store.subscribe(publish);
		const sweep = window.setInterval(() => {
			const cutoff = Date.now() - 5_000;
			setClaims((current) => {
				const next = new Map(
					[...current].filter(([, claim]) => claim.recordedAt >= cutoff),
				);
				return next.size === current.size ? current : next;
			});
			publish();
		}, 2_000);
		publish();
		return () => {
			window.clearInterval(sweep);
			unsubscribe();
			channel.close();
			if (channelRef.current === channel) channelRef.current = null;
			setClaims(new Map());
		};
	}, [desktopAuthorityKey, publish, scope, store]);
	const settings = (lane: VisualizationRuntimeLane) => {
		const matching = [...claims.values()].filter(
			(claim) => claim.lane === lane,
		);
		return {
			enabled: matching.length > 0,
			intervalMillis: Math.min(
				...matching.map((claim) => claim.intervalMillis),
				1_000,
			),
		};
	};
	const normal = settings("normal");
	const preload = settings("preload");
	useVisualizationRuntimeActivation(
		"normal",
		normal.enabled,
		normal.intervalMillis,
		"desktop-mirror-normal",
	);
	useVisualizationRuntimeActivation(
		"preload",
		preload.enabled,
		preload.intervalMillis,
		"desktop-mirror-preload",
	);
	return (
		<DesktopRuntimeRenderAckContext.Provider value={null}>
			<RemoteActivationContext.Provider value={null}>
				{children}
			</RemoteActivationContext.Provider>
		</DesktopRuntimeRenderAckContext.Provider>
	);
}

function DesktopRuntimeMirror({
	children,
	scope,
	store,
	desktopAuthorityKey,
	fallbackSession,
}: PropsWithChildren<{
	scope: VisualizationRuntimeScope | null;
	store: VisualizationRuntimeStore;
	desktopAuthorityKey: string;
	fallbackSession: VisualizationRuntimeSession | null;
}>) {
	const channelRef = useRef<BroadcastChannel | null>(null);
	const claimsRef = useRef(new Map<string, DesktopRuntimeClaim>());
	const nextClaimId = useRef(0);
	const lastOwnerStateAt = useRef(0);
	const mirrorId = useRef(
		typeof crypto.randomUUID === "function"
			? crypto.randomUUID()
			: `${Date.now()}-${Math.random()}`,
	);
	const sendClaims = useCallback(() => {
		const channel = channelRef.current;
		if (!channel) return;
		const recordedAt = Date.now();
		for (const claim of claimsRef.current.values())
			channel.postMessage({ ...claim, recordedAt });
	}, []);
	useEffect(() => {
		if (!scope) return;
		const channel = new BroadcastChannel(DESKTOP_RUNTIME_CHANNEL);
		channelRef.current = channel;
		lastOwnerStateAt.current = Date.now();
		channel.onmessage = (event) => {
			const message = event.data as Partial<DesktopRuntimeStateMessage>;
			if (
				message.type !== "state" ||
				message.showId !== scope.showId ||
				message.sessionId !== scope.sessionId ||
				message.authorityKey !== desktopAuthorityKey ||
				!message.state
			)
				return;
			lastOwnerStateAt.current = Date.now();
			installMirroredLane(store, "normal", message.state.normal);
			installMirroredLane(store, "preload", message.state.preload);
		};
		sendClaims();
		const heartbeat = window.setInterval(sendClaims, 2_000);
		return () => {
			window.clearInterval(heartbeat);
			for (const claim of claimsRef.current.values())
				channel.postMessage({
					...claim,
					enabled: false,
					recordedAt: Date.now(),
				});
			channel.close();
			if (channelRef.current === channel) channelRef.current = null;
		};
	}, [desktopAuthorityKey, scope, sendClaims, store]);
	const activate = useCallback(
		(
			lane: VisualizationRuntimeLane,
			intervalMillis: number,
			consumerId?: string,
		) => {
			if (!scope) return () => undefined;
			nextClaimId.current++;
			const claimId = `${mirrorId.current}:${consumerId ?? "consumer"}:${nextClaimId.current}`;
			const claim: DesktopRuntimeClaim = {
				type: "claim",
				showId: scope.showId,
				sessionId: scope.sessionId,
				authorityKey: desktopAuthorityKey,
				claimId,
				lane,
				intervalMillis,
				enabled: true,
				recordedAt: Date.now(),
			};
			claimsRef.current.set(claimId, claim);
			channelRef.current?.postMessage(claim);
			let releaseFallback: (() => void) | undefined;
			const watchdog = window.setInterval(() => {
				const ownerIsFresh = Date.now() - lastOwnerStateAt.current <= 5_000;
				if (!ownerIsFresh && !releaseFallback && fallbackSession)
					releaseFallback = fallbackSession.activate(
						lane,
						intervalMillis,
						`desktop-fallback:${claimId}`,
					);
				else if (ownerIsFresh && releaseFallback) {
					releaseFallback();
					releaseFallback = undefined;
				}
			}, 2_000);
			return () => {
				window.clearInterval(watchdog);
				releaseFallback?.();
				claimsRef.current.delete(claimId);
				channelRef.current?.postMessage({
					...claim,
					enabled: false,
					recordedAt: Date.now(),
				});
			};
		},
		[desktopAuthorityKey, fallbackSession, scope],
	);
	const acknowledgeRender = useCallback(() => {
		if (!scope) return;
		channelRef.current?.postMessage({
			type: "mirror-render",
			showId: scope.showId,
			sessionId: scope.sessionId,
			authorityKey: desktopAuthorityKey,
			recordedAt: Date.now(),
		} satisfies DesktopRuntimeMirrorRender);
	}, [desktopAuthorityKey, scope]);
	return (
		<DesktopRuntimeRenderAckContext.Provider value={acknowledgeRender}>
			<RemoteActivationContext.Provider value={activate}>
				{children}
			</RemoteActivationContext.Provider>
		</DesktopRuntimeRenderAckContext.Provider>
	);
}

function serializableLane(lane: VisualizationRuntimeState["normal"]) {
	return {
		status: lane.status,
		snapshot: lane.snapshot,
		errorMessage: lane.error?.message ?? null,
	};
}

function installMirroredLane(
	store: VisualizationRuntimeStore,
	lane: VisualizationRuntimeLane,
	state: DesktopRuntimeLaneStateMessage,
) {
	const generation = store.captureScope();
	if (state.snapshot) store.install(lane, state.snapshot, generation);
	if (state.errorMessage) {
		store.setError(lane, new Error(state.errorMessage), generation);
		return;
	}
	if (state.status === "loading") store.setLoading(lane, generation);
	else if (state.status === "idle") store.setIdle(lane, generation);
	else if (state.status === "error")
		store.setError(
			lane,
			new Error("The desktop visualization owner reported an error"),
			generation,
		);
}

export function useDesktopVisualizationRuntimeRenderAcknowledgement() {
	return useContext(DesktopRuntimeRenderAckContext);
}

function desktopBridgeAvailable() {
	return typeof BroadcastChannel === "function" && desktopRuntimeAvailable();
}

function useVisualizationRuntimeSelector<T>(
	selector: (state: VisualizationRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
	enabled: boolean,
) {
	const store = useVisualizationRuntimeStore();
	const cache = useRef<{
		state: VisualizationRuntimeState | null;
		selector: ((state: VisualizationRuntimeState) => T) | null;
		value?: T;
	}>({ state: null, selector: null });
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.state === state && cache.current.selector === selector)
			return cache.current.value as T;
		const value = selector(state);
		if (
			cache.current.selector === selector &&
			cache.current.value !== undefined &&
			equal(cache.current.value, value)
		) {
			cache.current.state = state;
			return cache.current.value;
		}
		cache.current = { state, selector, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

function selectLane(
	state: VisualizationRuntimeState,
	lane: VisualizationRuntimeLane,
): VisualizationRuntimeView {
	const selected = state[lane];
	const ready = selected.status === "ready" && selected.snapshot !== null;
	return {
		status: selected.status,
		snapshot: ready ? selected.snapshot : null,
		error: selected.error,
		ready,
	};
}

function equalView(
	left: VisualizationRuntimeView,
	right: VisualizationRuntimeView,
	reconcileSnapshots: boolean,
) {
	return (
		left.status === right.status &&
		(!reconcileSnapshots || left.snapshot === right.snapshot) &&
		left.error === right.error &&
		left.ready === right.ready
	);
}
