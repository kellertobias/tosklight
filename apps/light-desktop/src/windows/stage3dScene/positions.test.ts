import { expect, it } from "vitest";
import { defaultStagePosition, migrateStagePosition, physicalStagePosition } from "./positions";

it.each(["root", "copy"])("keeps an explicitly placed %s at the Stage origin", (id) => {
	const instance = { id, location: { x: 0, y: 0, z: 0 }, stage_position_set: true };
	expect(physicalStagePosition(instance, {}, undefined, 3)).toMatchObject({ x: 0, y: 0, z: 0 });
	expect(physicalStagePosition({ ...instance, stage_position_set: false }, {}, undefined, 3))
		.toEqual(defaultStagePosition(3));
});

it("honours explicit layout overrides and legacy positions for unplaced fixtures", () => {
	const instance = { id: "root", location: { x: 0, y: 0, z: 0 }, stage_position_set: true };
	const override = { x: 2, y: 3, z: 4, rotationX: 0, rotationY: 0, rotationZ: 0 };
	expect(physicalStagePosition(instance, { root: override }, undefined, 0)).toBe(override);
	const legacy = { x: 20, y: 30, rotation: 90 };
	expect(physicalStagePosition({ ...instance, stage_position_set: false }, {}, legacy, 0))
		.toEqual(migrateStagePosition(legacy, 0));
});

it("uses the nonzero heuristic only for a legacy absent marker", () => {
	const instance = { id: "root", location: { x: 1000, y: 0, z: 0 } };
	expect(physicalStagePosition(instance, {}, undefined, 0)).toMatchObject({ x: 1, y: 0, z: 0 });
	expect(physicalStagePosition({ ...instance, stage_position_set: false }, {}, undefined, 0))
		.toEqual(defaultStagePosition(0));
	expect(physicalStagePosition({ id: "root", location: { x: 0, y: 0, z: 0 } }, {}, undefined, 0))
		.toEqual(defaultStagePosition(0));
});
