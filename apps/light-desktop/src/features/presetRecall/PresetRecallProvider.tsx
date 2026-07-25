import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import { useProgrammerCaptureModeAuthority } from "../programmerCaptureMode/ProgrammerCaptureModeView";
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
	userId: string | null;
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
	userId,
	deskId,
	authorityKey,
	showStore,
	transport,
	loadPreset,
	onError,
}: PropsWithChildren<PresetRecallProviderProps>) {
	const values = useProgrammerValuesAuthority();
	const captureMode = useProgrammerCaptureModeAuthority();
	const selection = useProgrammingSelectionAuthority();
	const scope = useMemo<PresetRecallScope | null>(
		() => (showId && userId && deskId ? { showId, userId, deskId } : null),
		[deskId, showId, userId],
	);
	const writer = useMemo(
		() =>
			scope && transport && values && captureMode && selection
				? new PresetRecallWriter({
						scope,
						showStore,
						valuesStore: values.store,
						captureModeStore: captureMode.store,
						programmingStore: selection.store,
						transport,
						loadPreset,
						repairValues: values.repairAuthority,
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
			scope,
			selection,
			showStore,
			transport,
			values,
		],
	);
	const actions = useMemo<PresetRecallActions>(
		() =>
			writer ?? {
				recall: async () => {
					onError?.(new Error("Preset recall is unavailable"));
					return null;
				},
			},
		[onError, writer],
	);
	useStrictModeSafeStop(writer);
	return (
		<PresetRecallContext.Provider value={actions}>
			{children}
		</PresetRecallContext.Provider>
	);
}

/** Activates only the four exact authorities required by a visible Presets view. */
export function usePresetRecall(enabled = true) {
	const actions = useContext(PresetRecallContext);
	const values = useProgrammerValuesAuthority();
	const captureMode = useProgrammerCaptureModeAuthority();
	useShowObjectView("preset", enabled);
	const selection = useProgrammingSelectionView(enabled);
	useEffect(() => {
		if (!enabled) return;
		const releaseValues = values?.activate();
		const releaseCaptureMode = captureMode?.activate();
		return () => {
			releaseValues?.();
			releaseCaptureMode?.();
		};
	}, [captureMode, enabled, values]);
	return { actions: enabled ? actions : null, selection };
}
