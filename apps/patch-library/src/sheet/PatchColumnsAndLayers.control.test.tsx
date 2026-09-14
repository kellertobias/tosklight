import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../wire";
import { FixturePatchSetup } from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const server = vi.hoisted(() => ({
	patch: { fixtures: [] as unknown[] },
	patchLayers: [] as Array<{ body: { id: string; name: string; order: number } }>,
	fixtureVisibility: new Map(),
	fixtureNotes: new Map(),
	fixtureProfiles: [],
	fixtureLibrary: [],
	unresolvedMvrFixtures: [],
	selectedFixtures: [],
	setSelection: vi.fn(),
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
	deletePatchLayer: vi.fn() as ((layerId: string) => Promise<boolean>) | undefined,
	saveFixtureVisibility: vi.fn(),
	saveFixtureNote: vi.fn(),
}));
const selection = vi.hoisted(() => ({
	fixtureIds: new Set<string>(),
	orderedFixtureIds: [] as string[],
	replace: vi.fn(),
}));

const patchFixtures = vi.hoisted(() => vi.fn());

vi.mock("../host", async (importOriginal) => ({
	...(await importOriginal<object>()),
	usePatchHost: () => ({
		library: server,
		selection,
		editArmed: true,
		desktopEditing: true,
		setEditArmed: vi.fn(),
	}),
}));
vi.mock("../state/PatchContext", async (importOriginal) => ({
	...(await importOriginal<typeof import("../state/PatchContext")>()),
	PatchViewProvider: ({ children }: { children: ReactNode }) => children,
	usePatch: () => ({
		status: "ready",
		showId: "show",
		showRevision: 1,
		patchRevision: 1,
		cursor: 1,
		fixtures: server.patch.fixtures,
		pendingFixtureIds: new Set<string>(),
		error: null,
		patchFixtures: patchFixtures,
		updateFixture: vi.fn(),
		updatePolicy: vi.fn(),
		deleteFixture: vi.fn(),
		deleteFixtures: vi.fn(),
	}),
}));

afterEach(cleanup);
beforeEach(() => {
	// This environment's own storage is incomplete, so each test gets a fresh, whole one.
	const stored = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => stored.get(key) ?? null,
		setItem: (key: string, value: string) => stored.set(key, value),
		removeItem: (key: string) => stored.delete(key),
		clear: () => stored.clear(),
	});
	server.patchLayers = [];
});

function wash(
	id: string,
	fixtureNumber: number,
	universe: number | null,
	address: number | null,
): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = `profile-${id}`;
	profile.manufacturer = "Acme";
	profile.name = "Wash";
	profile.short_name = "Wash";
	profile.modes[0].splits = [{ number: 1, footprint: 4 }];
	return {
		fixture_id: id,
		fixture_number: fixtureNumber,
		name: `Wash ${fixtureNumber}`,
		definition: {
			schema_version: 2,
			id: profile.id,
			revision: 1,
			manufacturer: profile.manufacturer,
			device_type: "wash",
			name: profile.name,
			model: profile.short_name,
			mode: "Default",
			footprint: 4,
			heads: [],
			color_calibration: null,
			physical: {},
			model_asset: null,
			icon_asset: null,
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			profile_id: profile.id,
			mode_id: profile.modes[0].id,
			profile_snapshot: profile,
		},
		universe,
		address,
		split_patches: [],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		multipatch: [],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
}

const rowOrder = () =>
	[
		...document.querySelectorAll<HTMLElement>("tbody tr[data-fixture-id]"),
	].map((row) => row.dataset.fixtureId);


function venue(id: string, fixtureNumber: number, layerId: string) {
	const fixture = wash(id, fixtureNumber, null, null);
	fixture.name = `Truss ${fixtureNumber}`;
	fixture.layer_id = layerId;
	fixture.definition.device_type = "venue";
	if (fixture.definition.profile_snapshot)
		fixture.definition.profile_snapshot.patch_policy = "visual_only";
	return fixture;
}

const headers = () =>
	screen.getAllByRole("columnheader").map((header) => header.textContent?.replace(/[▲▼]/gu, ""));

const layersSidebar = () => {
	const aside = screen.getByRole("heading", { name: "Layers" }).closest("aside");
	if (!aside) throw new Error("Layers sidebar was not rendered");
	return within(aside);
};

describe("patch sheet columns", () => {
	it("hides a column from the window settings and remembers it under the storage key", () => {
		server.patch.fixtures = [wash("first", 1, 1, 1)];
		render(<FixturePatchSetup columnStorageKey="test.columns" />);
		expect(headers()).toContain("Manufacturer");

		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		const settings = screen.getByRole("dialog", { name: "Show Patch" });
		fireEvent.click(within(settings).getByRole("switch", { name: "Manufacturer" }));
		expect(headers()).not.toContain("Manufacturer");
		expect(screen.getAllByRole("cell")).toHaveLength(headers().length);
		cleanup();

		render(<FixturePatchSetup columnStorageKey="test.columns" />);
		expect(headers()).not.toContain("Manufacturer");
	});

	it("offers the quick views only where the host asks for them", () => {
		server.patch.fixtures = [wash("first", 1, 1, 1)];
		render(<FixturePatchSetup />);
		expect(screen.queryByRole("button", { name: "Compact" })).toBeNull();
		cleanup();

		render(<FixturePatchSetup quickViews />);
		fireEvent.click(screen.getByRole("button", { name: "Compact" }));
		expect(headers()).toEqual(["Fixture ID", "Name", "Patch", "Layer", "Note"]);
		expect(screen.getAllByRole("cell")).toHaveLength(5);
		expect(screen.getByRole("button", { name: "Compact" })).toHaveClass("is-active");

		fireEvent.click(screen.getByRole("button", { name: "Visualization" }));
		expect(headers()).toContain("Bracket");
		expect(headers()).not.toContain("Patch");
		expect(screen.getByRole("button", { name: "Compact" })).not.toHaveClass("is-active");
	});
});

describe("patch sheet layers", () => {
	it("lists fixtures without a layer of their own under No Layer Assigned", () => {
		server.patchLayers = [
			{ body: { id: "default", name: "Default", order: 0 } },
			{ body: { id: "front", name: "Front", order: 1 } },
		];
		const orphan = wash("orphan", 3, 1, 20);
		orphan.layer_id = "deleted-layer";
		const front = wash("front", 2, 1, 10);
		front.layer_id = "front";
		server.patch.fixtures = [wash("default", 1, 1, 1), front, orphan];
		render(<FixturePatchSetup scope="dmx" />);

		fireEvent.click(layersSidebar().getByRole("button", { name: /^No Layer Assigned/ }));
		expect(rowOrder()).toEqual(["default", "orphan"]);
	});

	it("hides layers holding only fixtures this screen does not show, even after an old reveal", () => {
		server.patchLayers = [
			{ body: { id: "default", name: "Default", order: 0 } },
			{ body: { id: "trusses", name: "Trusses", order: 1 } },
		];
		server.patch.fixtures = [wash("light", 1, 1, 1), venue("truss", 2, "trusses")];
		// The Architect remounts the sheet with the request an earlier CAD selection left behind.
		render(<FixturePatchSetup scope="dmx" showAllLayersRequest={4} />);
		expect(layersSidebar().queryByRole("button", { name: /^Trusses/ })).toBeNull();

		fireEvent.click(screen.getByRole("switch", { name: "Show all layers" }));
		expect(layersSidebar().getByRole("button", { name: /^Trusses/ })).toBeInTheDocument();
	});

	it("hides No Layer Assigned when this screen has nothing without a layer", () => {
		server.patchLayers = [{ body: { id: "trusses", name: "Trusses", order: 1 } }];
		server.patch.fixtures = [venue("truss", 2, "trusses"), venue("loose", 3, "")];
		render(<FixturePatchSetup scope="venue" />);
		expect(layersSidebar().getByRole("button", { name: /^No Layer Assigned/ })).toBeInTheDocument();
		cleanup();

		server.patch.fixtures = [venue("truss", 2, "trusses")];
		render(<FixturePatchSetup scope="venue" />);
		expect(layersSidebar().queryByRole("button", { name: /^No Layer Assigned/ })).toBeNull();
	});
});

describe("deleting a layer", () => {
	beforeEach(() => {
		server.deletePatchLayer = vi.fn().mockResolvedValue(true);
		patchFixtures.mockReset().mockResolvedValue([]);
	});

	it("offers a bin beside every stored layer but the default one", () => {
		server.patchLayers = [
			{ body: { id: "default", name: "Default", order: 0 } },
			{ body: { id: "front", name: "Front", order: 1 } },
		];
		server.patch.fixtures = [wash("light", 1, 1, 1)];
		render(<FixturePatchSetup showAllLayersRequest={0} />);
		fireEvent.click(screen.getByRole("switch", { name: "Show all layers" }));
		expect(
			layersSidebar().getByRole("button", { name: "Delete layer Front" }),
		).toBeInTheDocument();
		expect(
			layersSidebar().queryByRole("button", { name: "Delete layer Default" }),
		).toBeNull();
	});

	it("offers no bin where the host deletes layers another way, as a desk does", () => {
		server.deletePatchLayer = undefined;
		server.patchLayers = [{ body: { id: "front", name: "Front", order: 1 } }];
		const front = wash("front", 2, 1, 10);
		front.layer_id = "front";
		server.patch.fixtures = [front];
		render(<FixturePatchSetup />);
		expect(
			layersSidebar().queryByRole("button", { name: "Delete layer Front" }),
		).toBeNull();
	});

	it("moves the layer's fixtures to the default layer, then deletes it, after confirming", async () => {
		server.patchLayers = [{ body: { id: "front", name: "Front", order: 1 } }];
		const front = wash("front", 2, 1, 10);
		front.layer_id = "front";
		server.patch.fixtures = [wash("light", 1, 1, 1), front];
		render(<FixturePatchSetup />);

		fireEvent.click(
			layersSidebar().getByRole("button", { name: "Delete layer Front" }),
		);
		const confirm = screen.getByRole("alertdialog", {
			name: "Delete layer Front?",
		});
		expect(confirm).toHaveTextContent(
			"Its 1 fixture stay in the show and move to No Layer Assigned.",
		);
		expect(server.deletePatchLayer).not.toHaveBeenCalled();
		fireEvent.click(within(confirm).getByRole("button", { name: "Delete layer" }));

		await vi.waitFor(() =>
			expect(server.deletePatchLayer).toHaveBeenCalledWith("front"),
		);
		expect(patchFixtures).toHaveBeenCalledOnce();
		expect(patchFixtures.mock.calls[0][0]).toEqual([
			expect.objectContaining({
				fixture: expect.objectContaining({ fixture_id: "front", layer_id: "default" }),
			}),
		]);
		expect(patchFixtures.mock.invocationCallOrder[0]).toBeLessThan(
			(server.deletePatchLayer as ReturnType<typeof vi.fn>).mock.invocationCallOrder[0],
		);
		await vi.waitFor(() =>
			expect(screen.queryByRole("alertdialog", { name: "Delete layer Front?" })).toBeNull(),
		);
	});
});
