// Layer control: the writes, their optimistic effect, and their rollback.
//
// An optimistic change here is not a guess about what the server will do — the projection the
// server returns replaces it either way. It exists so a fader feels attached to the value. The
// rollback matters more than the optimism: when a desk takes an output, the write is refused and
// the control must snap back to what the desk says, not to what the operator dragged.

import { useCallback, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import type { OutputView, UpdateLayer } from "../../shared/api/generated/media-wire";
import { KEYS } from "../../shared/api/queries";
import { useResource, writeResource } from "../../shared/api/resource";

/** Outputs change continuously while a desk is driving them. */
export const OUTPUT_POLL_MS = 1_000;

export function useOutputsForControl() {
	return useResource<OutputView[]>(KEYS.outputs, api.outputs, { pollMs: OUTPUT_POLL_MS });
}

export interface LayerControl {
	/** The last write that was refused, still on screen so the operator learns why. */
	refusal: ApiFailure | undefined;
	dismissRefusal: () => void;
	update: (output: OutputView, layer: number, change: UpdateLayer) => Promise<void>;
	reset: (output: OutputView, layer: number) => Promise<void>;
	busy: boolean;
}

export function useLayerControl(outputs: OutputView[] | undefined): LayerControl {
	const [refusal, setRefusal] = useState<ApiFailure | undefined>(undefined);
	const [busy, setBusy] = useState(false);

	const update = useCallback(
		async (output: OutputView, layer: number, change: UpdateLayer) => {
			const previous = outputs;
			if (previous) writeResource(KEYS.outputs, applyLocally(previous, output.id, layer, change));
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

	return {
		refusal,
		dismissRefusal: useCallback(() => setRefusal(undefined), []),
		update,
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
						candidate.index === layer ? withChange(candidate, change) : candidate,
					),
				}
			: output,
	);
}

function withChange(layer: OutputView["layers"][number], change: UpdateLayer): OutputView["layers"][number] {
	return {
		...layer,
		dimmer: change.dimmer ?? layer.dimmer,
		address: {
			...layer.address,
			folder: change.folder ?? layer.address.folder,
			file: change.file ?? layer.address.file,
		},
	};
}

function replaceOutput(outputs: OutputView[], updated: OutputView): OutputView[] {
	return outputs.map((output) => (output.id === updated.id ? updated : output));
}
