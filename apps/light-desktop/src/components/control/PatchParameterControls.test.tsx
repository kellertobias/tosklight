import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
		for (const button of screen.getAllByRole("button"))
			expect(button).toBeDisabled();
	});

	it("does not expose stale fixtures or write while Patch authority loads", () => {
		mocks.patchStatus = "loading";
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		expect(screen.getByText("Patch loading…")).toBeInTheDocument();
		for (const button of screen.getAllByRole("button"))
			expect(button).toBeDisabled();
		expect(mocks.update).not.toHaveBeenCalled();
	});

	it("edits the selected primary physical instance", () => {
		mocks.selection = { fixtureId: "fixture-1", multipatchInstanceId: null };
		render(<PatchParameterControls />);

		expect(screen.getByText("Front Truss")).toBeInTheDocument();
		const locationX = screen.getByText("Location X").closest("div")!;
		fireEvent.click(locationX.querySelectorAll("button")[1]);

		expect(mocks.update).toHaveBeenCalledWith("fixture-1", {
			location: { x: 110, y: 0, z: 0 },
		});
	});

	it("edits only the exact selected multi-patch instance", () => {
		mocks.selection = {
			fixtureId: "fixture-1",
			multipatchInstanceId: "copy-1",
		};
		render(<PatchParameterControls />);

		expect(screen.getByText("Balcony copy")).toBeInTheDocument();
		const locationX = screen.getByText("Location X").closest("div")!;
		fireEvent.click(locationX.querySelectorAll("button")[1]);
		expect(mocks.update).toHaveBeenCalledWith("fixture-1", {
			multipatch: [
				expect.objectContaining({
					id: "copy-1",
					location: { x: 310, y: 0, z: 0 },
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
		for (const button of screen.getAllByRole("button"))
			expect(button).toBeDisabled();
		expect(mocks.update).not.toHaveBeenCalled();
	});
});
