import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PatchParameterControls } from "./PatchParameterControls";

const mocks = vi.hoisted(() => ({
	selection: null as null | {
		fixtureId: string;
		multipatchInstanceId: string | null;
	},
	patchStatus: "ready" as "loading" | "ready",
	update: vi.fn(),
}));

const fixture = {
	fixture_id: "fixture-1",
	name: "Front Truss",
	definition: {
		name: "Fixture",
		mode_id: "mode-1",
		profile_snapshot: {
			modes: [
				{
					id: "mode-1",
					channels: [
						{
							attribute: "shaper.blade.1.position",
							functions: [],
						},
						{
							attribute: "shaper.blade.2.angle",
							functions: [],
						},
						{ attribute: "shaper.rotation", functions: [] },
					],
					geometry: { emitters: [{}] },
				},
			],
		},
	},
	logical_heads: [],
	location: { x: 100, y: 0, z: 0 },
	rotation: { x: 5, y: 0, z: 0 },
	bracket_angle: 0,
	shaper_angle: null,
	installed_appearance: {
		shaper_angles_degrees: [0, 0, 0, 0],
	},
	multipatch: [
		{
			id: "copy-1",
			name: "Balcony copy",
			universe: 2,
			address: 1,
			location: { x: 300, y: 0, z: 0 },
			rotation: { x: 15, y: 0, z: 0 },
			bracket_angle: 0,
			shaper_angle: null,
			installed_appearance: {
				shaper_angles_degrees: [0, 0, 0, 0],
			},
		},
	],
};
const secondFixture = {
	...fixture,
	fixture_id: "fixture-2",
	name: "Rear Truss",
	logical_heads: [{ fixture_id: "head-2" }],
	location: { x: 200, y: 0, z: 0 },
};

vi.mock("../../features/patch/PatchContext", () => ({
	usePatch: () => ({
		status: mocks.patchStatus,
		fixtures: mocks.patchStatus === "ready" ? [fixture, secondFixture] : [],
		selectedPatchInstance: mocks.selection,
		updateFixtureIntent: mocks.update,
	}),
	usePatchView: vi.fn(),
}));

beforeEach(() => {
	mocks.selection = null;
	mocks.patchStatus = "ready";
	mocks.update.mockReset().mockResolvedValue(true);
});

afterEach(cleanup);

describe("Patch parameter selection", () => {
	it("asks for a physical patch row and keeps every edit inert without one", () => {
		render(<PatchParameterControls />);

		expect(screen.getByText("Select a physical patch row")).toBeInTheDocument();
		for (const slot of locationEncoderSlots())
			expect(slot).toHaveAttribute("aria-disabled", "true");
	});

	it("does not expose stale fixtures or write while Patch authority loads", () => {
		mocks.patchStatus = "loading";
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		expect(screen.getByText("Patch loading…")).toBeInTheDocument();
		for (const slot of locationEncoderSlots())
			expect(slot).toHaveAttribute("aria-disabled", "true");
		expect(mocks.update).not.toHaveBeenCalled();
	});

	it("edits the selected primary physical instance", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		expect(screen.getByText("Front Truss")).toBeInTheDocument();
		fireEvent.keyDown(
			screen.getByRole("group", { name: "Enc 1 · Location X" }),
			{ key: "ArrowUp" },
		);

		expect(mocks.update).toHaveBeenCalledWith("fixture-1", null, {
			type: "set_location_axis",
			axis: "x",
			millimetres: 101,
		});
	});

	it("edits only the exact selected multi-patch instance", () => {
		mocks.selection = {
			fixtureId: "fixture-1",
			multipatchInstanceId: "copy-1",
		};
		render(<PatchParameterControls />);

		expect(screen.getByText("Balcony copy")).toBeInTheDocument();
		fireEvent.keyDown(
			screen.getByRole("group", { name: "Enc 1 · Location X" }),
			{ key: "ArrowUp" },
		);
		expect(mocks.update).toHaveBeenCalledWith("fixture-1", "copy-1", {
			type: "set_location_axis",
			axis: "x",
			millimetres: 301,
		});
	});

	it("does not retarget a stale multi-patch selection to its parent", () => {
		mocks.selection = {
			fixtureId: "fixture-1",
			multipatchInstanceId: "removed-copy",
		};
		render(<PatchParameterControls />);

		expect(screen.getByText("Select a physical patch row")).toBeInTheDocument();
		for (const slot of locationEncoderSlots())
			expect(slot).toHaveAttribute("aria-disabled", "true");
		expect(mocks.update).not.toHaveBeenCalled();
	});

	it("keeps exact Location and typed Visualization slot geometry", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		expect(
			locationEncoderSlots().map((slot) => slot.getAttribute("aria-label")),
		).toEqual([
			"Enc 1 · Location X",
			"Enc 2 · Location Y",
			"Enc 3 · Location Z",
			"Enc 4 · Rotation X",
			"Enc 5 · Rotation Y",
			"Enc 6 · Rotation Z",
		]);
		expect(screen.getByText("0.100 m")).toBeInTheDocument();
		expect(screen.getByText("5°")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Visualization" }));
		expect(
			screen
				.getAllByRole("group")
				.map((slot) => slot.getAttribute("aria-label")),
		).toEqual([
			"Enc 1 · Bracket",
			"Enc 2 · Shaper 1 Angle",
			"Enc 3 · Shaper 2 Angle",
			"Enc 4 · Shaper 3 Angle",
			"Enc 5 · Shaper 4 Angle",
			"Enc 6 · Shaper Module Rotation",
		]);
		expect(
			screen.getByRole("group", { name: "Enc 1 · Bracket" }),
		).toHaveAttribute("aria-disabled", "false");
		expect(
			screen.getByRole("group", { name: "Enc 2 · Shaper 1 Angle" }),
		).toHaveAttribute("aria-disabled", "false");
		for (const name of [
			"Enc 3 · Shaper 2 Angle",
			"Enc 4 · Shaper 3 Angle",
			"Enc 5 · Shaper 4 Angle",
			"Enc 6 · Shaper Module Rotation",
		])
			expect(screen.getByRole("group", { name })).toHaveAttribute(
				"aria-disabled",
				"true",
			);
	});

	it("routes installed visualization turns to the exact root or copy and blocks live roles", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		const rendered = render(<PatchParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Visualization" }));
		fireEvent.keyDown(screen.getByRole("group", { name: "Enc 1 · Bracket" }), {
			key: "ArrowUp",
		});
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_bracket_angle",
			degrees: 1,
		});
		fireEvent.keyDown(
			screen.getByRole("group", { name: "Enc 2 · Shaper 1 Angle" }),
			{ key: "ArrowRight" },
		);
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_static_shaper_angle",
			element: 1,
			degrees: 1,
		});
		const calls = mocks.update.mock.calls.length;
		fireEvent.keyDown(
			screen.getByRole("group", { name: "Enc 3 · Shaper 2 Angle" }),
			{ key: "ArrowUp" },
		);
		expect(mocks.update).toHaveBeenCalledTimes(calls);

		rendered.unmount();
		mocks.selection = {
			fixtureId: "fixture-1",
			multipatchInstanceId: "copy-1",
		};
		render(<PatchParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Visualization" }));
		fireEvent.keyDown(screen.getByRole("group", { name: "Enc 1 · Bracket" }), {
			key: "ArrowDown",
		});
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", "copy-1", {
			type: "set_bracket_angle",
			degrees: -1,
		});
	});

	it("routes fine and coarse hardware turns to the same exact physical target", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls hardwareConnected />);

		expect(
			screen.getByRole("button", { name: "Encoder 1: Location X, 0.100 m" }),
		).toBeInTheDocument();
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "up" },
			}),
		);
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_location_axis",
			axis: "x",
			millimetres: 101,
		});
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/4", value: "right" },
			}),
		);
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_rotation_axis",
			axis: "x",
			degrees: 15,
		});
	});

	it("uses the shared Set Value editor for exact absolute entry", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Location X value" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Enc 1 · Location X value",
		});
		for (let index = 0; index < 3; index += 1)
			fireEvent.click(within(dialog).getByRole("button", { name: "⌫" }));
		for (const key of ["1", ".", "2", "5", "ENTER"])
			fireEvent.click(within(dialog).getByRole("button", { name: key }));

		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_location_axis",
			axis: "x",
			millimetres: 1_250,
		});
	});

	it("opens the same exact Set Value path from a hardware encoder press", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls hardwareConnected />);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "press" },
			}),
		);
		const dialog = screen.getByRole("dialog", { name: "Encoder 1 value" });
		for (let index = 0; index < 3; index += 1)
			fireEvent.click(within(dialog).getByRole("button", { name: "⌫" }));
		for (const key of ["2", ".", "5", "ENTER"])
			fireEvent.click(within(dialog).getByRole("button", { name: key }));

		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", null, {
			type: "set_location_axis",
			axis: "x",
			millimetres: 2_500,
		});
	});
});

function locationEncoderSlots() {
	return screen.getAllByRole("group", { name: /^Enc [1-6] · / });
}
