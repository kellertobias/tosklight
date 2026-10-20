import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { PatchFixtureProjection } from "@tosklight/patch";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, expect, it, vi } from "vitest";
import { newArchitecture, snapWallDraft } from "./architectureDraft";
import { CadArchitectureModal } from "./CadArchitectureModal";

const mocks = vi.hoisted(() => ({
	patchSnapshot: vi.fn(),
	patchVenue: vi.fn(),
	snapshot: vi.fn(),
}));
vi.mock("../document/session", () => ({
	documentSession: { patchSnapshot: mocks.patchSnapshot },
}));
vi.mock("./session", () => ({
	cadSession: { snapshot: mocks.snapshot, patchVenue: mocks.patchVenue },
}));
vi.mock("./cadPlacement", () => ({ placeProfile: vi.fn() }));
const wall = () =>
	({
		fixtureId: "wall-a",
		name: "North wall",
		sceneryOptions: {
			architecture: {
				kind: "wall",
				openings: [],
				ceiling_height_millimetres: 4000,
			},
		},
		scenerySizeMetres: { x: 6000, y: 4000, z: 200 },
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
	}) as unknown as PatchFixtureProjection;
beforeEach(() => {
	vi.clearAllMocks();
	mocks.patchSnapshot.mockResolvedValue({
		fixtures: [],
		showId: "show",
		patchRevision: 7,
	});
	mocks.snapshot.mockResolvedValue({ sceneRevision: 9 });
	mocks.patchVenue.mockResolvedValue({});
});
function edit(fixture = wall()) {
	render(
		<ModalProvider>
			<CadArchitectureModal fixture={fixture} onClose={() => {}} />
		</ModalProvider>,
	);
}
function number(label: string, value: string) {
	const input = screen.getByRole("textbox", { name: label });
	fireEvent.change(input, { target: { value } });
	fireEvent.blur(input);
}
it("stores a wall and attached door/window in one undoable command with stable opening identities", async () => {
	edit();
	fireEvent.click(screen.getByRole("button", { name: "Add window" }));
	fireEvent.change(screen.getByRole("combobox", { name: "Window 1 shape" }), {
		target: { value: "round" },
	});
	number("Opening 1 width", "1.2");
	fireEvent.click(screen.getByRole("button", { name: "Add door" }));
	fireEvent.change(screen.getByRole("combobox", { name: "Door 2 leaves" }), {
		target: { value: "double" },
	});
	fireEvent.change(screen.getByRole("combobox", { name: "Door 2 swing" }), {
		target: { value: "outward" },
	});
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	await waitFor(() => expect(mocks.patchVenue).toHaveBeenCalledOnce());
	const [revision, fixtures] = mocks.patchVenue.mock.calls[0];
	expect(revision).toBe(9);
	expect(fixtures[0].sceneryOptions.architecture.openings).toEqual([
		expect.objectContaining({
			id: expect.any(String),
			width_millimetres: 1200,
			height_millimetres: 1200,
			opening: { kind: "window", shape: "round" },
		}),
		expect.objectContaining({
			sill_millimetres: 0,
			opening: {
				kind: "door",
				leaves: "double",
				swing: "outward",
				hinge: "left",
			},
		}),
	]);
});
it("edits a balcony outline and independent height while preserving its placement", async () => {
	const fixture = {
		...wall(),
		sceneryOptions: { architecture: newArchitecture("slab").architecture },
	} as PatchFixtureProjection;
	edit(fixture);
	expect(screen.queryByRole("group", { name: "Dimensions" })).toBeNull();
	number("Elevation above floor", "4.8");
	number("Point 1 X", "-4");
	number("Slab thickness", "0.3");
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	await waitFor(() => expect(mocks.patchVenue).toHaveBeenCalledOnce());
	expect(mocks.patchVenue.mock.calls[0][1][0]).toMatchObject({
		location: { x: 0, y: 0, z: 0 },
		sceneryOptions: {
			architecture: {
				elevation_millimetres: 4800,
				thickness_millimetres: 300,
				outline_millimetres: [
					{ x: -4000, y: -2000 },
					{ x: 3000, y: -2000 },
					{ x: 3000, y: 2000 },
					{ x: -3000, y: 2000 },
				],
			},
		},
	});
});
it("keeps a refused venue edit open and displays the authoritative validation reason", async () => {
	mocks.patchVenue.mockRejectedValue(
		new Error("venue wall openings must not overlap"),
	);
	edit();
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	expect(await screen.findByRole("alert")).toHaveTextContent(
		"must not overlap",
	);
	expect(screen.getByRole("dialog")).toBeVisible();
});
it("snaps a chosen endpoint to a rotated wall and excludes the wall being edited", () => {
	const other = {
		...wall(),
		fixtureId: "other",
		location: { x: 3100, y: 3000, z: 0 },
		rotation: { x: 0, y: 0, z: 90 },
	};
	const draft = newArchitecture("wall");
	expect(snapWallDraft(draft, [other]).position).toEqual({
		x: 100,
		y: 0,
		z: 0,
	});
	expect(snapWallDraft(draft, [other], "other")).toEqual(draft);
});
