import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { ShowObject, ShowObjectKind } from "./contracts";
import {
	selectCueLists,
	selectPlaybackPages,
	selectPlaybacks,
	selectPortableGroups,
	selectPresets,
} from "./selectors";
import type { ShowObjectsSnapshot, ShowObjectsStore } from "./store";

export {
	selectCueLists,
	selectPlaybackPages,
	selectPlaybacks,
	selectPortableGroups,
	selectPresets,
} from "./selectors";

type GroupObject = ShowObject<"group">;
type PresetObject = ShowObject<"preset">;

export interface ShowObjectMutationState {
	pending: boolean;
	status: ShowObjectsSnapshot["status"];
	error: Error | null;
}

const ShowObjectsStoreContext = createContext<ShowObjectsStore | null>(null);

export function ShowObjectsStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: ShowObjectsStore }>) {
	return (
		<ShowObjectsStoreContext.Provider value={store}>
			{children}
		</ShowObjectsStoreContext.Provider>
	);
}

export function usePortableGroups(enabled = true): readonly GroupObject[] {
	const selector = useCallback(
		(snapshot: ShowObjectsSnapshot) =>
			enabled ? selectPortableGroups(snapshot) : EMPTY_GROUPS,
		[enabled],
	);
	return useShowObjectsSelector(selector, equalGroupCollection, enabled);
}

export function useShowObjectsStore(): ShowObjectsStore {
	const store = useContext(ShowObjectsStoreContext);
	if (!store)
		throw new Error(
			"Show-object state hooks must be used inside ShowObjectsViewProvider",
		);
	return store;
}

export function usePresets(enabled = true): readonly PresetObject[] {
	return useShowObjectsSelector(selectPresets, shallowEqualArray, enabled);
}

export function useCueLists(enabled = true): readonly ShowObject<"cue_list">[] {
	return useShowObjectsSelector(selectCueLists, shallowEqualArray, enabled);
}

export function usePlaybackDefinitions(
	enabled = true,
): readonly ShowObject<"playback">[] {
	return useShowObjectsSelector(selectPlaybacks, shallowEqualArray, enabled);
}

export function usePlaybackPages(
	enabled = true,
): readonly ShowObject<"playback_page">[] {
	return useShowObjectsSelector(
		selectPlaybackPages,
		shallowEqualArray,
		enabled,
	);
}

export function useShowObjectMutationState(
	kind: ShowObjectKind,
	objectId: string,
): ShowObjectMutationState {
	const selector = useCallback(
		(snapshot: ShowObjectsSnapshot): ShowObjectMutationState => ({
			pending: snapshot.pendingObjectKeys.has(`${kind}:${objectId}`),
			status: snapshot.status,
			error: snapshot.error,
		}),
		[kind, objectId],
	);
	return useShowObjectsSelector(selector, equalMutationState);
}

export function useShowObjectsStatus(
	enabled = true,
): Pick<ShowObjectsSnapshot, "status" | "error"> {
	return useShowObjectsSelector(selectStatus, equalStatus, enabled);
}

export function useShowObjectCollectionsReady(
	kinds: readonly ShowObjectKind[],
	enabled = true,
): boolean {
	const key = kinds.join("|");
	const selector = useCallback(
		(snapshot: ShowObjectsSnapshot) =>
			key.length > 0 &&
			key
				.split("|")
				.every((kind) => snapshot.readyCollections.has(kind as ShowObjectKind)),
		[key],
	);
	return useShowObjectsSelector(selector, Object.is, enabled);
}

function useShowObjectsSelector<T>(
	selector: (snapshot: ShowObjectsSnapshot) => T,
	equal: (left: T, right: T) => boolean = Object.is,
	enabled = true,
): T {
	const store = useShowObjectsStore();
	const cache = useRef<{
		source: ShowObjectsSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: ShowObjectsSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store.getSnapshot();
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
			equal(cache.current.selection as T, selection)
		) {
			cache.current.source = source;
			return cache.current.selection as T;
		}
		cache.current = { source, selection, hasSelection: true, selector };
		return selection;
	}, [equal, selector, store]);
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
const EMPTY_GROUPS: readonly GroupObject[] = [];

function shallowEqualArray<T>(left: readonly T[], right: readonly T[]) {
	return (
		left.length === right.length &&
		left.every((item, index) => item === right[index])
	);
}

function equalGroupCollection(
	left: readonly GroupObject[],
	right: readonly GroupObject[],
) {
	return (
		left.length === right.length &&
		left.every((group, index) => equalGroup(group, right[index]))
	);
}

function equalGroup(left: GroupObject, right: GroupObject | undefined) {
	if (left === right) return true;
	if (
		!right ||
		left.id !== right.id ||
		left.revision !== right.revision ||
		left.updated_at !== right.updated_at
	)
		return false;
	const leftBody = left.body;
	const rightBody = right.body;
	return (
		leftBody.name === rightBody.name &&
		leftBody.color === rightBody.color &&
		leftBody.icon === rightBody.icon &&
		leftBody.master === rightBody.master &&
		leftBody.playback_fader === rightBody.playback_fader &&
		leftBody.programming === rightBody.programming &&
		leftBody.derived_from === rightBody.derived_from &&
		leftBody.frozen_from === rightBody.frozen_from &&
		shallowEqualArray(leftBody.fixtures, rightBody.fixtures)
	);
}

function selectStatus(snapshot: ShowObjectsSnapshot) {
	return { status: snapshot.status, error: snapshot.error };
}

function equalStatus(
	left: Pick<ShowObjectsSnapshot, "status" | "error">,
	right: Pick<ShowObjectsSnapshot, "status" | "error">,
) {
	return left.status === right.status && left.error === right.error;
}

function equalMutationState(
	left: ShowObjectMutationState,
	right: ShowObjectMutationState,
) {
	return (
		left.pending === right.pending &&
		left.status === right.status &&
		left.error === right.error
	);
}
