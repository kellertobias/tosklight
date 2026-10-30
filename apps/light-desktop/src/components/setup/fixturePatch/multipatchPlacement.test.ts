import { expect, it, vi } from "vitest";
import type { PatchController } from "./controller";
import {
	PRIMARY_PHYSICAL_PATCH,
	saveMultipatchVectorInput,
} from "./multipatchActions";

it("marks only physical copies receiving a location THRU edit, including zero", async () => {
	const copy = (id: string) => ({ id, location: { x: 0, y: 0, z: 0 }, stage_position_set: false });
	const updateFixture = vi.fn().mockResolvedValue(true);
	const fixture = { fixture_id: "root", location: { x: 0, y: 0, z: 0 },
		stage_position_set: false, multipatch: [copy("first"), copy("second")] };
	const controller = {
		data: { all: [fixture] },
		patch: { updateFixture },
		ui: { multipatchEdit: { fixtureId: "root", instanceId: "first", kind: "location", axis: "x",
			physicalTargets: [PRIMARY_PHYSICAL_PATCH, "first"] }, setMultipatchEdit: vi.fn(), setEditError: vi.fn() },
	} as unknown as PatchController;
	await saveMultipatchVectorInput(controller, "0 THRU 0");
	expect(updateFixture).toHaveBeenCalledOnce();
	const [id, changes] = updateFixture.mock.calls[0];
	expect(id).toBe("root");
	expect(changes.location).toEqual({ x: 0, y: 0, z: 0 });
	expect(changes.multipatch.map((item: { stage_position_set: boolean }) => item.stage_position_set))
		.toEqual([true, false]);
});
