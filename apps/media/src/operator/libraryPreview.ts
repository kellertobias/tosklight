/**
 * The Library's live preview: one media slot shown on its own, on the output as it really is.
 *
 * Preview shows the chosen slot on Layer 1 — full up, looping — and puts every other layer out,
 * with the master full up, so the picture on the output is that slot alone through the output's
 * current visualizer and pixel-map settings. It is a plain set of layer writes rather than a
 * separate renderer, which is why a settings change shows at once. Turning preview off puts every
 * layer and the master back as they were when it was turned on, so nothing of the preview lingers
 * and no second layer is left playing.
 */
import { useCallback, useMemo, useRef, useState } from "react";
import type { OutputView, UpdateLayer, UpdateMaster } from "../shared/api/generated/media-wire";
import type { LayerControl } from "../shared/api/layerControl";

/** The layer the preview plays on, and the play mode it loops in. */
export const PREVIEW_LAYER = 0;
const LOOP = 0;

type Slot = { folder: number; file: number };

/**
 * What the preview shows on its layer: a media, text or visualizer slot, and over it an Effects
 * library slot or a 3D model. An effect or a model with no slot of its own is shown over the slot
 * the preview last played, so it is judged on real content.
 */
export interface PreviewTarget {
	slot?: Slot | null;
	/** An Effects library slot, `1..=255`, put on the layer's first effect bank at full strength. */
	effect?: number;
	/** A 3D model slot, `1..=255`, the layer is mapped onto. */
	model?: number;
}

export interface PreviewWrites {
	layers: Array<{ index: number; change: UpdateLayer }>;
	master: UpdateMaster | null;
}

type Layer = OutputView["layers"][number];

/** The preview layer's effect banks and model set for a target: only what it asks for, every other bank off. */
function facetWrites(layer: Layer, target: PreviewTarget): UpdateLayer[] {
	const writes: UpdateLayer[] = layer.effectBanks.flatMap((bank) => {
		const select = bank.index === 0 ? (target.effect ?? 0) : 0;
		const strength = select ? 1 : bank.strength;
		return bank.select !== select || bank.strength !== strength
			? [{ effectBank: bank.index, effectSelect: select, effectStrength: strength }]
			: [];
	});
	const model = target.model ?? 0;
	if (layer.model !== model) writes.push({ model });
	return writes;
}

/** Every other layer out and the master full up; with a target, the preview layer showing it alone. */
export function previewWrites(output: OutputView, target: PreviewTarget | null): PreviewWrites {
	const layers = output.layers.flatMap((layer) => {
		if (layer.index === PREVIEW_LAYER && target) {
			const content: UpdateLayer = { dimmer: 1, playModeDmx: LOOP };
			if (target.slot) Object.assign(content, { folder: target.slot.folder, file: target.slot.file });
			return [content, ...facetWrites(layer, target)].map((change) => ({ index: layer.index, change }));
		}
		return layer.dimmer > 0 ? [{ index: layer.index, change: { dimmer: 0 } }] : [];
	});
	return { layers, master: output.master.dimmer < 1 ? { dimmer: 1 } : null };
}

/**
 * What puts `current` back to how `before` was: each layer's slot, dimmer and mode, its effect
 * banks and model, and the master.
 */
export function restoreWrites(before: OutputView, current: OutputView): PreviewWrites {
	const layers = before.layers.flatMap((was) => {
		const now = current.layers.find((layer) => layer.index === was.index);
		if (!now) return [];
		const change: UpdateLayer = {};
		if (now.address.folder !== was.address.folder || now.address.file !== was.address.file) {
			change.folder = was.address.folder;
			change.file = was.address.file;
		}
		if (now.dimmer !== was.dimmer) change.dimmer = was.dimmer;
		if (now.playModeDmx !== was.playModeDmx) change.playModeDmx = was.playModeDmx;
		if (now.model !== was.model) change.model = was.model;
		const banks = was.effectBanks.flatMap((bank) => {
			const shown = now.effectBanks.find((each) => each.index === bank.index);
			return shown && (shown.select !== bank.select || shown.strength !== bank.strength)
				? [{ effectBank: bank.index, effectSelect: bank.select, effectStrength: bank.strength }]
				: [];
		});
		return [...(Object.keys(change).length ? [change] : []), ...banks].map((each) => ({
			index: was.index,
			change: each,
		}));
	});
	return {
		layers,
		master: current.master.dimmer !== before.master.dimmer ? { dimmer: before.master.dimmer } : null,
	};
}

/** Writes one layer's changes in order, since each effect bank is its own write; layers side by side. */
async function write(control: LayerControl, output: OutputView, writes: PreviewWrites) {
	const byLayer = new Map<number, UpdateLayer[]>();
	for (const { index, change } of writes.layers) byLayer.set(index, [...(byLayer.get(index) ?? []), change]);
	await Promise.all([
		...[...byLayer].map(async ([index, changes]) => {
			for (const change of changes) await control.update(output, index, change);
		}),
		...(writes.master ? [control.updateMaster(output, writes.master)] : []),
	]);
}

export interface LibraryPreview {
	/** The output preview is on, or null while it is off. */
	outputId: string | null;
	setEnabled: (enabled: boolean) => Promise<void>;
	/** Shows a target on the preview layer; does nothing while preview is off. */
	show: (target: PreviewTarget) => Promise<void>;
}

/**
 * Preview's state: what the output looked like when it was turned on, so turning it off can put
 * that back. It lives only in memory — a reload or a restart forgets it, and nothing is saved.
 */
export function useLibraryPreview(
	outputs: OutputView[] | undefined,
	control: LayerControl,
	selectedOutputId: string,
): LibraryPreview {
	const [before, setBefore] = useState<OutputView | null>(null);
	const lastSlot = useRef<Slot | null>(null);
	const enabling = useRef(false);
	const latest = useRef(outputs);
	latest.current = outputs;
	const current = (id: string) => latest.current?.find((output) => output.id === id);

	// biome-ignore lint/correctness/useExhaustiveDependencies: `current` reads the latest outputs from a ref.
	const setEnabled = useCallback(
		async (enabled: boolean) => {
			if (enabled) {
				const output = current(selectedOutputId);
				if (!output || before || enabling.current) return;
				enabling.current = true;
				try {
					if (!output.playbackTakeover) await control.setTakeover(output, true);
					await write(control, output, previewWrites(output, null));
				} finally {
					enabling.current = false;
				}
				// On only once every other layer is out, so what an editor shows next is not put out too.
				setBefore(output);
				return;
			}
			if (!before) return;
			setBefore(null);
			const output = current(before.id);
			if (!output) return;
			await write(control, output, restoreWrites(before, output));
			if (!before.playbackTakeover) await control.setTakeover(output, false);
		},
		// `current` reads the ref, so the latest outputs never go stale here.
		[before, control, selectedOutputId],
	);

	// biome-ignore lint/correctness/useExhaustiveDependencies: `current` reads the latest outputs from a ref.
	const show = useCallback(
		async (target: PreviewTarget) => {
			const output = before && current(before.id);
			if (!output) return;
			// An effect or a model goes over the slot last shown, or over whatever the layer holds.
			const slot = target.slot ?? (target.effect || target.model ? lastSlot.current : null);
			if (target.slot) lastSlot.current = target.slot;
			await write(control, output, previewWrites(output, { ...target, slot }));
		},
		[before, control],
	);

	return useMemo(
		() => ({ outputId: before?.id ?? null, setEnabled, show }),
		[before, setEnabled, show],
	);
}
