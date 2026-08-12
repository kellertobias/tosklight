// Layer control: the writes, their optimistic effect, and their rollback.
//
// This lives in the shared API layer rather than inside the layers feature because two features
// write to layers — the layer page and the visualizer picker — and a feature must never reach
// into another feature's internals to do it.
//
// An optimistic change here is not a guess about what the server will do — the projection the
// server returns replaces it either way. It exists so a fader feels attached to the value. The
// rollback matters more than the optimism: when a desk takes an output, the write is refused and
// the control must snap back to what the desk says, not to what the operator dragged.

import { useCallback, useState } from "react";
import { ApiFailure, api } from "./client";
import type {
	OutputView,
	UpdateLayer,
	UpdateMaster,
} from "./generated/media-wire";
import { KEYS } from "./queries";
import { type Resource, useResource, writeResource } from "./resource";

/** Outputs change continuously while a desk is driving them. */
export const OUTPUT_POLL_MS = 1_000;

export function useOutputsForControl(): Resource<OutputView[]> {
	return useResource<OutputView[]>(KEYS.outputs, api.outputs, {
		pollMs: OUTPUT_POLL_MS,
	});
}

export interface LayerControl {
	/** The last write that was refused, still on screen so the operator learns why. */
	refusal: ApiFailure | undefined;
	dismissRefusal: () => void;
	update: (
		output: OutputView,
		layer: number,
		change: UpdateLayer,
	) => Promise<void>;
	updateMaster: (output: OutputView, change: UpdateMaster) => Promise<void>;
	reset: (output: OutputView, layer: number) => Promise<void>;
	busy: boolean;
}

export function useLayerControl(
	outputs: OutputView[] | undefined,
): LayerControl {
	const [refusal, setRefusal] = useState<ApiFailure | undefined>(undefined);
	const [busy, setBusy] = useState(false);

	const update = useCallback(
		async (output: OutputView, layer: number, change: UpdateLayer) => {
			const previous = outputs;
			if (previous)
				writeResource(
					KEYS.outputs,
					applyLocally(previous, output.id, layer, change),
				);
			setBusy(true);
			try {
				const updated = await api.updateLayer(output.id, layer, change);
				writeResource(KEYS.outputs, replaceOutput(previous ?? [], updated));
				setRefusal(undefined);
			} catch (error) {
				// Roll back to exactly the projection we had, then let the poll reconcile.
				if (previous) writeResource(KEYS.outputs, previous);
				setRefusal(asFailure(error));
			} finally {
				setBusy(false);
			}
		},
		[outputs],
	);

	const reset = useCallback(async (output: OutputView, layer: number) => {
		setBusy(true);
		try {
			await api.resetLayer(output.id, layer);
			setRefusal(undefined);
		} catch (error) {
			setRefusal(asFailure(error));
		} finally {
			setBusy(false);
		}
	}, []);
	const updateMaster = useCallback(
		async (output: OutputView, change: UpdateMaster) => {
			setBusy(true);
			try {
				const updated = await api.updateMaster(output.id, change);
				writeResource(KEYS.outputs, replaceOutput(outputs ?? [], updated));
				setRefusal(undefined);
			} catch (error) {
				setRefusal(asFailure(error));
			} finally {
				setBusy(false);
			}
		},
		[outputs],
	);

	return {
		refusal,
		dismissRefusal: useCallback(() => setRefusal(undefined), []),
		update,
		updateMaster,
		reset,
		busy,
	};
}

function asFailure(error: unknown): ApiFailure {
	return error instanceof ApiFailure
		? error
		: new ApiFailure("unexpected-error", String(error), 0);
}

/** The same change the server will make, applied to the projection we already hold. */
export function applyLocally(
	outputs: OutputView[],
	outputId: string,
	layer: number,
	change: UpdateLayer,
): OutputView[] {
	return outputs.map((output) =>
		output.id === outputId
			? {
					...output,
					layers: output.layers.map((candidate) =>
						candidate.index === layer
							? withChange(candidate, change)
							: candidate,
					),
				}
			: output,
	);
}

function withChange(
	layer: OutputView["layers"][number],
	change: UpdateLayer,
): OutputView["layers"][number] {
	return {
		...layer,
		dimmer: change.dimmer ?? layer.dimmer,
		playModeDmx: change.playModeDmx ?? layer.playModeDmx,
		scaleX: change.scaleX ?? layer.scaleX,
		scaleY: change.scaleY ?? layer.scaleY,
		scalingMode: change.scalingMode ?? layer.scalingMode,
		positionX: change.positionX ?? layer.positionX,
		positionY: change.positionY ?? layer.positionY,
		rotation: change.rotation ?? layer.rotation,
		volume: change.volume ?? layer.volume,
		tintRed: change.tintRed ?? layer.tintRed,
		tintGreen: change.tintGreen ?? layer.tintGreen,
		tintBlue: change.tintBlue ?? layer.tintBlue,
		grayscale: change.grayscale ?? layer.grayscale,
		speedMultiplierDmx: change.speedMultiplierDmx ?? layer.speedMultiplierDmx,
		playbackBpm:
			change.playbackBpm === 0
				? null
				: (change.playbackBpm ?? layer.playbackBpm),
		address: {
			...layer.address,
			folder: change.folder ?? layer.address.folder,
			file: change.file ?? layer.address.file,
		},
		mask: {
			...layer.mask,
			address: {
				...layer.mask.address,
				folder: change.maskFolder ?? layer.mask.address.folder,
				file: change.maskFile ?? layer.mask.address.file,
			},
			scaleX: change.maskScaleX ?? layer.mask.scaleX,
			scaleY: change.maskScaleY ?? layer.mask.scaleY,
			invert: change.maskInvert ?? layer.mask.invert,
			opacity: change.maskOpacity ?? layer.mask.opacity,
		},
	};
}

function replaceOutput(
	outputs: OutputView[],
	updated: OutputView,
): OutputView[] {
	return outputs.map((output) => (output.id === updated.id ? updated : output));
}
