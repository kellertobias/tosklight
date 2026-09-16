import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadElementsPanel } from "./CadElementsPanel";
import type { CadTools } from "./cadTools";
import type { CadEntity } from "./types";
import type { CadUnderlays } from "./useCadUnderlays";
import type { VenueGroups } from "./venueGroups";

const tauri = vi.hoisted(() => ({
	stored: { groups: [] } as { groups: unknown[] },
	invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: tauri.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(() => Promise.resolve(() => undefined)),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../document/session", () => ({ documentSession: {} }));

beforeEach(() => {
	tauri.stored = { groups: [] };
	tauri.invoke.mockReset().mockImplementation((command: string, args?: { groups: VenueGroups }) => {
		if (command === "cad_venue_groups") return Promise.resolve(tauri.stored);
		if (command === "save_cad_venue_groups") return Promise.resolve(args?.groups);
		return Promise.resolve({ folders: [], items: {} });
	});
});

function venue(id: string, name: string, extra: Partial<CadEntity> = {}): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name,
		fixtureNumber: null,
		fixtureDisplayId: `0.${id.slice(0, 1)}`,
		dmxAddress: "Visual only",
		fixtureProfile: `Venue ${name}`,
		kind: "venue",
		fixtureType: "venue",
		drawingId: `drawing:${id}`,
		layerId: "default",
		selectable: true,
		positionMillimetres: [0, 0, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [1000, 1000, 1000],
		outputDirection: [0, 1, 0],
		...extra,
	};
}

function renderObjects(entities: CadEntity[], selectedIds: string[] = []) {
	const onSelect = vi.fn();
	render(
		<CadElementsPanel
			tab="objects"
			requests={{ newFolder: 0, chooseDrawing: 0, importModel: 0 }}
			documentKey="show"
			underlayState={{ underlays: [] } as unknown as CadUnderlays}
			tools={{ annotations: [] } as unknown as CadTools}
			defaultView="top_down"
			entities={entities}
			selectedIds={selectedIds}
			onSelect={onSelect}
		/>,
	);
	return onSelect;
}

const names = (region: HTMLElement) =>
	within(region)
		.queryAllByRole("button")
		.map((row) => row.querySelector("strong")?.textContent)
		.filter(Boolean);

describe("Elements › Objects", () => {
	it("lists only imported models under 3D models and every shipped Venue object under Venue items", () => {
		renderObjects([
			venue("1", "Four-Point Truss", {
				scenery: { kind: "truss", chords: 4, pattern: "standard" },
			}),
			// Model-backed shipped parts carry no generated scenery, yet they are Venue items.
			venue("2", "Four-Point Truss Corner 2-Way"),
			venue("3", "Stage Deck 2 × 1 m, Legs 0.4 m"),
			venue("4", "Disco Ball 50 cm"),
			venue("5", "Hall", { fixtureProfile: "Imported models Hall", importedModel: true }),
		]);
		expect(names(screen.getByRole("region", { name: "Venue items" }))).toEqual([
			"Four-Point Truss",
			"Four-Point Truss Corner 2-Way",
			"Stage Deck 2 × 1 m, Legs 0.4 m",
			"Disco Ball 50 cm",
		]);
		expect(names(screen.getByRole("region", { name: "3D models" }))).toEqual(["Hall"]);
	});

	it("shows each object on one compact row with its name, kind and size but no fixture ID", () => {
		renderObjects([
			venue("7", "Four-Point Truss", {
				scenery: { kind: "truss", chords: 4, pattern: "standard" },
				sizeMillimetres: [3000, 290, 290],
			}),
		]);
		const row = within(screen.getByRole("region", { name: "Venue items" })).getByRole(
			"button",
			{ name: /Four-Point Truss/ },
		);
		expect(row).toHaveClass("cad-elements-object");
		expect(row.querySelector("strong")).toHaveTextContent("Four-Point Truss");
		expect(row.querySelector(".cad-elements-object-kind")).toHaveTextContent("truss");
		expect(row.querySelector(".cad-elements-object-size")).toHaveTextContent(
			"3 × 0.29 × 0.29 m",
		);
		expect(row).not.toHaveTextContent("0.7");
		expect(row.textContent).not.toContain("·");
	});

	it("says there are no 3D models when only shipped Venue objects are placed", () => {
		renderObjects([venue("2", "Stage Railing 2 m")]);
		expect(
			within(screen.getByRole("region", { name: "3D models" })).getByText(
				"No 3D models placed yet.",
			),
		).toBeVisible();
	});
});

describe("Elements › Objects groups", () => {
	const entities = [venue("1", "Truss A"), venue("2", "Truss B"), venue("3", "Deck")];

	it("lists a group with its members; a member picks the whole group, Shift the member alone", async () => {
		tauri.stored = { groups: [{ id: "g1", name: "Upstage truss", memberIds: ["1", "2"] }] };
		const onSelect = renderObjects(entities);
		const group = await screen.findByRole("group", { name: "Upstage truss" });
		// A grouped element is listed in its group, not again among the loose Venue items.
		expect(names(screen.getByRole("region", { name: "Venue items" }))).toEqual(["Deck"]);

		fireEvent.click(within(group).getByRole("button", { name: /^Upstage truss/u }));
		expect(onSelect).toHaveBeenLastCalledWith(["1", "2"]);

		fireEvent.click(within(group).getByRole("button", { name: "Expand Upstage truss" }));
		const member = within(group).getByRole("button", { name: /Truss B/u });
		fireEvent.click(member);
		expect(onSelect).toHaveBeenLastCalledWith(["1", "2"]);
		fireEvent.click(member, { shiftKey: true });
		expect(onSelect).toHaveBeenLastCalledWith(["2"]);
	});

	it("groups the selected Venue elements and ungroups a selected group", async () => {
		renderObjects(entities, ["1", "3"]);
		const groupButton = screen.getByRole("button", { name: "Group" });
		expect(screen.getByRole("button", { name: "Ungroup" })).toBeDisabled();
		await waitFor(() => expect(groupButton).toBeEnabled());
		fireEvent.click(groupButton);
		await waitFor(() =>
			expect(tauri.invoke).toHaveBeenCalledWith("save_cad_venue_groups", {
				groups: {
					groups: [{ id: expect.any(String), name: "Group 1", memberIds: ["1", "3"] }],
				},
			}),
		);
		expect(await screen.findByRole("group", { name: "Group 1" })).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "Ungroup" }));
		await waitFor(() =>
			expect(tauri.invoke).toHaveBeenLastCalledWith("save_cad_venue_groups", {
				groups: { groups: [] },
			}),
		);
	});
});
