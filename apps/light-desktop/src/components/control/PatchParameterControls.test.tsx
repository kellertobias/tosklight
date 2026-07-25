import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PatchParameterControls } from "./PatchParameterControls";

const mocks = vi.hoisted(() => ({
	selection: null as null | { selected: readonly string[] },
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
		updateFixture: mocks.update,
	}),
	usePatchView: vi.fn(),
}));

vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: () => mocks.selection,
	}),
);

beforeEach(() => {
	mocks.selection = null;
	mocks.patchStatus = "ready";
	mocks.update.mockReset().mockResolvedValue(true);
});

afterEach(cleanup);

describe("Patch parameter selection", () => {
	it("shows selection loading and keeps every edit inert without scoped authority", () => {
		render(<PatchParameterControls />);

		expect(
			screen.getByText("Programmer selection loading…"),
		).toBeInTheDocument();
		for (const button of screen.getAllByRole("button"))
			expect(button).toBeDisabled();
	});

	it("does not expose stale fixtures or write while Patch authority loads", () => {
		mocks.patchStatus = "loading";
		mocks.selection = { selected: ["fixture-1"] };
		render(<PatchParameterControls />);

		expect(screen.getByText("Patch loading…")).toBeInTheDocument();
		for (const button of screen.getAllByRole("button"))
			expect(button).toBeDisabled();
		expect(mocks.update).not.toHaveBeenCalled();
	});

	it("edits the first fixture selected by the scoped projection", () => {
		mocks.selection = { selected: ["fixture-1"] };
		render(<PatchParameterControls />);

		expect(screen.getByText("Front Truss")).toBeInTheDocument();
		const locationX = screen.getByText("Location X").closest("div")!;
		fireEvent.click(locationX.querySelectorAll("button")[1]);

		expect(mocks.update).toHaveBeenCalledWith("fixture-1", {
			location: { x: 110, y: 0, z: 0 },
		});
	});

	it("respects ordered logical-head selection when choosing a patched fixture", () => {
		mocks.selection = { selected: ["head-2", "fixture-1"] };
		render(<PatchParameterControls />);

		expect(screen.getByText("Rear Truss")).toBeInTheDocument();
		const locationX = screen.getByText("Location X").closest("div")!;
		fireEvent.click(locationX.querySelectorAll("button")[1]);
		expect(mocks.update).toHaveBeenCalledWith("fixture-2", {
			location: { x: 210, y: 0, z: 0 },
		});
	});
});
