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
	definition: { name: "Fixture" },
	logical_heads: [],
	location: { x: 100, y: 0, z: 0 },
	rotation: { x: 5, y: 0, z: 0 },
	multipatch: [
		{
			id: "copy-1",
			name: "Balcony copy",
			universe: 2,
			address: 1,
			location: { x: 300, y: 0, z: 0 },
			rotation: { x: 15, y: 0, z: 0 },
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
		updateFixture: mocks.update,
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

		expect(mocks.update).toHaveBeenCalledWith("fixture-1", {
			location: { x: 101, y: 0, z: 0 },
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
		expect(mocks.update).toHaveBeenCalledWith("fixture-1", {
			multipatch: [
				expect.objectContaining({
					id: "copy-1",
					location: { x: 301, y: 0, z: 0 },
				}),
			],
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

	it("keeps the exact Location slot order, units, and six-slot Visualization geometry", () => {
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
		for (let slot = 1; slot <= 6; slot += 1)
			expect(
				screen.getByRole("img", {
					name: `Visualization encoder ${slot} unavailable`,
				}),
			).toBeInTheDocument();
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
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", {
			location: { x: 101, y: 0, z: 0 },
		});
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/4", value: "right" },
			}),
		);
		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", {
			rotation: { x: 15, y: 0, z: 0 },
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

		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", {
			location: { x: 1_250, y: 0, z: 0 },
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

		expect(mocks.update).toHaveBeenLastCalledWith("fixture-1", {
			location: { x: 2_500, y: 0, z: 0 },
		});
	});
});

function locationEncoderSlots() {
	return screen.getAllByRole("group", { name: /^Enc [1-6] · / });
}
