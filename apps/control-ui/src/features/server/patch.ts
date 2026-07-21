import type { PatchLayer, VersionedObject } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createPatchActions(
	model: ServerController,
): Pick<ServerContextValue, "savePatchLayer"> {
	const { client, setError, bootstrap, patchLayers, setPatchLayers } = model;
	return {
		savePatchLayer: async (layer) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("No active show is available");
				const existing = patchLayers.find((item) => item.id === layer.id);
				const outcome = await client.putObject(
					bootstrap.active_show.id,
					"patch_layer",
					layer.id,
					layer,
					existing?.revision ?? 0,
				);
				setPatchLayers((current) =>
					reconcileSavedLayer(current, layer, outcome.revision),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}

export function reconcileSavedLayer(
	current: VersionedObject<PatchLayer>[],
	layer: PatchLayer,
	revision: number,
): VersionedObject<PatchLayer>[] {
	const existing = current.find((item) => item.id === layer.id);
	if (existing && existing.revision > revision) return current;
	return [
		...current.filter((item) => item.id !== layer.id),
		{
			kind: "patch_layer",
			id: layer.id,
			revision,
			updated_at: existing?.updated_at ?? "",
			body: layer,
		},
	];
}
