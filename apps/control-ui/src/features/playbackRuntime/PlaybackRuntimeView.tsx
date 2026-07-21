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
import { legacyPlaybackRuntime } from "./legacy";
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
	const session = useMemo(
		() =>
			showId && deskId
				? new PlaybackRuntimeSession({
						showId,
						deskId,
						authorityKey,
						store,
						transport,
						loadSnapshot,
						onError,
						resetStore: false,
					})
				: null,
		[authorityKey, deskId, loadSnapshot, onError, showId, store, transport],
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
						onError,
					})
				: null,
		[applyAction, applyDeskPage, authorityKey, deskId, onError, showId, store],
	);
	const authority = useMemo<PlaybackRuntimeAuthority | null>(
		() =>
			session
				? {
						store,
						activate: (identity) => session.activate(identity),
						activateDesk: () => session.activateDesk(),
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

export function usePlaybackProjection(
	playbackNumber: number | null | undefined,
) {
	const identity = useMemo(
		() => (playbackNumber == null ? [] : [playbackIdentity(playbackNumber)]),
		[playbackNumber],
	);
	usePlaybackRuntimeView(identity);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) =>
				playbackNumber == null
					? undefined
					: state.projections
							.get(`playback:${playbackNumber}`)
							?.find((item) => item.playback_number === playbackNumber),
			[playbackNumber],
		),
		Object.is,
	);
}

export function usePlaybackRuntime(playbackNumber: number | null | undefined) {
	return legacyPlaybackRuntime(usePlaybackProjection(playbackNumber));
}

export function useCueListRuntime(
	cueListId: string | null | undefined,
	playbackNumber?: number | null,
) {
	const identities = useMemo(
		() => (cueListId ? [cueListIdentity(cueListId)] : []),
		[cueListId],
	);
	usePlaybackRuntimeView(identities);
	const projection = usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => {
				const candidates = cueListId
					? state.projections.get(`cuelist:${cueListId}`)
					: undefined;
				return (
					candidates?.find(
						(item) => item.playback_number === (playbackNumber ?? null),
					) ?? candidates?.[0]
				);
			},
			[cueListId, playbackNumber],
		),
		Object.is,
	);
	return legacyPlaybackRuntime(projection);
}

export function usePlaybackProjectionMap(playbackNumbers: readonly number[]) {
	const canonical = [...new Set(playbackNumbers)].sort((a, b) => a - b);
	const key = canonical.join(",");
	const identities = useMemo(
		() => canonical.map(playbackIdentity),
		// The canonical numeric key owns array equality.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[key],
	);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) =>
				new Map(
					canonical.map((number) => [
						number,
						state.projections
							.get(`playback:${number}`)
							?.find((item) => item.playback_number === number),
					]),
				),
			// The same canonical key denotes the selected number set.
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[key],
		),
		equalProjectionMap,
		playbackNumbers.length > 0,
	);
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
		() => canonical.map(cueListIdentity),
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
	left: ReadonlyMap<number, PlaybackProjection | undefined>,
	right: ReadonlyMap<number, PlaybackProjection | undefined>,
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
		const candidates = state.projections.get(`cuelist:${cueListId}`);
		const requested = candidates?.filter(
			(projection) =>
				projection.requested.kind === "cue_list" &&
				projection.requested.cue_list_id === cueListId,
		);
		// A mapped projection can share the same Cuelist key and replace the
		// request-shaped copy in the normalized store. Its presence still proves
		// that the exact Cuelist snapshot completed; only direct-row selection
		// remains restricted to an explicitly Cuelist-requested projection.
		if (!candidates?.length) ready = false;
		projections.set(
			cueListId,
			requested?.find((projection) => projection.playback_number === null),
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
