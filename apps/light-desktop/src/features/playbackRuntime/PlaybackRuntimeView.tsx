import type { PlaybackAddressValue } from "../../api/playbackAddress";
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
import type { PlaybackAction } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import {
	type PlaybackDeskPageApply,
	type PlaybackRuntimeActionApply,
	type PlaybackRuntimeActions,
	PlaybackRuntimeActionWriter,
} from "./actionWriter";
import type { PlaybackIdentity, PlaybackProjection } from "./contracts";
import {
	cueListIdentity,
	groupIdentity,
	identityKey,
	playbackIdentity,
} from "./contracts";
import {
	equalGroupProjectionSelection,
	type GroupProjectionSelection,
	selectGroupProjections,
} from "./groupProjectionSelection";
import { playbackControlRuntime } from "./controlRuntime";
import {
	PlaybackRuntimeSession,
	type PlaybackRuntimeSessionOptions,
} from "./session";
import { type PlaybackRuntimeState, PlaybackRuntimeStore } from "./store";
import type { PlaybackEventTransport } from "./transport";

export interface PlaybackRuntimeViewProviderProps {
	showId: string | null;
	deskId: string | null;
	authorityKey: string;
	store: PlaybackRuntimeStore;
	transport: PlaybackEventTransport | null;
	loadSnapshot: PlaybackRuntimeSessionOptions["loadSnapshot"];
	applyAction?: PlaybackRuntimeActionApply | null;
	applyDeskPage?: PlaybackDeskPageApply | null;
	onError?: (error: Error | null) => void;
}

export interface PlaybackRuntimeAuthority {
	store: PlaybackRuntimeStore;
	activate(identity: PlaybackIdentity): () => void;
	activateDesk(): () => void;
	activateWarm(identity: PlaybackIdentity): () => void;
	activateDeskWarm(): () => void;
	refreshAuthority(): Promise<void>;
	repairAuthority(error: Error): Promise<void>;
}

const StoreContext = createContext<PlaybackRuntimeStore | null>(null);
const SessionContext = createContext<PlaybackRuntimeSession | null>(null);
const ActionsContext = createContext<PlaybackRuntimeActions | null>(null);
const AuthorityContext = createContext<PlaybackRuntimeAuthority | null>(null);
const fallbackStore = new PlaybackRuntimeStore();

export function PlaybackRuntimeViewProvider({
	children,
	showId,
	deskId,
	authorityKey,
	store,
	transport,
	loadSnapshot,
	applyAction,
	applyDeskPage,
	onError,
}: PropsWithChildren<PlaybackRuntimeViewProviderProps>) {
	const measuredLoadSnapshot = useCallback<
		PlaybackRuntimeSessionOptions["loadSnapshot"]
	>(
		async (identities) => {
			const finish =
				frontendPerformanceDiagnostics.beginSnapshotRequest("playback-runtime");
			try {
				const result = await loadSnapshot(identities);
				finish(result);
				return result;
			} catch (error) {
				finish(undefined, error);
				throw error;
			}
		},
		[loadSnapshot],
	);
	const session = useMemo(
		() =>
			showId && deskId
				? new PlaybackRuntimeSession({
						showId,
						deskId,
						authorityKey,
						store,
						transport,
						loadSnapshot: measuredLoadSnapshot,
						onError,
						resetStore: false,
					})
				: null,
		[
			authorityKey,
			deskId,
			measuredLoadSnapshot,
			onError,
			showId,
			store,
			transport,
		],
	);
	const actions = useMemo(
		() =>
			showId && deskId && applyAction
				? new PlaybackRuntimeActionWriter({
						showId,
						deskId,
						store,
						applyAction,
						applyDeskPage: applyDeskPage ?? undefined,
						repair: session
							? (error) => session.repairAuthority(error)
							: undefined,
						onActionStarted: (request) =>
							frontendPerformanceDiagnostics.recordPlaybackAction(
								crypto.randomUUID(),
								playbackActionDiagnosticLabel(request.action),
							),
						onError,
					})
				: null,
		[
			applyAction,
			applyDeskPage,
			authorityKey,
			deskId,
			onError,
			session,
			showId,
			store,
		],
	);
	const authority = useMemo<PlaybackRuntimeAuthority | null>(
		() =>
			session
				? {
						store,
						activate: (identity) => session.activate(identity),
						activateDesk: () => session.activateDesk(),
						activateWarm: (identity) => session.activate(identity, false),
						activateDeskWarm: () => session.activateDesk(false),
						refreshAuthority: () => session.refreshAuthority(),
						repairAuthority: (error) => session.repairAuthority(error),
					}
				: null,
		[session, store],
	);
	useLayoutEffect(() => {
		store.reset(showId, deskId, authorityKey);
	}, [authorityKey, deskId, showId, store]);
	useStrictModeSafeStop(session);
	useStrictModeSafeStop(actions);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				<AuthorityContext.Provider value={authority}>
					<ActionsContext.Provider value={actions}>
						{children}
					</ActionsContext.Provider>
				</AuthorityContext.Provider>
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

function playbackActionDiagnosticLabel(action: PlaybackAction) {
	return action.type === "flash"
		? action.pressed
			? "playback_flash_press"
			: "playback_flash_release"
		: `playback_${action.type}`;
}

export function usePlaybackRuntimeActions() {
	return useContext(ActionsContext);
}

export function usePlaybackRuntimeAuthority() {
	return useContext(AuthorityContext);
}

export function usePlaybackRuntimeView(
	identities: readonly PlaybackIdentity[],
) {
	const session = useContext(SessionContext);
	const key = JSON.stringify(identities.map(identityKey).sort());
	useEffect(() => {
		if (!session || !identities.length) return;
		const releases = identities.map((identity) => session.activate(identity));
		return () => {
			for (const release of releases) release();
		};
		// The canonical key owns identity equality across render-created DTOs.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [key, session]);
}

export function usePlaybackDeskView(enabled = true) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activateDesk();
	}, [enabled, session]);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => (enabled ? state.desk : null),
			[enabled],
		),
		Object.is,
		enabled,
	);
}

export function usePlaybackProjection(address: PlaybackAddressValue | null | undefined) {
	const key = address ? identityKey(playbackIdentity(address.page, address.position)) : null;
	const identities = useMemo(() => address ? [playbackIdentity(address.page, address.position)] : [], [key]);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(useCallback((state: PlaybackRuntimeState) => key ? state.projections.get(key)?.[0] : undefined, [key]), Object.is);
}

export function usePlaybackRuntime(address: PlaybackAddressValue | null | undefined) {
	return playbackControlRuntime(usePlaybackProjection(address));
}

/**
 * The retained ~10 Hz sampled telemetry row for one Playback, or null before its first sample.
 *
 * Reads the desk-lifetime store only: a freshly-mounted window renders the retained samples
 * immediately and keeps applying pushed delta ticks — no snapshot fetch and no polling.
 */
export function usePlaybackTelemetry(address: PlaybackAddressValue | null | undefined, enabled = true) {
	const key = address ? identityKey(playbackIdentity(address.page, address.position)) : null;
	return usePlaybackSelector(useCallback((state: PlaybackRuntimeState) => key ? state.telemetry.get(key) ?? null : null, [key]), Object.is, enabled);
}

/** The complete retained telemetry map, keyed by playback number. */
export function usePlaybackTelemetryMap(enabled = true) {
	return usePlaybackSelector(
		useCallback((state: PlaybackRuntimeState) => state.telemetry, []),
		Object.is,
		enabled,
	);
}

export function useCueListRuntime(cueListId: string | null | undefined) {
	const identities = useMemo(() => cueListId ? [cueListIdentity(cueListId)] : [], [cueListId]);
	usePlaybackRuntimeView(identities);
	const projection = usePlaybackSelector(useCallback((state: PlaybackRuntimeState) => cueListId ? state.projections.get(`cuelist:${cueListId}`)?.[0] : undefined, [cueListId]), Object.is);
	return playbackControlRuntime(projection);
}

/** Exact-address map shared by screens, panes, attached hardware and software controls. */
export function usePlaybackProjectionMap(addresses: readonly PlaybackAddressValue[]) {
	const canonical = [...new Map(addresses.map(address => [identityKey(playbackIdentity(address.page, address.position)), address])).entries()].sort(([left], [right]) => left.localeCompare(right));
	const key = canonical.map(([id]) => id).join(",");
	const identities = useMemo(() => canonical.map(([, address]) => playbackIdentity(address.page, address.position)), [key]);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(useCallback((state: PlaybackRuntimeState) => new Map(canonical.map(([id]) => [id, state.projections.get(id)?.[0]])), [key]), equalProjectionMap, addresses.length > 0);
}

export interface DirectCueListProjectionSelection {
	ready: boolean;
	projections: ReadonlyMap<string, PlaybackProjection | undefined>;
}

export function useDirectCueListProjectionMap(
	cueListIds: readonly string[],
	enabled = true,
): DirectCueListProjectionSelection {
	const canonical = enabled ? [...new Set(cueListIds)].sort() : [];
	const key = JSON.stringify(canonical);
	const identities = useMemo(
		() =>
			canonical.map(
				(cue_list_id): PlaybackIdentity => ({
					kind: "direct_cue_list",
					cue_list_id,
				}),
			),
		// The canonical Cuelist key owns array equality.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[key],
	);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => directCueListSelection(state, canonical),
			// The same canonical key denotes the selected Cuelist set.
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[key],
		),
		equalDirectCueListSelection,
		enabled && cueListIds.length > 0,
	);
}

export function useGroupProjectionMap(
	groupIds: readonly string[],
	enabled = true,
): GroupProjectionSelection {
	const canonical = enabled ? [...new Set(groupIds)].sort() : [];
	const key = JSON.stringify(canonical);
	const identities = useMemo(
		() => canonical.map(groupIdentity),
		// The canonical Group key owns array equality.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[key],
	);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => selectGroupProjections(state, canonical),
			// The same canonical key denotes the selected Group set.
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[key],
		),
		equalGroupProjectionSelection,
		enabled && groupIds.length > 0,
	);
}

export function usePlaybackRuntimeStatus(enabled = true) {
	return usePlaybackSelector(selectStatus, equalStatus, enabled);
}

function usePlaybackSelector<T>(
	selector: (state: PlaybackRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
	enabled = true,
) {
	const store = useContext(StoreContext) ?? fallbackStore;
	const cache = useRef<{
		state: PlaybackRuntimeState | null;
		selector: ((state: PlaybackRuntimeState) => T) | null;
		value?: T;
	}>({ state: null, selector: null });
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.selector === selector && cache.current.state === state)
			return cache.current.value as T;
		const value = selector(state);
		if (
			cache.current.selector === selector &&
			cache.current.state &&
			equal(cache.current.value as T, value)
		) {
			cache.current.state = state;
			return cache.current.value as T;
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

const NO_SUBSCRIPTION = () => () => undefined;

function equalProjectionMap(
	left: ReadonlyMap<string | number, PlaybackProjection | undefined>,
	right: ReadonlyMap<string | number, PlaybackProjection | undefined>,
) {
	return (
		left.size === right.size &&
		[...left].every(([key, value]) => right.get(key) === value)
	);
}

function directCueListSelection(
	state: PlaybackRuntimeState,
	cueListIds: readonly string[],
): DirectCueListProjectionSelection {
	let ready = true;
	const projections = new Map<string, PlaybackProjection | undefined>();
	for (const cueListId of cueListIds) {
		const candidates = state.projections.get(`direct-cuelist:${cueListId}`);
		const requested = candidates?.filter(
			(projection) =>
				projection.identity.kind === "direct_cue_list" &&
				projection.identity.cue_list_id === cueListId,
		);
		// The exact direct source stays separate from Cuelist aggregate aliases.
		if (!candidates?.length) ready = false;
		projections.set(
			cueListId,
			requested?.[0],
		);
	}
	return { ready, projections };
}

function equalDirectCueListSelection(
	left: DirectCueListProjectionSelection,
	right: DirectCueListProjectionSelection,
) {
	return (
		left.ready === right.ready &&
		left.projections.size === right.projections.size &&
		[...left.projections].every(
			([key, value]) => right.projections.get(key) === value,
		)
	);
}

function selectStatus(state: PlaybackRuntimeState) {
	return { status: state.status, error: state.error };
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return left.status === right.status && left.error === right.error;
}
