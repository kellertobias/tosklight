import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import { useProgrammerCaptureModeAuthority } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerPreloadValuesAuthority } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesAuthority } from "../programmerValues/ProgrammerValuesView";
import {
	useProgrammingSelectionAuthority,
	useProgrammingSelectionView,
} from "../programmingInteraction/ProgrammingInteractionView";
import { useStrictModeSafeStop } from "../shared/useStrictModeSafeStop";
import type { ShowObject } from "../showObjects/contracts";
import { useShowObjectView } from "../showObjects/ShowObjectsView";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	PresetRecallActions,
	PresetRecallScope,
	PresetRecallTransport,
} from "./contracts";
import { PresetRecallWriter } from "./writer";

interface PresetRecallProviderProps {
	showId: string | null;
	sessionId: string | null;
	deskId: string | null;
	authorityKey: string;
	showStore: ShowObjectsStore;
	transport: PresetRecallTransport | null;
	loadPreset(
		showId: string,
		objectId: string,
	): Promise<{ object: ShowObject<"preset"> | null; showRevision: number }>;
	onError?: (error: Error | null) => void;
}

const PresetRecallContext = createContext<PresetRecallActions | null>(null);

/** Action-only composition; reads and subscriptions remain view-owned. */
export function PresetRecallProvider({
	children,
	showId,
	sessionId,
	deskId,
	authorityKey,
	showStore,
	transport,
	loadPreset,
	onError,
}: PropsWithChildren<PresetRecallProviderProps>) {
	const values = useProgrammerValuesAuthority();
	const preloadValues = useProgrammerPreloadValuesAuthority();
	const captureMode = useProgrammerCaptureModeAuthority();
	const selection = useProgrammingSelectionAuthority();
	const scope = useMemo<PresetRecallScope | null>(
		() => (showId && sessionId && deskId ? { showId, sessionId, deskId } : null),
		[deskId, showId, sessionId],
	);
	const writer = useMemo(
		() =>
			scope && transport && values && preloadValues && captureMode && selection
				? new PresetRecallWriter({
						scope,
						showStore,
						valuesStore: values.store,
						preloadValuesStore: preloadValues.store,
						captureModeStore: captureMode.store,
						programmingStore: selection.store,
						transport,
						loadPreset,
						repairValues: values.repairAuthority,
						repairPreloadValues: preloadValues.repairAuthority,
						repairCaptureMode: captureMode.repairAuthority,
						repairSelection: selection.repairAuthority,
						onError,
					})
				: null,
		[
			authorityKey,
			captureMode,
			loadPreset,
			onError,
			preloadValues,
			scope,
			selection,
			showStore,
			transport,
			values,
		],
	);
	useStrictModeSafeStop(writer);
	return (
		<PresetRecallContext.Provider value={writer}>
			{children}
		</PresetRecallContext.Provider>
	);
}

/** Activates only the four exact authorities required by a visible Presets view. */
export function usePresetRecall(enabled = true) {
	const actions = useContext(PresetRecallContext);
	const values = useProgrammerValuesAuthority();
	const preloadValues = useProgrammerPreloadValuesAuthority();
	const captureMode = useProgrammerCaptureModeAuthority();
	useShowObjectView("preset", enabled);
	const selection = useProgrammingSelectionView(enabled);
	useEffect(() => {
		if (!enabled) return;
		const releaseValues = values?.activate();
		const releasePreloadValues = preloadValues?.activate();
		const releaseCaptureMode = captureMode?.activate();
		return () => {
			releaseValues?.();
			releasePreloadValues?.();
			releaseCaptureMode?.();
		};
	}, [captureMode, enabled, preloadValues, values]);
	return { actions: enabled ? actions : null, selection };
}
