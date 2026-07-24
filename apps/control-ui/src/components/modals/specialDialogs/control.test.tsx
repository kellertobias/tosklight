import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FixtureDefinition, PatchedFixture } from "../../../api/types";
import { ControlDialog } from "./control";

const mocks = vi.hoisted(() => ({
	fixtures: [] as PatchedFixture[],
	controlFixtureAction: vi.fn().mockResolvedValue(undefined),
	generateFixturePresets: vi.fn().mockResolvedValue({ created: [] }),
}));

vi.mock("../../../features/patch/PatchState", () => ({
	useSelectedPatchedFixtures: () => mocks.fixtures,
}));

vi.mock("../../../features/programmerActions/ProgrammerActionsContext", () => ({
	useProgrammerActions: () => ({
		controlFixtureAction: mocks.controlFixtureAction,
		generateFixturePresets: mocks.generateFixturePresets,
	}),
}));

function fixture(): PatchedFixture {
	const modeId = "mode-1";
	return {
		fixture_id: "fixture-1",
		fixture_number: 1,
		name: "Fixture",
		universe: 1,
		address: 1,
		layer_id: "default",
		definition: {
			schema_version: 2,
			id: "definition-1",
			revision: 1,
			manufacturer: "Test",
			device_type: "profile",
			name: "Fixture",
			model: "Fixture",
			mode: "Test",
			mode_id: modeId,
			footprint: 1,
			heads: [],
			color_calibration: null,
			physical: {},
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			profile_snapshot: {
				schema_version: 2,
				id: "profile-1",
				revision: 1,
				manufacturer: "Test",
				name: "Fixture",
				short_name: "Fixture",
				fixture_type: "Moving light",
				notes: "",
				photograph_asset: null,
				stage_icon_asset: null,
				model_asset: null,
				physical: {
					width_millimetres: null,
					height_millimetres: null,
					depth_millimetres: null,
					weight_kilograms: null,
					power_watts: null,
				},
				modes: [
					{
						id: modeId,
						name: "Test",
						notes: "",
						splits: [],
						heads: [],
						channels: [],
						color_systems: [],
						control_actions: [
							{
								id: "fan-high",
								name: "Quiet Fan",
								semantic: "fan_high",
								kind: "latched",
								duration_millis: null,
								assignments: [],
							},
							{
								id: "custom-hold",
								name: "Service Hold",
								semantic: "custom",
								kind: "momentary",
								duration_millis: null,
								assignments: [],
							},
							{
								id: "custom-pulse",
								name: "Factory Pulse",
								semantic: "custom",
								kind: "timed_pulse",
								duration_millis: 750,
								assignments: [],
							},
						],
						geometry: { nodes: [], emitters: [] },
					},
				],
				hazardous: false,
				direct_control_protocols: [],
				signal_loss_policy: { type: "hold_last" },
				reserved_source: null,
			},
		} as FixtureDefinition,
		logical_heads: [],
	};
}

describe("ControlDialog", () => {
	afterEach(cleanup);

	beforeEach(() => {
		mocks.fixtures = [fixture()];
		mocks.controlFixtureAction.mockClear();
		mocks.generateFixturePresets.mockReset();
		mocks.generateFixturePresets.mockResolvedValue({
			created: [{ name: "Open" }],
		});
	});

	it("exposes authored actions with their complete lifecycle", async () => {
		render(<ControlDialog selectedFixtureIds={["fixture-1"]} />);

		expect(
			screen.getByRole("button", { name: "Fan High" }),
		).toBeInTheDocument();

		const momentary = screen.getByRole("button", {
			name: "Service Hold momentary control action",
		});
		fireEvent.pointerDown(momentary);
		fireEvent.pointerUp(momentary);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Factory Pulse timed_pulse control action",
			}),
		);

		const latched = screen.getByRole("button", {
			name: "Quiet Fan latched control action",
		});
		fireEvent.click(latched);
		await waitFor(() =>
			expect(mocks.controlFixtureAction).toHaveBeenCalledWith(
				"fixture-1",
				"fan-high",
				true,
			),
		);
		fireEvent.click(latched);

		await waitFor(() =>
			expect(mocks.controlFixtureAction.mock.calls).toEqual([
				["fixture-1", "custom-hold", true],
				["fixture-1", "custom-hold", false],
				["fixture-1", "custom-pulse", true],
				["fixture-1", "fan-high", true],
				["fixture-1", "fan-high", false],
			]),
		);
	});

	it("generates portable presets only after the explicit action", async () => {
		render(<ControlDialog selectedFixtureIds={["fixture-1"]} />);

		expect(mocks.generateFixturePresets).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Generate portable presets" }),
		);

		expect(mocks.generateFixturePresets).toHaveBeenCalledWith(["fixture-1"]);
		expect(await screen.findByRole("status")).toHaveTextContent(
			"Created 1 portable preset",
		);
	});
});
