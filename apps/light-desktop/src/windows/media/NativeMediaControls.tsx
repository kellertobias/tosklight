import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	NativeMediaEffectSlot,
	NativeMediaSnapshot,
	NativeMediaVisualizerChannel,
} from "../../api/client/mediaOutput";
import type { MediaPointFrameRate } from "./mediaPaneModel";

const NO_MEDIA_API: MediaPointFrameRate = {
	kind: "unknown",
	detail:
		"the desk reads it only from a ToskLight Media Server patched with its Media API in Show Patch > Media Servers.",
};

interface NativeMediaEffectsInput {
	active: boolean;
	fixtureId: string | undefined;
	layer: number | undefined;
	/**
	 * What the layer shows. The visualizer channels follow the shown visualizer, so a change reloads
	 * them.
	 */
	sourceKey?: string;
	load(fixtureId: string): Promise<NativeMediaSnapshot>;
	update(
		fixtureId: string,
		layer: number,
		controlId: string,
		value: string | number | boolean,
	): Promise<NativeMediaEffectSlot[]>;
}

export function useNativeMediaEffects({
	active,
	fixtureId,
	layer,
	sourceKey,
	load,
	update,
}: NativeMediaEffectsInput) {
	const loadRef = useRef(load);
	const updateRef = useRef(update);
	loadRef.current = load;
	updateRef.current = update;
	const [slots, setSlots] = useState<NativeMediaEffectSlot[]>([]);
	const [visualizerChannels, setVisualizerChannels] = useState<
		NativeMediaVisualizerChannel[]
	>([]);
	const [error, setError] = useState<string | null>(null);
	const [pointFrameRate, setPointFrameRate] =
		useState<MediaPointFrameRate>(NO_MEDIA_API);
	const [reloads, setReloads] = useState(0);
	const changeVersion = useRef(0);

	useEffect(() => {
		changeVersion.current += 1;
		if (!active || !fixtureId || layer == null) {
			setSlots([]);
			setVisualizerChannels([]);
			setError(null);
			setPointFrameRate(NO_MEDIA_API);
			return;
		}
		let current = true;
		setError(null);
		setPointFrameRate((previous) =>
			previous.kind === "known" ? previous : { kind: "loading" },
		);
		// A new source reloads the snapshot, whose visualizer channels follow what is shown; a
		// retry asks for the frame rate again.
		void sourceKey;
		void reloads;
		void loadRef.current(fixtureId).then(
			(snapshot) => {
				if (!current) return;
				setSlots(snapshot.effectLayers[layer] ?? []);
				setVisualizerChannels(snapshot.visualizerLayers?.[layer] ?? []);
				setPointFrameRate(
					snapshot.frameRate
						? { kind: "known", framesPerSecond: snapshot.frameRate }
						: {
								kind: "unknown",
								retryable: true,
								detail:
									"this Media Server does not report the rate its In and Out points count in. Update the ToskLight Media Server, then check again.",
							},
				);
			},
			(reason) => {
				if (!current) return;
				const message =
					reason instanceof Error ? reason.message : String(reason);
				setError(message);
				setPointFrameRate({
					kind: "unknown",
					retryable: true,
					detail: `the Media Server could not be read (${message}). Check that it is running and reachable, then check again.`,
				});
			},
		);
		return () => {
			current = false;
		};
	}, [active, fixtureId, layer, sourceKey, reloads]);

	const retryPointFrameRate = useCallback(
		() => setReloads((count) => count + 1),
		[],
	);

	const change = useCallback(
		(controlId: string, value: string | number | boolean) => {
			if (!fixtureId || layer == null) return;
			const version = ++changeVersion.current;
			void updateRef.current(fixtureId, layer, controlId, value).then(
				(nextSlots) => {
					if (version !== changeVersion.current) return;
					setSlots(nextSlots);
					setError(null);
				},
				(reason) => {
					if (version !== changeVersion.current) return;
					setError(reason instanceof Error ? reason.message : String(reason));
				},
			);
		},
		[fixtureId, layer],
	);

	// What the Media pane model reads, stable while none of it changes.
	const modelInput = useMemo(
		() => ({
			nativeEffects: slots,
			nativeEffectsError: error,
			visualizerChannels,
			pointFrameRate,
			retryPointFrameRate,
		}),
		[slots, error, visualizerChannels, pointFrameRate, retryPointFrameRate],
	);

	return {
		slots,
		visualizerChannels,
		error,
		change,
		modelInput,
		pointFrameRate,
		retryPointFrameRate,
	};
}
