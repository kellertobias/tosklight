import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	NativeMediaEffectSlot,
	NativeMediaSnapshot,
	NativeMediaVisualizerChannel,
} from "../../api/client/mediaOutput";

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
	const changeVersion = useRef(0);

	useEffect(() => {
		changeVersion.current += 1;
		if (!active || !fixtureId || layer == null) {
			setSlots([]);
			setVisualizerChannels([]);
			setError(null);
			return;
		}
		let current = true;
		setError(null);
		// A new source reloads the snapshot, whose visualizer channels follow what is shown.
		void sourceKey;
		void loadRef.current(fixtureId).then(
			(snapshot) => {
				if (!current) return;
				setSlots(snapshot.effectLayers[layer] ?? []);
				setVisualizerChannels(snapshot.visualizerLayers?.[layer] ?? []);
			},
			(reason) => {
				if (current)
					setError(reason instanceof Error ? reason.message : String(reason));
			},
		);
		return () => {
			current = false;
		};
	}, [active, fixtureId, layer, sourceKey]);

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
		}),
		[slots, error, visualizerChannels],
	);

	return { slots, visualizerChannels, error, change, modelInput };
}
