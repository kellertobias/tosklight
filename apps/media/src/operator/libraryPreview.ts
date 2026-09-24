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

export interface PreviewWrites {
	layers: Array<{ index: number; change: UpdateLayer }>;
	master: UpdateMaster | null;
}

/** Every other layer out and the master full up; with a slot, the preview layer playing it. */
export function previewWrites(
	output: OutputView,
	slot: { folder: number; file: number } | null,
): PreviewWrites {
	const layers = output.layers.flatMap((layer) => {
		if (layer.index === PREVIEW_LAYER && slot)
			return [
				{
					index: layer.index,
					change: { folder: slot.folder, file: slot.file, dimmer: 1, playModeDmx: LOOP },
				},
			];
		return layer.dimmer > 0 ? [{ index: layer.index, change: { dimmer: 0 } }] : [];
	});
	return { layers, master: output.master.dimmer < 1 ? { dimmer: 1 } : null };
}

/** What puts `current` back to how `before` was: each layer's slot, dimmer and mode, and the master. */
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
		return Object.keys(change).length ? [{ index: was.index, change }] : [];
	});
	return {
		layers,
		master: current.master.dimmer !== before.master.dimmer ? { dimmer: before.master.dimmer } : null,
	};
}

async function write(control: LayerControl, output: OutputView, writes: PreviewWrites) {
	await Promise.all([
		...writes.layers.map(({ index, change }) => control.update(output, index, change)),
		...(writes.master ? [control.updateMaster(output, writes.master)] : []),
	]);
}

export interface LibraryPreview {
	/** The output preview is on, or null while it is off. */
	outputId: string | null;
	setEnabled: (enabled: boolean) => Promise<void>;
	/** Shows a slot on the preview layer; does nothing while preview is off. */
	show: (slot: { folder: number; file: number }) => Promise<void>;
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
	const latest = useRef(outputs);
	latest.current = outputs;
	const current = (id: string) => latest.current?.find((output) => output.id === id);

	// biome-ignore lint/correctness/useExhaustiveDependencies: `current` reads the latest outputs from a ref.
	const setEnabled = useCallback(
		async (enabled: boolean) => {
			if (enabled) {
				const output = current(selectedOutputId);
				if (!output || before) return;
				setBefore(output);
				if (!output.playbackTakeover) await control.setTakeover(output, true);
				await write(control, output, previewWrites(output, null));
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
		async (slot: { folder: number; file: number }) => {
			const output = before && current(before.id);
			if (output) await write(control, output, previewWrites(output, slot));
		},
		[before, control],
	);

	return useMemo(
		() => ({ outputId: before?.id ?? null, setEnabled, show }),
		[before, setEnabled, show],
	);
}
