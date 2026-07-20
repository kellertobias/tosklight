import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import {
	FixturePatchSetup,
	fixtureDisplayId,
	parseVirtualFixtureNumber,
	UniverseMap,
} from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const state = { patchSetArmed: false };
const dispatch = vi.fn();
const programming = vi.hoisted(() => ({
	ready: true,
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
const server = {
	patch: { fixtures: [] as PatchedFixture[] },
	patchLayers: [] as Array<{
		body: { id: string; name: string; order: number };
	}>,
	fixtureProfiles: [] as ReturnType<typeof blankFixtureProfile>[],
	fixtureLibrary: [],
	unresolvedMvrFixtures: [],
	selectedFixtures: [] as string[],
	setSelection: vi.fn(),
	updatePatchedFixture: vi.fn().mockResolvedValue(true),
	deletePatchedFixture: vi.fn().mockResolvedValue(true),
	patchFixture: vi.fn(),
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
};
const patchFeature = {
	patchFixtures: vi.fn(),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: (active = true) =>
			active && programming.ready ? programming.selection : null,
		useProgrammingSelectionActions: (active = true) =>
			active && programming.ready ? programming.actions : null,
	}),
);
vi.mock("../../features/patch/PatchContext", async (importOriginal) => {
	const actual =
		await importOriginal<typeof import("../../features/patch/PatchContext")>();
	return {
		...actual,
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
			patchFixtures: patchFeature.patchFixtures,
			updateFixture: server.updatePatchedFixture,
			deleteFixture: server.deletePatchedFixture,
		}),
	};
});
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch }),
}));

function splitFixture(): PatchedFixture {
	const profile = blankFixtureProfile();
	profile.id = "profile-split";
	profile.revision = 1;
	profile.manufacturer = "Acme";
	profile.name = "Split Wash";
	profile.short_name = "Split";
	profile.modes[0].id = "mode-split";
	profile.modes[0].splits = [
		{ number: 1, footprint: 4 },
		{ number: 3, footprint: 12 },
	];
	return {
		fixture_id: "fixture-split",
		fixture_number: 17,
		name: "Split Wash 17",
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
		universe: 1,
		address: 101,
		split_patches: [
			{ split: 1, universe: 1, address: 101 },
			{ split: 3, universe: 2, address: 201 },
		],
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

function openDimmerPlacement() {
	const profile = blankFixtureProfile();
	profile.manufacturer = "Generic";
	profile.name = "Dimmer";
	profile.short_name = "Dimmer";
	server.fixtureProfiles = [profile];
	render(<FixturePatchSetup />);
	fireEvent.click(screen.getByRole("button", { name: "+ Add fixture" }));
	fireEvent.click(screen.getByRole("button", { name: /^Add fixture$/ }));
	return screen
		.getByRole("heading", { name: "Patch Dimmer" })
		.closest("section") as HTMLElement;
}

function fixturesWithConflict() {
	const current = splitFixture();
	current.multipatch = [
		{
			id: "current-mp",
			name: "Current duplicate",
			universe: 6,
			address: 101,
			split_patches: [
				{ split: 1, universe: 6, address: 101 },
				{ split: 3, universe: 7, address: 201 },
			],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
		},
	];
	const blocked = splitFixture();
	blocked.fixture_id = "fixture-blocked";
	blocked.fixture_number = 18;
	blocked.name = "Blocked Wash 18";
	blocked.universe = 4;
	blocked.address = 401;
	blocked.split_patches = [
		{ split: 1, universe: 4, address: 401 },
		{ split: 3, universe: 5, address: 201 },
	];
	blocked.multipatch = [
		{
			id: "blocked-mp",
			name: "Blocked duplicate",
			universe: 8,
			address: 301,
			split_patches: [
				{ split: 1, universe: 8, address: 301 },
				{ split: 3, universe: 9, address: 401 },
			],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
		},
	];
	return { current, blocked };
}

async function requestConflictingSplitPatch() {
	fireEvent.click(screen.getByRole("button", { name: "Split 3 patch 2.201" }));
	const addressScreen = await screen.findByRole("dialog", {
		name: "Fixture Address",
	});
	fireEvent.click(
		within(addressScreen).getByRole("button", {
			name: "Clear address · Unpatch",
		}),
	);
	for (const key of ["4", "Universe separator", "4", "0", "1"])
		fireEvent.click(
			within(addressScreen).getByRole("button", {
				name: key === "Universe separator" ? key : `Address ${key}`,
			}),
		);
	fireEvent.click(
		within(addressScreen).getByRole("button", { name: "Set Address" }),
	);
	expect(
		await screen.findByRole("heading", { name: "Patch conflict" }),
	).toBeInTheDocument();
}

beforeEach(() => {
	state.patchSetArmed = false;
	server.patch.fixtures = [splitFixture()];
	server.fixtureProfiles = [];
	server.selectedFixtures = [];
	programming.ready = true;
	programming.selection.selected = [];
	vi.clearAllMocks();
	programming.actions.replace.mockResolvedValue(null);
	programming.actions.gesture.mockResolvedValue(null);
	server.updatePatchedFixture.mockResolvedValue(true);
	server.deletePatchedFixture.mockResolvedValue(true);
	server.patchFixture.mockResolvedValue("new-fixture");
	patchFeature.patchFixtures.mockImplementation(
		async (candidates: Array<{ fixture: PatchedFixture }>) =>
			candidates.map((candidate) => ({
				fixtureId: candidate.fixture.fixture_id,
				selectionFixtureIds: [candidate.fixture.fixture_id],
			})),
	);
});

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe("selected split selection and SET editing", () => {
	it("places Preview Stage before existing title actions and supports additive and range selection", () => {
		const second = splitFixture();
		second.fixture_id = "fixture-18";
		second.fixture_number = 18;
		second.name = "Split Wash 18";
		const third = splitFixture();
		third.fixture_id = "fixture-19";
		third.fixture_number = 19;
		third.name = "Split Wash 19";
		server.patch.fixtures = [splitFixture(), second, third];
		const onStagePreview = vi.fn();
		const rendered = render(
			<FixturePatchSetup
				onMedia={vi.fn()}
				stagePreviewOpen
				onStagePreview={onStagePreview}
			/>,
		);

		const actions = screen
			.getByText("Show Patch")
			.closest("header") as HTMLElement;
		expect(
			within(actions)
				.getAllByRole("button")
				.slice(0, 3)
				.map((button) => button.textContent),
		).toEqual(["Preview Stage", "Fixtures", "Media Servers"]);
		expect(screen.getByRole("button", { name: "Preview Stage" })).toHaveClass(
			"active",
		);
		fireEvent.click(screen.getByRole("button", { name: "Preview Stage" }));
		expect(onStagePreview).toHaveBeenCalledOnce();

		fireEvent.click(screen.getByRole("row", { name: /17 Split Wash 17/ }));
		expect(programming.actions.replace).toHaveBeenLastCalledWith({
			resolvedFixtures: ["fixture-split"],
		});
		programming.selection.selected = ["fixture-split"];
		rendered.rerender(
			<FixturePatchSetup
				onMedia={vi.fn()}
				stagePreviewOpen
				onStagePreview={onStagePreview}
			/>,
		);
		fireEvent.click(screen.getByRole("row", { name: /18 Split Wash 18/ }), {
			metaKey: true,
		});
		expect(programming.actions.replace).toHaveBeenLastCalledWith({
			resolvedFixtures: ["fixture-split", "fixture-18"],
		});
		fireEvent.click(screen.getByRole("row", { name: /19 Split Wash 19/ }), {
			shiftKey: true,
		});
		expect(programming.actions.replace).toHaveBeenLastCalledWith({
			resolvedFixtures: ["fixture-18", "fixture-19"],
		});
		expect(
			document.querySelector(".patch-stage-scroll-clearance"),
		).toBeInTheDocument();
	});

	it("uses the selected split for an armed touch, keyboard, or attached-hardware SET action", async () => {
		const { rerender } = render(<FixturePatchSetup />);
		fireEvent.click(
			screen.getByRole("button", { name: "Split 3 patch 2.201" }),
		);
		expect(programming.actions.replace).toHaveBeenCalledWith({
			resolvedFixtures: ["fixture-split"],
		});

		state.patchSetArmed = true;
		rerender(<FixturePatchSetup />);

		const addressScreen = await screen.findByRole("dialog", {
			name: "Fixture Address",
		});
		expect(
			within(addressScreen).getByText("Complete footprint").parentElement,
		).toHaveTextContent("16 slots");
		expect(
			within(addressScreen).getByRole("button", { name: /Split 3/ }),
		).toHaveClass("active");
		expect(within(addressScreen).getAllByRole("gridcell")).toHaveLength(512);
		fireEvent.click(
			within(addressScreen).getByRole("button", {
				name: "Clear address · Unpatch",
			}),
		);
		for (const key of ["4", "Universe separator", "4", "0", "1"])
			fireEvent.click(
				within(addressScreen).getByRole("button", {
					name: key === "Universe separator" ? key : `Address ${key}`,
				}),
			);
		expect(within(addressScreen).getAllByText("4.401")).not.toHaveLength(0);
		fireEvent.click(
			within(addressScreen).getByRole("button", { name: "Set Address" }),
		);

		await waitFor(() =>
			expect(server.updatePatchedFixture).toHaveBeenCalledWith(
				"fixture-split",
				{
					split_patches: [
						{ split: 1, universe: 1, address: 101 },
						{ split: 3, universe: 4, address: 401 },
					],
					universe: 1,
					address: 101,
				},
			),
		);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PATCH_ARMED",
			value: false,
		});
	});

	it("toggles every logical head through one typed replacement", () => {
		const fixture = splitFixture();
		fixture.logical_heads = [
			{ fixture_id: "head-left", head_index: 1 },
			{ fixture_id: "head-right", head_index: 2 },
		];
		server.patch.fixtures = [fixture];
		programming.selection.selected = ["head-left", "head-right"];
		render(<FixturePatchSetup />);
		const row = screen.getByRole("row", { name: /17 Split Wash 17/ });
		fireEvent.click(row);
		expect(programming.actions.replace).toHaveBeenLastCalledWith({
			resolvedFixtures: ["head-left", "head-right"],
		});
		programming.actions.replace.mockClear();

		fireEvent.click(row, { metaKey: true });

		expect(programming.actions.replace).toHaveBeenCalledWith({
			resolvedFixtures: [],
		});
	});

	it("does not treat legacy selection as scoped authority while loading", () => {
		server.selectedFixtures = ["fixture-split"];
		programming.ready = false;
		render(<FixturePatchSetup />);
		const row = screen.getByRole("row", { name: /17 Split Wash 17/ });

		expect(row).not.toHaveClass("selected");
		fireEvent.click(row, { metaKey: true });
		expect(programming.actions.gesture).not.toHaveBeenCalled();
		expect(programming.actions.replace).not.toHaveBeenCalled();
		expect(server.setSelection).not.toHaveBeenCalled();
	});
});

describe("selected split conflict validation", () => {
	it("excludes the fixture's own slots, rejects another fixture's full range, and cancels on Escape", async () => {
		const occupied = splitFixture();
		occupied.fixture_id = "fixture-other";
		occupied.fixture_number = 18;
		occupied.name = "Split Wash 18";
		occupied.universe = 4;
		occupied.address = 401;
		occupied.split_patches = [
			{ split: 1, universe: 4, address: 401 },
			{ split: 3, universe: 5, address: 201 },
		];
		server.patch.fixtures = [splitFixture(), occupied];

		const { rerender } = render(<FixturePatchSetup />);
		fireEvent.click(
			screen.getByRole("button", { name: "Split 3 patch 2.201" }),
		);
		state.patchSetArmed = true;
		rerender(<FixturePatchSetup />);

		const addressScreen = await screen.findByRole("dialog", {
			name: "Fixture Address",
		});
		const current = within(addressScreen).getByRole("gridcell", {
			name: /DMX address 201/,
		});
		expect(current).toHaveClass("proposed");
		expect(current).not.toHaveClass("used");

		fireEvent.click(
			within(addressScreen).getByRole("button", {
				name: "Clear address · Unpatch",
			}),
		);
		for (const key of ["4", "Universe separator", "4", "0", "1"])
			fireEvent.click(
				within(addressScreen).getByRole("button", {
					name: key === "Universe separator" ? key : `Address ${key}`,
				}),
			);
		expect(within(addressScreen).getByRole("alert")).toHaveTextContent(
			"complete Split 3 footprint is unavailable",
		);
		expect(
			within(addressScreen).getByRole("button", { name: "Set Address" }),
		).toBeEnabled();

		fireEvent.keyDown(window, { key: "Escape" });
		expect(
			screen.queryByRole("dialog", { name: "Fixture Address" }),
		).not.toBeInTheDocument();
		expect(server.updatePatchedFixture).not.toHaveBeenCalled();
	});
});

describe("fixture batch IDs and title actions", () => {
	it("shows a regular start-ID number field and skips occupied IDs for the complete batch", async () => {
		const occupied = splitFixture();
		occupied.fixture_id = "fixture-101";
		occupied.fixture_number = 101;
		occupied.universe = 2;
		occupied.address = 1;
		occupied.split_patches = [
			{ split: 1, universe: 2, address: 1 },
			{ split: 3, universe: 3, address: 1 },
		];
		server.patch.fixtures = [occupied];

		openDimmerPlacement();

		const startId = screen.getByRole("textbox", { name: "Start fixture ID" });
		expect(startId).toHaveAttribute("inputmode", "numeric");
		fireEvent.change(startId, { target: { value: "100" } });
		fireEvent.change(screen.getByRole("textbox", { name: "Count" }), {
			target: { value: "4" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Add 4 fixtures" }));

		await waitFor(() => expect(patchFeature.patchFixtures).toHaveBeenCalledOnce());
		expect(
			patchFeature.patchFixtures.mock.calls[0][0].map(
				(candidate: { fixture: PatchedFixture }) =>
					candidate.fixture.fixture_number,
			),
		).toEqual([100, 102, 103, 104]);
		expect(server.patchFixture).not.toHaveBeenCalled();
		expect(server.refresh).not.toHaveBeenCalled();
	});

	it("keeps Cancel and Add in the title bar and confirms closing changed placement data", () => {
		const placement = openDimmerPlacement();
		const header = placement.querySelector(":scope > header") as HTMLElement;
		expect(
			within(header)
				.getAllByRole("button")
				.map(
					(button) => button.getAttribute("aria-label") ?? button.textContent,
				),
		).toEqual(["Cancel", "Add 1 fixtures", "Close Add Fixture"]);

		fireEvent.change(
			within(placement).getByRole("textbox", { name: "Fixture name" }),
			{ target: { value: "Changed Dimmer" } },
		);
		fireEvent.click(
			within(header).getByRole("button", { name: "Close Add Fixture" }),
		);
		const confirmation = screen.getByRole("dialog", {
			name: "Close Add Fixture?",
		});
		expect(
			within(confirmation).getByRole("button", { name: "Yes, close" }),
		).toBeInTheDocument();
		fireEvent.click(
			within(confirmation).getByRole("button", { name: "Stay in Add Fixture" }),
		);
		expect(
			within(placement).getByRole("textbox", { name: "Fixture name" }),
		).toHaveValue("Changed Dimmer");

		fireEvent.click(within(header).getByRole("button", { name: "Cancel" }));
		fireEvent.click(
			within(
				screen.getByRole("dialog", { name: "Close Add Fixture?" }),
			).getByRole("button", { name: "Yes, close" }),
		);
		expect(
			screen.queryByRole("heading", { name: "Patch Dimmer" }),
		).not.toBeInTheDocument();
	});
});

describe("fixture batch DMX placement", () => {
	it("renders 512 hittable DMX squares and marks used, proposed, and conflicting ranges", () => {
		const placement = openDimmerPlacement();
		const grid = within(placement).getByRole("grid", {
			name: "DMX universe 1",
		});
		expect(within(grid).getAllByRole("gridcell")).toHaveLength(512);
		expect(grid.querySelector('[data-dmx-address="1"]')).toHaveClass(
			"proposed",
		);
		const occupied = grid.querySelector(
			'[data-dmx-address="101"]',
		) as HTMLElement;
		expect(occupied).toHaveClass("used");
		expect(occupied).toHaveAccessibleName(/used by Fixture 17 Split Wash 17/);

		fireEvent.click(occupied);
		expect(
			within(placement).getByRole("textbox", {
				name: "Address (universe.address)",
			}),
		).toHaveValue("1.101");
		expect(grid.querySelector('[data-dmx-address="101"]')).toHaveClass(
			"proposed",
			"conflict",
		);
	});

	it("patches dragged batch addresses and selects authoritative targets", async () => {
		const placement = openDimmerPlacement();
		fireEvent.change(
			within(placement).getByRole("textbox", { name: "Count" }),
			{ target: { value: "3" } },
		);
		const grid = within(placement).getByRole("grid", {
			name: "DMX universe 1",
		});
		expect(grid.querySelector('[data-dmx-address="1"]')).toHaveAccessibleName(
			/Fixture 1/,
		);
		expect(grid.querySelector('[data-dmx-address="2"]')).toHaveAccessibleName(
			/Fixture 2/,
		);
		expect(grid.querySelector('[data-dmx-address="3"]')).toHaveAccessibleName(
			/Fixture 3/,
		);

		const second = grid.querySelector('[data-dmx-address="2"]') as HTMLElement;
		const destination = grid.querySelector(
			'[data-dmx-address="50"]',
		) as HTMLElement;
		const originalElementFromPoint = document.elementFromPoint;
		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: vi.fn(() => destination),
		});
		try {
			fireEvent.pointerDown(second, { pointerId: 9, clientX: 10, clientY: 10 });
			fireEvent.pointerMove(grid, { pointerId: 9, clientX: 100, clientY: 100 });
			fireEvent.pointerUp(grid, { pointerId: 9 });
		} finally {
			Object.defineProperty(document, "elementFromPoint", {
				configurable: true,
				value: originalElementFromPoint,
			});
		}
		expect(grid.querySelector('[data-dmx-address="50"]')).toHaveAccessibleName(
			/Fixture 2/,
		);
		patchFeature.patchFixtures.mockImplementationOnce(
			async (candidates: Array<{ fixture: PatchedFixture }>) =>
				candidates.map((candidate, index) => ({
					fixtureId: candidate.fixture.fixture_id,
					selectionFixtureIds:
						index === candidates.length - 1
							? ["last-head-left", "last-head-right"]
							: [candidate.fixture.fixture_id],
				})),
		);

		fireEvent.click(
			within(placement).getByRole("button", { name: "Add 3 fixtures" }),
		);
		await waitFor(() => expect(patchFeature.patchFixtures).toHaveBeenCalledOnce());
		expect(
			patchFeature.patchFixtures.mock.calls[0][0].map(
				(candidate: { fixture: PatchedFixture }) =>
					candidate.fixture.address,
			),
		).toEqual([1, 50, 3]);
		expect(programming.actions.replace).toHaveBeenCalledWith({
			resolvedFixtures: ["last-head-left", "last-head-right"],
		});
	});
});

describe("fixture browser filtering", () => {
	it("filters the Add Fixture browser while typing and clears the active search", () => {
		const dimmer = blankFixtureProfile();
		dimmer.manufacturer = "Generic";
		dimmer.name = "Dimmer";
		dimmer.short_name = "Dimmer";
		const orbit = blankFixtureProfile();
		orbit.id = "orbit-profile";
		orbit.manufacturer = "Acme";
		orbit.name = "Orbit Wash";
		orbit.short_name = "Orbit";
		server.fixtureProfiles = [dimmer, orbit];

		render(<FixturePatchSetup />);
		fireEvent.click(screen.getByRole("button", { name: "+ Add fixture" }));
		fireEvent.change(screen.getByRole("textbox", { name: "Search" }), {
			target: { value: "orbit" },
		});

		expect(
			screen.getByRole("button", { name: /Orbit Wash/ }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /^Dimmer/ }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
		expect(screen.getByRole("button", { name: /^Dimmer/ })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Clear search" }),
		).not.toBeInTheDocument();
	});
});

describe("visual-only Venue placement", () => {
	it("adds an addressless object and never offers a DMX patch control", async () => {
		const venue = blankFixtureProfile();
		venue.manufacturer = "Venue";
		venue.name = "Four-Point Truss";
		venue.short_name = "Four-Point Truss";
		venue.fixture_type = "venue";
		venue.patch_policy = "visual_only";
		venue.model_units = "metres";
		venue.modes[0].name = "2 m";
		venue.modes[0].splits[0].footprint = 0;
		server.fixtureProfiles = [venue];
		server.patch.fixtures = [];

		render(<FixturePatchSetup />);
		fireEvent.click(screen.getByRole("button", { name: "+ Add fixture" }));
		fireEvent.click(screen.getByRole("button", { name: /^Add fixture$/ }));

		const placement = screen
			.getByRole("heading", { name: "Add Four-Point Truss" })
			.closest("section") as HTMLElement;
		expect(
			within(placement).getByText(
				"This Venue element is visual only and has no DMX patch.",
			),
		).toBeInTheDocument();
		expect(
			within(placement).getByRole("textbox", { name: "Start fixture ID" }),
		).toHaveValue("0.1");
		expect(
			within(placement).queryByRole("textbox", {
				name: "Address (universe.address)",
			}),
		).not.toBeInTheDocument();
		expect(within(placement).queryByRole("grid")).not.toBeInTheDocument();
		fireEvent.click(
			within(placement).getByRole("button", { name: "Add 1 fixtures" }),
		);

		await waitFor(() => expect(patchFeature.patchFixtures).toHaveBeenCalledOnce());
		expect(
			patchFeature.patchFixtures.mock.calls[0][0][0].fixture,
		).toEqual(
			expect.objectContaining({
				fixture_number: null,
				virtual_fixture_number: 1,
				universe: null,
				address: null,
				split_patches: [{ split: 1, universe: null, address: null }],
			}),
		);
		expect(server.setSelection).not.toHaveBeenCalled();
	});

	it("parses and displays IDs in the reserved 0.x namespace", () => {
		expect(parseVirtualFixtureNumber("0.1")).toBe(1);
		expect(parseVirtualFixtureNumber("0.24")).toBe(24);
		expect(parseVirtualFixtureNumber("0.0")).toBeNull();
		expect(parseVirtualFixtureNumber("1")).toBeNull();
		expect(
			fixtureDisplayId({ fixture_number: null, virtual_fixture_number: 7 }),
		).toBe("0.7");
	});
});

describe("DMX address grid dragging", () => {
	it("moves the proposed footprint with mouse or touch pointer events while preserving the grabbed offset", () => {
		const onAddress = vi.fn();
		render(
			<UniverseMap
				fixtures={[]}
				universe={1}
				proposed={10}
				footprint={4}
				proposedLabel="Fixture 10 · Test"
				onAddress={onAddress}
				onUniverse={vi.fn()}
			/>,
		);
		const grid = screen.getByRole("grid", { name: "DMX universe 1" });
		const grabbed = grid.querySelector(
			'[data-dmx-address="11"]',
		) as HTMLElement;
		const destination = grid.querySelector(
			'[data-dmx-address="50"]',
		) as HTMLElement;
		const originalElementFromPoint = document.elementFromPoint;
		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: vi.fn(() => destination),
		});
		try {
			fireEvent.pointerDown(grabbed, {
				pointerId: 7,
				clientX: 10,
				clientY: 10,
			});
			fireEvent.pointerMove(grid, { pointerId: 7, clientX: 100, clientY: 100 });
			expect(onAddress).toHaveBeenLastCalledWith(49);
			fireEvent.pointerUp(grid, { pointerId: 7 });
		} finally {
			Object.defineProperty(document, "elementFromPoint", {
				configurable: true,
				value: originalElementFromPoint,
			});
		}
	});

	it("identifies which proposed fixture moved in a multi-fixture batch", () => {
		const onProposalAddress = vi.fn();
		render(
			<UniverseMap
				fixtures={[]}
				universe={1}
				proposed={10}
				footprint={4}
				proposedLabel="Fixture 10"
				proposals={[
					{ key: "first", start: 10, footprint: 4, label: "Fixture 10" },
					{ key: "second", start: 20, footprint: 4, label: "Fixture 11" },
				]}
				onAddress={vi.fn()}
				onProposalAddress={onProposalAddress}
				onUniverse={vi.fn()}
			/>,
		);
		const grid = screen.getByRole("grid", { name: "DMX universe 1" });
		const grabbed = grid.querySelector(
			'[data-dmx-address="21"]',
		) as HTMLElement;
		const destination = grid.querySelector(
			'[data-dmx-address="60"]',
		) as HTMLElement;
		const originalElementFromPoint = document.elementFromPoint;
		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: vi.fn(() => destination),
		});
		try {
			fireEvent.pointerDown(grabbed, {
				pointerId: 8,
				clientX: 10,
				clientY: 10,
			});
			fireEvent.pointerMove(grid, { pointerId: 8, clientX: 100, clientY: 100 });
			expect(onProposalAddress).toHaveBeenLastCalledWith("second", 59);
		} finally {
			Object.defineProperty(document, "elementFromPoint", {
				configurable: true,
				value: originalElementFromPoint,
			});
		}
	});
});

describe("schema-v2 location and multi-patch editing", () => {
	it("keeps Set and Close in the location title bar and confirms discarding changed axes", async () => {
		const { current } = fixturesWithConflict();
		server.patch.fixtures = [current];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		fireEvent.click(within(fixtureRow.cells[9]).getByRole("button"));

		const modal = screen
			.getByRole("heading", { name: "Set fixture location" })
			.closest("section") as HTMLElement;
		const titleBar = modal.querySelector(".ui-modal-titlebar") as HTMLElement;
		expect(
			within(titleBar)
				.getAllByRole("button")
				.map(
					(button) => button.getAttribute("aria-label") ?? button.textContent,
				),
		).toEqual(["Set", "Cancel fixture location"]);
		fireEvent.change(within(modal).getByRole("textbox", { name: "X (m)" }), {
			target: { value: "1" },
		});
		fireEvent.click(
			within(titleBar).getByRole("button", { name: "Cancel fixture location" }),
		);
		const confirmation = screen.getByRole("dialog", {
			name: "Discard fixture changes?",
		});
		fireEvent.click(
			within(confirmation).getByRole("button", { name: "Keep editing" }),
		);
		fireEvent.click(within(titleBar).getByRole("button", { name: "Set" }));
		await waitFor(() =>
			expect(server.updatePatchedFixture).toHaveBeenCalledWith(
				"fixture-split",
				{ location: { x: 1000, y: 0, z: 0 } },
			),
		);
	});

	it("edits a multi-patch address through the shared universe grid", async () => {
		const { current } = fixturesWithConflict();
		server.patch.fixtures = [current];
		render(<FixturePatchSetup />);
		const multipatchRow = document.querySelector(
			".multipatch-row",
		) as HTMLElement;
		fireEvent.click(
			within(multipatchRow).getByRole("button", { name: /S1 6\.101/ }),
		);

		const addressScreen = screen.getByRole("dialog", {
			name: "Multi-patch Address",
		});
		expect(within(addressScreen).getAllByRole("gridcell")).toHaveLength(512);
		expect(
			within(addressScreen).getByRole("button", { name: "Set Address" }),
		).toBeInTheDocument();
		expect(
			within(addressScreen).getByRole("button", {
				name: "Cancel Multi-patch Address",
			}),
		).toBeInTheDocument();
		fireEvent.click(
			within(addressScreen).getByRole("gridcell", { name: /^DMX address 301/ }),
		);
		fireEvent.click(
			within(addressScreen).getByRole("button", { name: "Set Address" }),
		);

		await waitFor(() =>
			expect(server.updatePatchedFixture).toHaveBeenCalledWith(
				"fixture-split",
				expect.objectContaining({
					multipatch: [
						expect.objectContaining({
							id: "current-mp",
							universe: 6,
							address: 301,
							split_patches: [
								{ split: 1, universe: 6, address: 301 },
								{ split: 3, universe: 7, address: 201 },
							],
						}),
					],
				}),
			),
		);
	});
});

describe("schema-v2 delete and unpatch controls", () => {
	it("uses toolbar Delete plus a fixture line to choose delete, unpatch, or abort", async () => {
		const { current } = fixturesWithConflict();
		server.patch.fixtures = [current];
		render(<FixturePatchSetup />);

		expect(
			screen.queryByRole("button", { name: "Remove multi-patch" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Delete" }));
		fireEvent.click(screen.getByRole("row", { name: /17 Split Wash 17/ }));

		const dialog = await screen.findByRole("alertdialog", {
			name: "Delete or unpatch Split Wash 17?",
		});
		expect(
			within(dialog)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["Delete fixture", "Unpatch fixture", "Abort"]);

		fireEvent.click(
			within(dialog).getByRole("button", { name: "Unpatch fixture" }),
		);
		await waitFor(() =>
			expect(server.updatePatchedFixture).toHaveBeenCalledWith(
				"fixture-split",
				expect.objectContaining({
					universe: null,
					address: null,
					multipatch: [
						expect.objectContaining({
							id: "current-mp",
							universe: null,
							address: null,
							split_patches: [
								{ split: 1, universe: null, address: null },
								{ split: 3, universe: null, address: null },
							],
						}),
					],
				}),
			),
		);
		expect(server.deletePatchedFixture).not.toHaveBeenCalled();
	});

	it("opens delete confirmation for the selected fixture and confirms with Enter", async () => {
		server.patch.fixtures = [splitFixture()];
		render(<FixturePatchSetup />);
		fireEvent.click(screen.getByRole("row", { name: /17 Split Wash 17/ }));

		fireEvent.keyDown(window, { key: "Delete" });
		expect(
			await screen.findByRole("alertdialog", {
				name: "Delete or unpatch Split Wash 17?",
			}),
		).toBeInTheDocument();

		fireEvent.keyDown(window, { key: "Enter" });
		await waitFor(() =>
			expect(server.deletePatchedFixture).toHaveBeenCalledWith("fixture-split"),
		);
		expect(server.updatePatchedFixture).not.toHaveBeenCalled();
	});
});

describe("schema-v2 current-fixture conflict resolution", () => {
	it("unpatches every split and multi-patch range on the current fixture", async () => {
		const { current, blocked } = fixturesWithConflict();
		server.patch.fixtures = [current, blocked];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		await requestConflictingSplitPatch();

		fireEvent.click(
			screen.getByRole("button", { name: "Unpatch current fixture" }),
		);
		await waitFor(() =>
			expect(server.updatePatchedFixture).toHaveBeenCalledWith(
				"fixture-split",
				{
					universe: null,
					address: null,
					split_patches: [
						{ split: 1, universe: null, address: null },
						{ split: 3, universe: null, address: null },
					],
					multipatch: [
						{
							id: "current-mp",
							name: "Current duplicate",
							universe: null,
							address: null,
							split_patches: [
								{ split: 1, universe: null, address: null },
								{ split: 3, universe: null, address: null },
							],
							location: { x: 0, y: 0, z: 0 },
							rotation: { x: 0, y: 0, z: 0 },
						},
					],
				},
			),
		);
	});
});

describe("schema-v2 all-conflict resolution", () => {
	it("unpatches every conflict and applies the requested split atomically", async () => {
		const { current, blocked } = fixturesWithConflict();
		server.patch.fixtures = [current, blocked];
		state.patchSetArmed = true;
		const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
		render(<FixturePatchSetup />);
		await requestConflictingSplitPatch();

		fireEvent.click(
			screen.getByRole("button", { name: "Unpatch conflicts and apply" }),
		);
		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		const candidates = patchFeature.patchFixtures.mock.calls[0][0] as Array<{
			fixture: PatchedFixture;
		}>;
		expect(candidates).toHaveLength(2);
		expect(candidates[0].fixture).toMatchObject({
			fixture_id: "fixture-blocked",
			universe: null,
			address: null,
			split_patches: [
				{ split: 1, universe: null, address: null },
				{ split: 3, universe: null, address: null },
			],
			multipatch: [
				{
					id: "blocked-mp",
					name: "Blocked duplicate",
					universe: null,
					address: null,
					split_patches: [
						{ split: 1, universe: null, address: null },
						{ split: 3, universe: null, address: null },
					],
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
				},
			],
		});
		expect(candidates[1].fixture).toMatchObject({
			fixture_id: "fixture-split",
			split_patches: [
				{ split: 1, universe: 1, address: 101 },
				{ split: 3, universe: 4, address: 401 },
			],
			universe: 1,
			address: 101,
		});
		expect(server.updatePatchedFixture).not.toHaveBeenCalled();
		expect(confirm).toHaveBeenCalledOnce();
	});
});
