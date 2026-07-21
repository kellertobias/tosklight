import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { StagePosition3d } from "../server/contracts";
import type { StoredStageLayout } from "../server/contracts";
import {
	EMPTY_STAGE_LAYOUT_SNAPSHOT,
	type StageLayoutSnapshot,
	StageLayoutStore,
} from "./store";

type Positions = StoredStageLayout["positions"];
type Positions3d = Record<string, StagePosition3d>;

const EMPTY_POSITIONS: Positions = {};
const EMPTY_POSITIONS_3D: Positions3d = {};

const StageLayoutStoreContext = createContext<StageLayoutStore | null>(null);

export function StageLayoutStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: StageLayoutStore }>) {
	return (
		<StageLayoutStoreContext.Provider value={store}>
			{children}
		</StageLayoutStoreContext.Provider>
	);
}

export function useStagePositions(): Positions {
	return useStageLayoutSelector(selectPositions);
}

export function useStagePositions3d(): Positions3d {
	return useStageLayoutSelector(selectPositions3d);
}

/** The stored revision a stage-layout write must be made against. */
export function useStageLayoutRevision(): number {
	return useStageLayoutSelector(selectRevision);
}

export function useStageLayoutStoreOrNull(): StageLayoutStore | null {
	return useContext(StageLayoutStoreContext);
}

function selectPositions(snapshot: StageLayoutSnapshot): Positions {
	return snapshot.layout?.body.positions ?? EMPTY_POSITIONS;
}

function selectPositions3d(snapshot: StageLayoutSnapshot): Positions3d {
	return snapshot.layout?.body.positions3d ?? EMPTY_POSITIONS_3D;
}

function selectRevision(snapshot: StageLayoutSnapshot): number {
	return snapshot.layout?.revision ?? 0;
}

/**
 * Equality-cached stage-layout projection.
 *
 * A reader outside a mounted stage-layout boundary observes the inert empty snapshot rather than
 * falling back to broad server state.
 */
function useStageLayoutSelector<T>(
	selector: (snapshot: StageLayoutSnapshot) => T,
): T {
	const store = useContext(StageLayoutStoreContext);
	const cache = useRef<{
		source: StageLayoutSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: StageLayoutSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store
			? store.getSnapshot()
			: EMPTY_STAGE_LAYOUT_SNAPSHOT;
		if (
			cache.current.selector === selector &&
			cache.current.source === source &&
			cache.current.hasSelection
		)
			return cache.current.selection as T;
		const selection = selector(source);
		if (
			cache.current.selector === selector &&
			cache.current.hasSelection &&
			Object.is(cache.current.selection as T, selection)
		) {
			cache.current.source = source;
			return cache.current.selection as T;
		}
		cache.current = { source, selection, hasSelection: true, selector };
		return selection;
	}, [selector, store]);
	return useSyncExternalStore(
		store ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
