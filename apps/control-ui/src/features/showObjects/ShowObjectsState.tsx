import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { ShowObject, ShowObjectKind } from "./contracts";
import { selectPortableGroups, selectPresets } from "./selectors";
import { ShowObjectsStore, type ShowObjectsSnapshot } from "./store";

export { selectPortableGroups, selectPresets } from "./selectors";

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

export function usePortableGroups(): readonly GroupObject[] {
	return useShowObjectsSelector(selectPortableGroups, equalGroupCollection);
}

export function useShowObjectsStore(): ShowObjectsStore {
	const store = useContext(ShowObjectsStoreContext);
	if (!store)
		throw new Error(
			"Show-object state hooks must be used inside ShowObjectsViewProvider",
		);
	return store;
}

export function usePresets(): readonly PresetObject[] {
	return useShowObjectsSelector(selectPresets, shallowEqualArray);
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

export function useShowObjectsStatus(): Pick<
	ShowObjectsSnapshot,
	"status" | "error"
> {
	return useShowObjectsSelector(selectStatus, equalStatus);
}

function useShowObjectsSelector<T>(
	selector: (snapshot: ShowObjectsSnapshot) => T,
	equal: (left: T, right: T) => boolean = Object.is,
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
	return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}

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
