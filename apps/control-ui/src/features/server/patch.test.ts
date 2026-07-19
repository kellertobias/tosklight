import { describe, expect, it, vi } from "vitest";
import type { PatchLayer, VersionedObject } from "../../api/types";
import type { ServerController } from "./model";
import { createPatchActions, reconcileSavedLayer } from "./patch";

function storedLayer(
	id: string,
	revision: number,
): VersionedObject<PatchLayer> {
	return {
		kind: "patch_layer",
		id,
		revision,
		updated_at: "2026-07-18T00:00:00Z",
		body: { id, name: id, order: revision },
	};
}

describe("Patch layer reconciliation", () => {
	it("applies the targeted write without replacing unrelated layers", () => {
		const current = [storedLayer("default", 0), storedLayer("front", 1)];
		const saved = reconcileSavedLayer(
			current,
			{ id: "front", name: "Front truss", order: 2 },
			2,
		);

		expect(saved).toEqual([
			current[0],
			{
				...current[1],
				revision: 2,
				body: { id: "front", name: "Front truss", order: 2 },
			},
		]);
	});

	it("does not overwrite a newer event projection", () => {
		const current = [storedLayer("front", 3)];
		expect(
			reconcileSavedLayer(
				current,
				{ id: "front", name: "Stale response", order: 2 },
				2,
			),
		).toBe(current);
	});

	it("saves one layer without reloading unrelated show objects", async () => {
		let layers = [storedLayer("default", 0)];
		const loadShowObjects = vi.fn();
		const refresh = vi.fn();
		const setPatchLayers = vi.fn(
			(update: (current: typeof layers) => typeof layers) => {
				layers = update(layers);
			},
		);
		const model = {
			client: { putObject: vi.fn().mockResolvedValue({ revision: 1 }) },
			bootstrap: { active_show: { id: "show-1" } },
			patchLayers: layers,
			setPatchLayers,
			setError: vi.fn(),
			loadShowObjects,
			refresh,
		} as unknown as ServerController;

		const saved = await createPatchActions(model).savePatchLayer({
			id: "front",
			name: "Front truss",
			order: 1,
		});

		expect(saved).toBe(true);
		expect(layers.map((layer) => layer.id)).toEqual(["default", "front"]);
		expect(loadShowObjects).not.toHaveBeenCalled();
		expect(refresh).not.toHaveBeenCalled();
	});
});
