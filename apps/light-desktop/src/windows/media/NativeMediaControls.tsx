import { useCallback, useEffect, useRef, useState } from "react";
import type {
	NativeMediaEffectSlot,
	NativeMediaSnapshot,
} from "../../api/client/mediaOutput";

interface NativeMediaEffectsInput {
	active: boolean;
	fixtureId: string | undefined;
	layer: number | undefined;
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
	load,
	update,
}: NativeMediaEffectsInput) {
	const loadRef = useRef(load);
	const updateRef = useRef(update);
	loadRef.current = load;
	updateRef.current = update;
	const [slots, setSlots] = useState<NativeMediaEffectSlot[]>([]);
	const [error, setError] = useState<string | null>(null);
	const changeVersion = useRef(0);

	useEffect(() => {
		changeVersion.current += 1;
		if (!active || !fixtureId || layer == null) {
			setSlots([]);
			setError(null);
			return;
		}
		let current = true;
		setError(null);
		void loadRef.current(fixtureId).then(
			(snapshot) => {
				if (current) setSlots(snapshot.effectLayers[layer] ?? []);
			},
			(reason) => {
				if (current)
					setError(reason instanceof Error ? reason.message : String(reason));
			},
		);
		return () => {
			current = false;
		};
	}, [active, fixtureId, layer]);

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

	return { slots, error, change };
}
