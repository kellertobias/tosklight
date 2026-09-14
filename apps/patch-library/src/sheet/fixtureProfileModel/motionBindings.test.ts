import { describe, expect, it } from "vitest";
import type { FixtureMode, FixtureProfile, GeometryGraph } from "../../wire";
import { geometryTemplate, liftMotionAttributes, modeGeometry } from "./geometry";

function mode(id: string, extra: Partial<FixtureMode> = {}): FixtureMode {
	return {
		id,
		name: id,
		notes: "",
		splits: [{ number: 1, footprint: 1 }],
		heads: [{ id: `${id}-head`, name: "Main", master_shared: false }],
		channels: [],
		color_systems: [],
		control_actions: [],
		geometry: { nodes: [], emitters: [] },
		...extra,
	};
}

function profile(geometry: GeometryGraph, modes: FixtureMode[]) {
	return { geometry, modes } as Pick<FixtureProfile, "geometry" | "modes">;
}

describe("moving parts bound per mode", () => {
	it("moves an attribute written on a part into every mode that does not bind the part", () => {
		const geometry = geometryTemplate("moving_head", ["head"]);
		const [, pan, tilt] = geometry.nodes;
		const lifted = liftMotionAttributes(
			profile(geometry, [
				mode("standard"),
				// A mode that already says what drives the pan keeps its own choice.
				mode("swapped", {
					motion_attributes: [{ node_id: pan.id, attribute: "tilt" }],
				}),
			]),
		);

		expect(lifted.geometry?.nodes.map((node) => node.motion?.attribute ?? null)).toEqual([
			null,
			null,
			null,
		]);
		expect(lifted.modes[0].motion_attributes).toEqual([
			{ node_id: pan.id, attribute: "pan" },
			{ node_id: tilt.id, attribute: "tilt" },
		]);
		expect(lifted.modes[1].motion_attributes).toEqual([
			{ node_id: pan.id, attribute: "tilt" },
			{ node_id: tilt.id, attribute: "tilt" },
		]);
		// Nothing left to move, so the profile is returned as it is.
		expect(liftMotionAttributes(lifted)).toBe(lifted);
	});

	it("draws a mode's moving parts with the attribute that mode binds, and rests the rest", () => {
		const { geometry, modes } = liftMotionAttributes(
			profile(geometryTemplate("moving_head", ["head"]), [mode("standard")]),
		);
		const [, pan, tilt] = geometry?.nodes ?? [];
		const onlyPan = {
			...modes[0],
			motion_attributes: [{ node_id: pan.id, attribute: "pan" }],
		};
		const bound = modeGeometry({ geometry }, onlyPan);

		expect(bound.nodes.find((node) => node.id === pan.id)?.motion?.attribute).toBe("pan");
		expect(bound.nodes.find((node) => node.id === tilt.id)?.motion?.attribute).toBeNull();
	});
});
