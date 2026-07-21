import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { DeskConfiguration } from "../../api/types";
import {
	selectDeskConfiguration,
	selectFileManagerSystemPickerFallback,
	selectMatterEnabled,
	selectPatchPreviewHighlightDmx,
	selectProgrammerFadeMillis,
	selectSequenceMasterFadeMillis,
	selectSpeedGroupsBpm,
} from "./selectors";
import {
	type ConfigurationSnapshot,
	ConfigurationStore,
	EMPTY_CONFIGURATION_SNAPSHOT,
} from "./store";

const ConfigurationStoreContext = createContext<ConfigurationStore | null>(null);

export function ConfigurationStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: ConfigurationStore }>) {
	return (
		<ConfigurationStoreContext.Provider value={store}>
			{children}
		</ConfigurationStoreContext.Provider>
	);
}

/** Programmer fade in milliseconds, or null while the desk configuration is unknown. */
export function useProgrammerFadeMillis(): number | null {
	return useConfigurationSelector(selectProgrammerFadeMillis, Object.is);
}

/** Sequence master fade in milliseconds, or null while the desk configuration is unknown. */
export function useSequenceMasterFadeMillis(): number | null {
	return useConfigurationSelector(selectSequenceMasterFadeMillis, Object.is);
}

export function useSpeedGroupsBpm():
	| DeskConfiguration["speed_groups_bpm"]
	| null {
	return useConfigurationSelector(selectSpeedGroupsBpm, equalSpeedGroups);
}

export function usePatchPreviewHighlightDmx(): boolean {
	return useConfigurationSelector(selectPatchPreviewHighlightDmx, Object.is);
}

export function useMatterEnabled(): boolean {
	return useConfigurationSelector(selectMatterEnabled, Object.is);
}

export function useFileManagerSystemPickerFallback(): boolean {
	return useConfigurationSelector(
		selectFileManagerSystemPickerFallback,
		Object.is,
	);
}

/**
 * The whole desk configuration, for the settings surfaces that genuinely edit all of it.
 *
 * Prefer a scalar hook: a whole-configuration reader rerenders on every configuration change.
 */
export function useDeskConfiguration(): DeskConfiguration | null {
	return useConfigurationSelector(selectDeskConfiguration, Object.is);
}

/**
 * Equality-cached configuration projection.
 *
 * A reader outside a mounted configuration boundary observes the inert empty snapshot rather than
 * falling back to broad server state.
 */
function useConfigurationSelector<T>(
	selector: (snapshot: ConfigurationSnapshot) => T,
	equal: (left: T, right: T) => boolean,
): T {
	const store = useContext(ConfigurationStoreContext);
	const cache = useRef<{
		source: ConfigurationSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: ConfigurationSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store
			? store.getSnapshot()
			: EMPTY_CONFIGURATION_SNAPSHOT;
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
		store ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;

function equalSpeedGroups(
	left: DeskConfiguration["speed_groups_bpm"] | null,
	right: DeskConfiguration["speed_groups_bpm"] | null,
) {
	if (left === right) return true;
	if (!left || !right) return false;
	return left.every((value, index) => value === right[index]);
}
