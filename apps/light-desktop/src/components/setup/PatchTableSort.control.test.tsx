import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { FixturePatchSetup } from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const state = { patchSetArmed: false };
const server = vi.hoisted(() => ({
	patch: { fixtures: [] as unknown[] },
	patchLayers: [],
	fixtureProfiles: [],
	fixtureLibrary: [],
	unresolvedMvrFixtures: [],
	selectedFixtures: [],
	setSelection: vi.fn(),
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
	gelCatalogs: vi.fn().mockResolvedValue([]),
	previewGelCatalogCsvImport: vi.fn(),
	confirmGelCatalogCsvImport: vi.fn(),
}));
const programming = vi.hoisted(() => ({
	selection: {
		selected: [] as string[],
		expression: null,
		revision: 1,
		gestureOpen: false,
	},
	actions: {
		replace: vi.fn(),
		gesture: vi.fn(),
		selectGroup: vi.fn(),
		applyRule: vi.fn(),
	},
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock(
	"../../features/fixtureLibrary/FixtureLibraryContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useFixtureLibrary: () => server,
	}),
);
vi.mock("../../features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: { children: ReactNode }) => children,
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: (active = true) =>
			active ? programming.selection : null,
		useProgrammingSelectionActions: (active = true) =>
			active ? programming.actions : null,
	}),
);
vi.mock("../../features/patch/PatchContext", async (importOriginal) => ({
	...(await importOriginal<typeof import("../../features/patch/PatchContext")>()),
	PatchViewProvider: ({ children }: { children: ReactNode }) => children,
	usePatch: () => ({
		status: "ready",
		showId: "show",
		showRevision: 1,
		patchRevision: 1,
		cursor: 1,
		fixtures: server.patch.fixtures,
		selectedPatchInstance: null,
		selectPatchInstance: vi.fn(),
		pendingFixtureIds: new Set<string>(),
		error: null,
		patchFixtures: vi.fn(),
		spreadFixtureVector: vi.fn(),
		updateFixture: vi.fn(),
		updatePolicy: vi.fn(),
		updateFixtureIntent: vi.fn(),
		deleteFixture: vi.fn(),
	}),
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch: vi.fn() }),
}));
vi.mock("../../features/stageLayout/StageLayoutState", () => ({
	useStagePositions3d: () => ({}),
}));
vi.mock("../../features/stageLayout/StageLayoutActions", () => ({
	useStageLayoutActions: () => ({
		canWrite: true,
		regenerate2d: vi.fn(),
		setCrowdFootprint: vi.fn(),
	}),
}));

afterEach(cleanup);

function fixture(
	id: string,
	fixtureNumber: number,
	product: string,
	universe: number | null,
	address: number | null,
): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = `profile-${id}`;
	profile.manufacturer = "Acme";
	profile.name = product;
	profile.short_name = product;
	profile.modes[0].splits = [{ number: 1, footprint: 4 }];
	return {
		fixture_id: id,
		fixture_number: fixtureNumber,
		name: `${product} ${fixtureNumber}`,
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
	} as PatchedFixture;
}

const rowOrder = () =>
	[
		...document.querySelectorAll<HTMLElement>("tbody tr[data-fixture-id]"),
	].map((row) => row.dataset.fixtureId);

describe("Show Patch column headers", () => {
	it("order the table by the clicked column and reverse it on a second click", () => {
		server.patch.fixtures = [
			fixture("third", 3, "Beam", 2, 1),
			fixture("first", 1, "Wash", null, null),
			fixture("second", 2, "Spot", 1, 10),
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

		fireEvent.click(
			screen.getByRole("button", { name: "Sort by Fixture / mode" }),
		);
		expect(rowOrder()).toEqual(["third", "second", "first"]);

		fireEvent.click(screen.getByRole("button", { name: "Sort by Fixture ID" }));
		expect(rowOrder()).toEqual(["first", "second", "third"]);
		fireEvent.click(screen.getByRole("button", { name: "Sort by Fixture ID" }));
		expect(rowOrder()).toEqual(["third", "second", "first"]);
		expect(fixtureId).toHaveAttribute("aria-sort", "descending");
	});

	it("leaves columns that do not order the table as plain headers", () => {
		server.patch.fixtures = [fixture("first", 1, "Wash", 1, 1)];
		render(<FixturePatchSetup />);
		const header = screen.getByRole("columnheader", { name: "Masters" });
		expect(header).not.toHaveAttribute("aria-sort");
		expect(screen.queryByRole("button", { name: "Sort by Masters" })).toBeNull();
	});
});
