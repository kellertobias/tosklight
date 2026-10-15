import { describe, expect, it } from "vitest";
import {
	effectiveOrderingShape,
	inheritsOrdering,
	phaseOrderingChoice,
	phaseOrderingForChoice,
	shapeForOrdering,
	orderingWithUpdatedShape,
} from "./phaseOrderingShape";

const live = { type: "live_group" as const, group_id: "25" };
const frozen = { type: "frozen_targets" as const, targets: ["1"] };
const mapping = { projection: { type: "inherit" as const }, shape: { type: "inherit" as const } };

describe("Phase-local ordering authority", () => {
	it("preserves legacy own shapes instead of pretending their mapping shape controls them", () => {
		expect(phaseOrderingChoice({ type: "grid_linear", angle_degrees: 45 }, mapping, live)).toBe("grid");
		expect(shapeForOrdering({ type: "radial_in", center_x: 1, center_z: 2 })).toEqual({ type: "radial", center_u: 1, center_v: 2, direction: "inward" });
		expect(shapeForOrdering({ type: "random_each_loop", seed: 9 })).toEqual({ type: "random", seed: 9 });
	});
	it("distinguishes explicit Linear from legacy Group Selection and new Inherit", () => {
		expect(phaseOrderingChoice({ type: "source_order" }, mapping, live)).toBe("source_order");
		expect(inheritsOrdering(mapping, live, { type: "selection" })).toBe(true);
		expect(inheritsOrdering(mapping, live, { type: "spatial", shape: { type: "inherit" } })).toBe(true);
		expect(inheritsOrdering(mapping, frozen, { type: "spatial", shape: { type: "inherit" } })).toBe(false);
	});
	it("legacy Selection still reads its own saved shape, without changing it", () => {
		const shape = { type: "grid" as const, angle_degrees: 90, direction: "descending" as const };
		expect(effectiveOrderingShape({ type: "selection" }, { ...mapping, shape: { type: "replace", value: shape } })).toEqual(shape);
	});
	it("authors only Phase ordering and preserves complete current shape parameters", () => {
		const shape = { type: "radar" as const, center_u: -3, center_v: 2, start_angle_degrees: 225, sweep: "counter_clockwise" as const };
		expect(phaseOrderingForChoice("radar", shape)).toEqual({ type: "spatial", shape: { type: "replace", value: shape } });
		expect(phaseOrderingForChoice("source_order", shape)).toEqual({ type: "source_order" });
		expect(phaseOrderingForChoice("inherit", shape)).toEqual({ type: "spatial", shape: { type: "inherit" } });
	});
});

it("preserves legacy axial and random parameter semantics", () => {
	expect(shapeForOrdering({ type: "axial", center_x: 1, center_z: 2 })).toEqual({
		type: "radar", center_u: 1, center_v: 2, start_angle_degrees: -180, sweep: "counter_clockwise",
	});
	expect(orderingWithUpdatedShape({ type: "random_each_loop", seed: 2 }, { type: "random", seed: 7 }))
		.toEqual({ type: "random_each_loop", seed: 7 });
});
