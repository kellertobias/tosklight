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
	placeProfile: vi.fn(),
}));
vi.mock("../document/session", () => ({
	documentSession: { patchSnapshot: mocks.patchSnapshot },
}));
vi.mock("./session", () => ({
	cadSession: { snapshot: mocks.snapshot, patchVenue: mocks.patchVenue },
}));
vi.mock("./cadPlacement", () => ({ placeProfile: mocks.placeProfile }));
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
	mocks.placeProfile.mockResolvedValue({
		ok: true,
		fixtureIds: ["new-stairs"],
	});
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

it("creates stairs with explicit width, rise, run and an integer step count", async () => {
	const onPlaced = vi.fn();
	render(
		<ModalProvider>
			<CadArchitectureModal onClose={() => {}} onPlaced={onPlaced} />
		</ModalProvider>,
	);
	fireEvent.change(
		screen.getByRole("combobox", { name: "Venue element type" }),
		{ target: { value: "stairs" } },
	);
	expect(screen.getByRole("textbox", { name: "Width" })).toHaveValue("1.2");
	expect(screen.getByRole("textbox", { name: "Rise" })).toHaveValue("3");
	expect(screen.getByRole("textbox", { name: "Run" })).toHaveValue("4.5");
	expect(screen.getByRole("textbox", { name: "Steps" })).toHaveValue("16");
	number("Rise", "3.6");
	number("Run", "5.4");
	number("Steps", "18");
	fireEvent.click(screen.getByRole("button", { name: "Place" }));
	await waitFor(() => expect(onPlaced).toHaveBeenCalledWith(["new-stairs"]));
	expect(mocks.placeProfile.mock.calls[0][4]).toMatchObject({
		sceneryOptions: { architecture: { kind: "stairs", steps: 18 } },
		scenerySizeMetres: { x: 1200, y: 3600, z: 5400 },
	});
});
it("edits stair dimensions and steps through the whole-assembly undoable command", async () => {
	edit({
		...wall(),
		sceneryOptions: { architecture: { kind: "stairs", steps: 16 } },
		scenerySizeMetres: { x: 1200, y: 3000, z: 4500 },
	} as PatchFixtureProjection);
	number("Steps", "20");
	number("Rise", "4");
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	await waitFor(() => expect(mocks.patchVenue).toHaveBeenCalledOnce());
	expect(mocks.patchVenue.mock.calls[0][1][0]).toMatchObject({
		sceneryOptions: { architecture: { kind: "stairs", steps: 20 } },
		scenerySizeMetres: { x: 1200, y: 4000, z: 4500 },
	});
});
it("preserves a wall's stored colour and commits picker or hex changes with its complete assembly", async () => {
	edit({
		...wall(),
		sceneryOptions: {
			colourSrgb: "#aabbcc",
			architecture: { kind: "wall", openings: [] },
		},
	} as PatchFixtureProjection);
	expect(screen.getByLabelText("Wall colour")).toHaveValue("#aabbcc");
	fireEvent.change(screen.getByLabelText("Wall colour"), {
		target: { value: "#112233" },
	});
	const hex = screen.getByRole("textbox", { name: "Wall colour hex" });
	expect(hex).toHaveValue("#112233");
	fireEvent.change(hex, { target: { value: "#334455" } });
	fireEvent.blur(hex);
	fireEvent.click(screen.getByRole("button", { name: "Apply" }));
	await waitFor(() => expect(mocks.patchVenue).toHaveBeenCalledOnce());
	expect(mocks.patchVenue.mock.calls[0][1][0].sceneryOptions).toMatchObject({
		colourSrgb: "#334455",
		architecture: { kind: "wall", openings: [] },
	});
});

it("authors a floor with its own outline, floor datum and thickness", async () => {
	const onPlaced = vi.fn();
	render(
		<ModalProvider>
			<CadArchitectureModal onClose={() => {}} onPlaced={onPlaced} />
		</ModalProvider>,
	);
	fireEvent.change(
		screen.getByRole("combobox", { name: "Venue element type" }),
		{ target: { value: "slab" } },
	);
	expect(screen.getByRole("option", { name: "Floor" })).toBeInTheDocument();
	fireEvent.change(screen.getByRole("combobox", { name: "Slab type" }), {
		target: { value: "floor" },
	});
	number("Elevation above floor", "0");
	number("Slab thickness", "0.15");
	fireEvent.click(screen.getByRole("button", { name: "Place" }));
	await waitFor(() => expect(onPlaced).toHaveBeenCalled());
	expect(
		mocks.placeProfile.mock.calls[0][4].sceneryOptions.architecture,
	).toMatchObject({
		kind: "slab",
		purpose: "floor",
		elevation_millimetres: 0,
		thickness_millimetres: 150,
		outline_millimetres: expect.any(Array),
	});
});
