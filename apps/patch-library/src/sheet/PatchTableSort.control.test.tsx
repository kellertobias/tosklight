import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../wire";
import { FixturePatchSetup } from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const server = vi.hoisted(() => ({
	patch: { fixtures: [] as unknown[] },
	patchLayers: [],
	fixtureVisibility: new Map(),
	fixtureNotes: new Map(),
	fixtureProfiles: [],
	fixtureLibrary: [],
	unresolvedMvrFixtures: [],
	selectedFixtures: [],
	setSelection: vi.fn(),
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
	saveFixtureVisibility: vi.fn(),
	saveFixtureNote: vi.fn(),
}));
const selection = vi.hoisted(() => ({
	fixtureIds: new Set<string>(),
	orderedFixtureIds: [] as string[],
	replace: vi.fn(),
}));

vi.mock("../host", async (importOriginal) => ({
	...(await importOriginal<object>()),
	usePatchHost: () => ({
		library: server,
		selection,
		editArmed: false,
		desktopEditing: false,
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
		patchFixtures: vi.fn(),
		updateFixture: vi.fn(),
		updatePolicy: vi.fn(),
		deleteFixture: vi.fn(),
		deleteFixtures: vi.fn(),
	}),
}));

afterEach(cleanup);

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

describe("patch sheet column headers", () => {
	it("order the sheet by the clicked column and reverse it on a second click", () => {
		server.patch.fixtures = [
			wash("third", 3, 2, 1),
			wash("first", 1, null, null),
			wash("second", 2, 1, 10),
		];
		render(<FixturePatchSetup />);
		const fixtureId = screen.getByRole("columnheader", { name: "Fixture ID" });
		const patch = screen.getByRole("columnheader", { name: "Patch" });
		expect(rowOrder()).toEqual(["first", "second", "third"]);
		expect(fixtureId).toHaveAttribute("aria-sort", "ascending");
		expect(patch).toHaveAttribute("aria-sort", "none");

		fireEvent.click(screen.getByRole("button", { name: "Sort by Patch" }));
		expect(rowOrder()).toEqual(["second", "third", "first"]);
		expect(patch).toHaveAttribute("aria-sort", "ascending");
		expect(fixtureId).toHaveAttribute("aria-sort", "none");

		fireEvent.click(screen.getByRole("button", { name: "Sort by Patch" }));
		expect(rowOrder()).toEqual(["third", "second", "first"]);
		expect(patch).toHaveAttribute("aria-sort", "descending");

		fireEvent.click(screen.getByRole("button", { name: "Sort by Fixture ID" }));
		expect(rowOrder()).toEqual(["first", "second", "third"]);
		fireEvent.click(screen.getByRole("button", { name: "Sort by Fixture ID" }));
		expect(rowOrder()).toEqual(["third", "second", "first"]);
		expect(fixtureId).toHaveAttribute("aria-sort", "descending");
	});

	it("leaves columns that do not order the sheet as plain headers", () => {
		server.patch.fixtures = [wash("first", 1, 1, 1)];
		render(<FixturePatchSetup />);
		const header = screen.getByRole("columnheader", { name: "Group Masters" });
		expect(header).not.toHaveAttribute("aria-sort");
		expect(
			screen.queryByRole("button", { name: "Sort by Group Masters" }),
		).toBeNull();
	});
});
