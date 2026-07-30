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
	includeDynamicStack?: boolean;
	deliveryIntervalMillis?: number;
	snapshotEqual?: (
		left: VisualizationSnapshot | null,
		right: VisualizationSnapshot | null,
	) => boolean;
}

const StoreContext = createContext<VisualizationRuntimeStore | null>(null);
const SessionContext = createContext<VisualizationRuntimeSession | null>(null);
const DesktopRuntimeRenderAckContext = createContext<(() => void) | null>(null);
const RemoteActivationContext = createContext<
	| ((
			lane: VisualizationRuntimeLane,
			intervalMillis: number,
			consumerId?: string,
			includeDynamicStack?: boolean,
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
	includeDynamicStack = false,
	deliveryIntervalMillis = 0,
	snapshotEqual = Object.is,
}: VisualizationRuntimeViewOptions): VisualizationRuntimeView {
	useVisualizationRuntimeActivation(
		lane,
		enabled,
		intervalMillis,
		consumerId,
		includeDynamicStack,
	);
	return useVisualizationRuntimeSelector(
		useCallback(
			(state: VisualizationRuntimeState) =>
				enabled ? selectLane(state, lane) : DISABLED_VIEW,
			[enabled, lane],
		),
		useCallback(
			(left: VisualizationRuntimeView, right: VisualizationRuntimeView) =>
				equalView(left, right, reconcileSnapshots, snapshotEqual),
			[reconcileSnapshots, snapshotEqual],
		),
		enabled,
		deliveryIntervalMillis,
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
	options?: { dynamicStackOnly?: boolean; fixtureIds?: readonly string[] },
) {
	const session = useContext(SessionContext);
	const dynamicStackOnly = options?.dynamicStackOnly;
	const fixtureIds = options?.fixtureIds?.join(",");
	return useCallback((): Promise<VisualizationSnapshot> => {
		if (!session)
			return Promise.reject(
				new Error("The visualization runtime view is unavailable"),
			);
		return session.read(lane, {
			dynamicStackOnly,
			fixtureIds: fixtureIds ? fixtureIds.split(",") : undefined,
		});
	}, [dynamicStackOnly, fixtureIds, lane, session]);
}

function useVisualizationRuntimeActivation(
	lane: VisualizationRuntimeLane,
	enabled: boolean,
	intervalMillis: number,
	consumerId?: string,
	includeDynamicStack = false,
) {
	const session = useContext(SessionContext);
	const remoteActivation = useContext(RemoteActivationContext);
	useEffect(() => {
		if (!enabled) return;
		if (remoteActivation)
			return remoteActivation(
				lane,
				intervalMillis,
				consumerId,
				includeDynamicStack,
			);
		if (!session) return;
		return session.activate(
			lane,
			intervalMillis,
			consumerId,
			includeDynamicStack,
		);
	}, [
		consumerId,
		enabled,
		includeDynamicStack,
		intervalMillis,
		lane,
		remoteActivation,
		session,
	]);
}

const DESKTOP_RUNTIME_CHANNEL = "tosklight-visualization-runtime-v1";

type DesktopRuntimeMirrorRender = {
	type: "mirror-render";
	showId: string;
	authorityKey: string;
	recordedAt: number;
};

type DesktopRuntimeOwnerHeartbeat = {
	type: "owner-heartbeat";
	showId: string;
	authorityKey: string;
	recordedAt: number;
};

function DesktopVisualizationRuntimeBridge({
	children,
	role,
	scope,
	desktopAuthorityKey,
	fallbackSession,
}: PropsWithChildren<{
	role: "owner" | "mirror";
	scope: VisualizationRuntimeScope | null;
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
			desktopAuthorityKey={desktopAuthorityKey}
		>
			{children}
		</DesktopRuntimeOwner>
	) : (
		<DesktopRuntimeMirror
			scope={scope}
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
	desktopAuthorityKey,
}: PropsWithChildren<{
	scope: VisualizationRuntimeScope | null;
	desktopAuthorityKey: string;
}>) {
	const channelRef = useRef<BroadcastChannel | null>(null);
	useEffect(() => {
		if (!scope) return;
		const channel = new BroadcastChannel(DESKTOP_RUNTIME_CHANNEL);
		channelRef.current = channel;
		const sendHeartbeat = () =>
			channel.postMessage({
				type: "owner-heartbeat",
				showId: scope.showId,
				authorityKey: desktopAuthorityKey,
				recordedAt: Date.now(),
			} satisfies DesktopRuntimeOwnerHeartbeat);
		channel.onmessage = (event) => {
			const message = event.data as Partial<DesktopRuntimeMirrorRender>;
			if (
				message.type === "mirror-render" &&
				message.showId === scope.showId &&
				message.authorityKey === desktopAuthorityKey
			)
				frontendPerformanceDiagnostics.recordStageDesktopMirrorRender();
		};
		sendHeartbeat();
		const heartbeat = window.setInterval(sendHeartbeat, 2_000);
		return () => {
			window.clearInterval(heartbeat);
			channel.close();
			if (channelRef.current === channel) channelRef.current = null;
		};
	}, [desktopAuthorityKey, scope]);
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
	desktopAuthorityKey,
	fallbackSession,
}: PropsWithChildren<{
	scope: VisualizationRuntimeScope | null;
	desktopAuthorityKey: string;
	fallbackSession: VisualizationRuntimeSession | null;
}>) {
	const channelRef = useRef<BroadcastChannel | null>(null);
	const lastOwnerHeartbeatAt = useRef(0);
	useEffect(() => {
		if (!scope) return;
		const channel = new BroadcastChannel(DESKTOP_RUNTIME_CHANNEL);
		channelRef.current = channel;
		channel.onmessage = (event) => {
			const message = event.data as Partial<DesktopRuntimeOwnerHeartbeat>;
			if (
				message.type === "owner-heartbeat" &&
				message.showId === scope.showId &&
				message.authorityKey === desktopAuthorityKey
			)
				lastOwnerHeartbeatAt.current = Date.now();
		};
		return () => {
			channel.close();
			if (channelRef.current === channel) channelRef.current = null;
		};
	}, [desktopAuthorityKey, scope]);
	const activate = useCallback(
		(
			lane: VisualizationRuntimeLane,
			intervalMillis: number,
			consumerId?: string,
			includeDynamicStack = false,
		) => {
			if (!scope || !fallbackSession) return () => undefined;
			return fallbackSession.activate(
				lane,
				intervalMillis,
				`desktop-secondary:${consumerId ?? "consumer"}`,
				includeDynamicStack,
			);
		},
		[fallbackSession, scope],
	);
	const acknowledgeRender = useCallback(() => {
		if (!scope) return;
		channelRef.current?.postMessage({
			type: "mirror-render",
			showId: scope.showId,
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
	deliveryIntervalMillis: number,
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
	const subscribe = useCallback(
		(listener: () => void) => {
			if (deliveryIntervalMillis <= 100) return store.subscribe(listener);
			let lastNotification = 0;
			let timer: number | null = null;
			const notify = () => {
				lastNotification = Date.now();
				timer = null;
				listener();
			};
			const unsubscribe = store.subscribe(() => {
				const remaining =
					deliveryIntervalMillis - (Date.now() - lastNotification);
				if (remaining <= 0) {
					if (timer !== null) window.clearTimeout(timer);
					notify();
				} else if (timer === null) {
					timer = window.setTimeout(notify, remaining);
				}
			});
			return () => {
				unsubscribe();
				if (timer !== null) window.clearTimeout(timer);
			};
		},
		[deliveryIntervalMillis, store],
	);
	return useSyncExternalStore(
		enabled ? subscribe : NO_SUBSCRIPTION,
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
	snapshotEqual: (
		left: VisualizationSnapshot | null,
		right: VisualizationSnapshot | null,
	) => boolean,
) {
	return (
		left.status === right.status &&
		(!reconcileSnapshots || snapshotEqual(left.snapshot, right.snapshot)) &&
		left.error === right.error &&
		left.ready === right.ready
	);
}
