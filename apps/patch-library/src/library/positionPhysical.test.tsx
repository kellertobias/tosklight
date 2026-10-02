import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, it, expect } from "vitest";
import { useState } from "react";
import {
	blankFixtureProfile,
	blankChannel,
	blankFunction,
	geometryTemplate,
	modeGeometry,
} from "../sheet/fixtureProfileModel";
import { positionPhysicalErrors } from "../sheet/fixtureProfileModel/positionPhysical";
import { GeometryPhysicalContractEditor } from "./geometryPhysicalContract";
import { PositionPhysicalBindings } from "./positionPhysicalBindings";
afterEach(cleanup);
function example() {
	const profile = blankFixtureProfile(),
		mode = profile.modes[0];
	profile.geometry = geometryTemplate("moving_head", []);
	// The template needs a head to create its tilt axis.
	profile.geometry = geometryTemplate("moving_head", [mode.heads[0].id]);
	profile.geometry.emitters = [];
	const channel = blankChannel(mode, 1);
	channel.attribute = "pan";
	channel.fixture_attribute = "pan";
	channel.functions = [blankFunction(channel)];
	channel.functions[0].attribute = "pan";
	channel.functions[0].behavior = {
		type: "continuous",
		physical_min: 720,
		physical_max: -720,
		unit: "deg",
	};
	channel.functions[0].angular_motion = { kind: "absolute_position" };
	mode.channels = [channel];
	return profile;
}
describe("physical fixture authoring", () => {
	it("requires explicit geometry, authors a hinge and selects an exact native function", () => {
		let state = example();
		function Editor() {
			const [p, set] = useState(state);
			state = p;
			return (
				<>
					<GeometryPhysicalContractEditor
						geometry={p.geometry!}
						onChange={(geometry) => set({ ...p, geometry })}
					/>
					<PositionPhysicalBindings
						mode={p.modes[0]}
						geometry={p.geometry!}
						onChange={(mode) => set({ ...p, modes: [mode] })}
					/>
				</>
			);
		}
		render(<Editor />);
		expect(
			screen.getByRole("button", { name: "Configure physical Position" }),
		).toBeDisabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Declare physical geometry" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Bracket geometry" }));
		fireEvent.click(screen.getByRole("option", { name: "Hinge" }));
		expect(state.geometry!.physical_contract!.bracket.kind).toBe("hinge");
		fireEvent.click(
			screen.getByRole("button", { name: "Configure physical Position" }),
		);
		expect(state.modes[0].position_physical?.bindings[0]).toMatchObject({
			channel_id: state.modes[0].channels[0].id,
			function_id: state.modes[0].channels[0].functions[0].id,
			role: "pan",
		});
		expect(positionPhysicalErrors(state)).toEqual([]);
		fireEvent.click(screen.getByRole("button", { name: "Remove binding 1" }));
		expect(state.modes[0].position_physical).toBeNull();
	});
	it("preserves a mode-owned declaration and rejects static, unit and scale mistakes", () => {
		const p = example(),
			m = p.modes[0],
			c = m.channels[0];
		p.geometry!.physical_contract = {
			version: 1,
			provenance: { quality: "unknown", revision: 0 },
			bracket: { kind: "fixed" },
		};
		m.geometry = p.geometry!;
		p.geometry = { nodes: [], emitters: [] };
		m.position_physical = {
			version: 1,
			revision: 0,
			bindings: [
				{
					node_id: m.geometry.nodes[1].id,
					channel_id: c.id,
					function_id: c.functions[0].id,
					role: "pan",
				},
			],
		};
		expect(modeGeometry(p, m).physical_contract).toEqual(
			m.geometry.physical_contract,
		);
		expect(positionPhysicalErrors(p)).toEqual([]);
		c.behavior = "static";
		expect(positionPhysicalErrors(p).join()).toContain("controllable");
		c.behavior = "controlled";
		c.functions[0].angular_motion = { kind: "angular_velocity" };
		expect(positionPhysicalErrors(p).join()).toContain("unit disagrees");
		m.geometry.nodes[0].transform.scale.x = 2;
		expect(positionPhysicalErrors(p).join()).toContain("identity scale");
	});
});
