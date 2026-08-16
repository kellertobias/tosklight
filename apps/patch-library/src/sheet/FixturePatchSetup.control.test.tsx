import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { type ReactNode, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MultiPatchInstance, PatchedFixture } from "../wire";
import {
	FixtureAddFlow,
	FixturePatchSetup,
	fixtureDisplayId,
	parseVirtualFixtureNumber,
	UniverseMap,
} from "./FixturePatchSetup";
import { dmxGridColumnCount } from "./fixturePatch/UniverseMap";
import { blankFixtureProfile } from "./fixtureProfileModel";

const state = { patchSetArmed: false, desktopEditing: false };
const setEditArmed = vi.fn();
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
		body: {
			id: string;
			name: string;
			order: number;
			locked?: boolean;
			visible2d?: boolean;
			visible3d?: boolean;
		};
	}>,
	fixtureVisibility: new Map(),
	fixtureNotes: new Map(),
	fixtureProfiles: [] as ReturnType<typeof blankFixtureProfile>[],
	fixtureLibrary: [],
	unresolvedMvrFixtures: [],
	selectedFixtures: [] as string[],
	setSelection: vi.fn(),
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
	saveFixtureVisibility: vi.fn(),
	saveFixtureNote: vi.fn(),
};
const patchFeature = {
	patchFixtures: vi.fn(),
	updateFixture: vi.fn().mockResolvedValue(true),
	updatePolicy: vi.fn().mockResolvedValue(true),
	deleteFixture: vi.fn().mockResolvedValue(true),
};

vi.mock("../host", async (importOriginal) => ({
	...(await importOriginal<object>()),
	usePatchHost: () => ({
		library: server,
		selection: {
			fixtureIds: programming.ready
				? new Set(programming.selection.selected)
				: null,
			orderedFixtureIds: programming.ready
				? programming.selection.selected
				: null,
			replace: programming.actions.replace,
		},
		editArmed: state.patchSetArmed,
		desktopEditing: state.desktopEditing,
		setEditArmed,
	}),
}));
vi.mock("../state/PatchContext", async (importOriginal) => {
	const actual = await importOriginal<typeof import("../state/PatchContext")>();
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
			updateFixture: patchFeature.updateFixture,
			updatePolicy: patchFeature.updatePolicy,
			deleteFixture: patchFeature.deleteFixture,
		}),
	};
});

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

function policyFixture(): PatchedFixture {
	const fixture = splitFixture();
	const mode = fixture.definition.profile_snapshot?.modes[0];
	if (!mode) throw new Error("policy fixture mode is missing");
	const base = mode.channels[0];
	mode.channels = [
		{
			...base,
			id: "intensity-channel",
			fixture_attribute: "intensity",
			attribute: "intensity",
			reacts_to_group_master: true,
			reacts_to_grand_master: true,
		},
		{
			...base,
			id: "pan-channel",
			fixture_attribute: "pan",
			attribute: "pan",
			reacts_to_group_master: false,
			reacts_to_grand_master: false,
		},
		{
			...base,
			id: "tilt-channel",
			fixture_attribute: "tilt",
			attribute: "tilt",
			reacts_to_group_master: false,
			reacts_to_grand_master: false,
		},
	];
	fixture.group_masters_enabled = true;
	fixture.grand_master_enabled = true;
	fixture.invert_pan = false;
	fixture.invert_tilt = false;
	fixture.multipatch = [
		{
			id: "physical-copy",
			name: "Opposite hang",
			universe: 3,
			address: 1,
			split_patches: [
				{ split: 1, universe: 3, address: 1 },
				{ split: 3, universe: 4, address: 1 },
			],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			invert_pan: false,
			invert_tilt: true,
		},
	];
	return fixture;
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
	state.desktopEditing = false;
	server.patch.fixtures = [splitFixture()];
	server.patchLayers = [];
	server.fixtureProfiles = [];
	server.patchLayers = [];
	server.fixtureNotes = new Map();
	server.selectedFixtures = [];
	programming.ready = true;
	programming.selection.selected = [];
	vi.clearAllMocks();
	programming.actions.replace.mockResolvedValue(null);
	programming.actions.gesture.mockResolvedValue(null);
	patchFeature.updateFixture.mockResolvedValue(true);
	patchFeature.updatePolicy.mockResolvedValue(true);
	patchFeature.deleteFixture.mockResolvedValue(true);
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

describe("patch layer locks", () => {
	it("returns to All fixtures when an external selection requests a reveal", () => {
		const floorFixture = splitFixture();
		floorFixture.fixture_id = "fixture-floor";
		floorFixture.fixture_number = 18;
		floorFixture.name = "Floor Wash 18";
		floorFixture.layer_id = "floor";
		server.patch.fixtures = [splitFixture(), floorFixture];
		server.patchLayers = [
			{ body: { id: "default", name: "Stage", order: 0, locked: false } },
			{ body: { id: "floor", name: "Floor", order: 1, locked: false } },
		];
		const { rerender } = render(<FixturePatchSetup showAllLayersRequest={0} />);
		const layers = screen
			.getByRole("heading", { name: "Layers" })
			.closest("aside");
		if (!layers) throw new Error("Layers sidebar was not rendered");

		fireEvent.click(within(layers).getByRole("button", { name: /^Stage/ }));
		expect(
			screen.getByRole("row", { name: /Split Wash 17/ }),
		).toBeInTheDocument();
		expect(screen.queryByRole("row", { name: /Floor Wash 18/ })).toBeNull();

		rerender(<FixturePatchSetup showAllLayersRequest={1} />);
		expect(
			screen.getByRole("row", { name: /Floor Wash 18/ }),
		).toBeInTheDocument();
		expect(
			within(layers).getByRole("button", { name: /^All fixtures/ }),
		).toHaveClass("active");
	});

	it("offers Lock Layer in the title only after selecting an unlocked layer", async () => {
		server.patchLayers = [
			{ body: { id: "default", name: "Stage", order: 0, locked: false } },
		];
		server.savePatchLayer.mockResolvedValue(true);
		render(<FixturePatchSetup />);

		expect(
			screen.queryByRole("button", { name: "Lock Layer" }),
		).not.toBeInTheDocument();
		const layers = screen
			.getByRole("heading", { name: "Layers" })
			.closest("aside");
		if (!layers) throw new Error("Layers sidebar was not rendered");
		fireEvent.click(within(layers).getByRole("button", { name: /^Stage/ }));
		fireEvent.click(screen.getByRole("button", { name: "Lock Layer" }));
		await waitFor(() => expect(server.savePatchLayer).toHaveBeenCalledOnce());
		expect(server.savePatchLayer).toHaveBeenCalledWith(
			expect.objectContaining({ id: "default", locked: true }),
		);
	});

	it("persists the lock and prevents selecting fixtures in that layer", async () => {
		server.patchLayers = [
			{ body: { id: "default", name: "Stage", order: 0, locked: true } },
		];
		server.savePatchLayer.mockResolvedValue(true);
		render(<FixturePatchSetup />);

		const row = screen.getByRole("row", { name: /Split Wash 17/ });
		expect(row).toHaveAttribute("aria-disabled", "true");
		expect(screen.getByText("Layer Locked")).toBeInTheDocument();
		expect(screen.queryByText("🔒")).not.toBeInTheDocument();
		expect(screen.queryByText("🔓")).not.toBeInTheDocument();
		fireEvent.click(row);
		expect(programming.actions.replace).not.toHaveBeenCalled();

		expect(
			screen.queryByRole("button", { name: "Unlock Layer" }),
		).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: /Stage.*Layer Locked/ }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Unlock Layer" }));
		await waitFor(() => expect(server.savePatchLayer).toHaveBeenCalledOnce());
		expect(server.savePatchLayer).toHaveBeenCalledWith(
			expect.objectContaining({ id: "default", locked: false }),
		);
	});

	it("uses table eyes and title actions for independent 2D and 3D visibility", async () => {
		server.patchLayers = [{ body: { id: "default", name: "Stage", order: 0 } }];
		server.savePatchLayer.mockResolvedValue(true);
		server.saveFixtureVisibility.mockResolvedValue(true);
		render(<FixturePatchSetup />);

		fireEvent.click(
			screen.getByRole("button", { name: "Hide fixture 17 in 2D" }),
		);
		expect(server.saveFixtureVisibility).toHaveBeenCalledWith({
			fixtureId: "fixture-split",
			visible2d: false,
			visible3d: true,
		});

		const layers = screen
			.getByRole("heading", { name: "Layers" })
			.closest("aside");
		if (!layers) throw new Error("Layers sidebar was not rendered");
		fireEvent.click(within(layers).getByRole("button", { name: /^Stage/ }));
		fireEvent.click(screen.getByRole("button", { name: "Hide in 3D" }));
		await waitFor(() => expect(server.savePatchLayer).toHaveBeenCalled());
		expect(server.savePatchLayer).toHaveBeenLastCalledWith(
			expect.objectContaining({ id: "default", visible3d: false }),
		);
	});
});

describe("fixture notes", () => {
	it("shows and persists the canonical fixture note from the Patch table", async () => {
		state.patchSetArmed = true;
		server.fixtureNotes = new Map([
			[
				"fixture-split",
				{ fixtureId: "fixture-split", note: "Original rigging note" },
			],
		]);
		server.saveFixtureNote.mockResolvedValue(true);
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: "Note 17" }));
		const input = screen.getByRole("textbox", { name: "Fixture note" });
		expect(input).toHaveValue("Original rigging note");
		fireEvent.change(input, { target: { value: "Add secondary safety" } });
		fireEvent.click(screen.getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(server.saveFixtureNote).toHaveBeenCalledWith({
				fixtureId: "fixture-split",
				note: "Add secondary safety",
			}),
		);
	});
});

describe("fixture output policy cells", () => {
	it("does not expose per-fixture raw Highlight Look editing", () => {
		server.patch.fixtures = [policyFixture()];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		expect(
			screen.queryByRole("columnheader", { name: "Highlight Look" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Highlight Look 17" }),
		).not.toBeInTheDocument();
	});

	it("shows independent effective values and only edits a master through armed SET", async () => {
		server.patch.fixtures = [policyFixture()];
		const { rerender } = render(<FixturePatchSetup />);

		expect(
			screen.getByRole("button", { name: "Group Masters 17" }),
		).toHaveTextContent("Controlled");
		expect(
			screen.getByRole("button", { name: "Grand Master 17" }),
		).toHaveTextContent("Controlled");
		expect(
			screen.getByRole("button", { name: "Invert Pan 17" }),
		).toHaveTextContent("Normal");
		expect(
			screen.getByRole("button", { name: "Invert Tilt 17" }),
		).toHaveTextContent("Normal");

		fireEvent.click(screen.getByRole("button", { name: "Group Masters 17" }));
		expect(
			screen.queryByRole("heading", { name: "Set fixture Group Masters" }),
		).not.toBeInTheDocument();
		expect(patchFeature.updatePolicy).not.toHaveBeenCalled();

		state.patchSetArmed = true;
		rerender(<FixturePatchSetup />);
		fireEvent.click(screen.getByRole("button", { name: "Group Masters 17" }));
		const dialog = (
			await screen.findByRole("heading", {
				name: "Set fixture Group Masters",
			})
		).closest("section") as HTMLElement;
		fireEvent.click(within(dialog).getByRole("button", { name: "Controlled" }));
		fireEvent.click(screen.getByRole("option", { name: "Ignored" }));
		expect(
			within(dialog).getByText(
				"This fixture can remain live while the Group Masters are reduced.",
			),
		).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(patchFeature.updatePolicy).toHaveBeenCalledWith(
				"fixture-split",
				{ type: "group_masters", controlled: false },
				{ group_masters_enabled: false },
			),
		);
		expect(setEditArmed).toHaveBeenCalledWith(false);
	});

	it("edits one physical multi-patch axis without changing its shared policy", async () => {
		server.patch.fixtures = [policyFixture()];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		expect(screen.getAllByText("Shared · Controlled")).toHaveLength(2);
		expect(
			screen.getByRole("button", {
				name: "Invert tilt Opposite hang",
			}),
		).toHaveTextContent("Inverted");

		fireEvent.click(
			screen.getByRole("button", { name: "Invert pan Opposite hang" }),
		);
		const dialog = (
			await screen.findByRole("heading", {
				name: "Set multi-patch Invert Pan",
			})
		).closest("section") as HTMLElement;
		fireEvent.click(within(dialog).getByRole("button", { name: "Normal" }));
		fireEvent.click(screen.getByRole("option", { name: "Inverted" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() => {
			const [fixtureId, action, changes] =
				patchFeature.updatePolicy.mock.calls[0];
			expect(fixtureId).toBe("fixture-split");
			expect(action).toEqual({
				type: "axis_inversion",
				axis: "pan",
				inverted: true,
				multipatchInstanceId: "physical-copy",
			});
			expect(changes.multipatch[0]).toMatchObject({
				id: "physical-copy",
				invert_pan: true,
				invert_tilt: true,
			});
		});
	});
});

describe("selected split selection and SET editing", () => {
	it("can reveal cross-scope and empty layers without changing fixture scope", () => {
		const light = splitFixture();
		light.layer_id = "lights";
		const venue = splitFixture();
		venue.fixture_id = "venue-model";
		venue.fixture_number = null;
		venue.virtual_fixture_number = 1;
		venue.name = "Stage deck";
		venue.layer_id = "stage";
		venue.definition = {
			...venue.definition,
			device_type: "venue",
			footprint: 0,
		};
		if (venue.definition.profile_snapshot)
			venue.definition.profile_snapshot.patch_policy = "visual_only";
		server.patch.fixtures = [light, venue];
		server.patchLayers = [
			{ body: { id: "lights", name: "Lights", order: 0 } },
			{ body: { id: "stage", name: "Stage", order: 1 } },
			{ body: { id: "empty", name: "Empty", order: 2 } },
		];

		render(<FixturePatchSetup scope="dmx" />);
		expect(screen.getByRole("button", { name: "Lights1" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Empty0" })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Stage0" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("switch", { name: "Show all layers" }));
		expect(screen.getByRole("button", { name: "Stage0" })).toBeInTheDocument();
		expect(screen.queryByText("Stage deck")).not.toBeInTheDocument();
	});

	it("provides desktop inline edit apply, cancel, and keyboard behavior", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: "Edit Name 17" }));
		const input = screen.getByRole("textbox", { name: "Edit Name 17" });
		expect(input).toHaveFocus();
		fireEvent.change(input, { target: { value: "Front Wash" } });
		fireEvent.keyDown(input, { key: "Escape" });
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: "Edit Name 17" }));
		const applied = screen.getByRole("textbox", { name: "Edit Name 17" });
		fireEvent.change(applied, { target: { value: "Front Wash" } });
		fireEvent.keyDown(applied, { key: "Enter" });
		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				name: "Front Wash",
			}),
		);
	});

	it("keeps inline text fixed and guards only dirty outside-click closes", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		render(<FixturePatchSetup />);

		const value = screen.getByRole("textbox", { name: "Name 17" });
		const valueShell = value.closest(".patch-inline-value");
		const idleWidth = valueShell?.getAttribute("style");
		const idleSize = value.getAttribute("size");
		expect(valueShell?.firstElementChild).toBe(
			screen.getByRole("button", { name: "Edit Name 17" }),
		);
		expect(valueShell?.lastElementChild).toBe(value);

		fireEvent.click(screen.getByRole("button", { name: "Edit Name 17" }));
		let input = screen.getByRole("textbox", { name: "Edit Name 17" });
		const editor = input.closest(".patch-inline-editor");
		expect(editor?.getAttribute("style")).toBe(idleWidth);
		expect(input.getAttribute("size")).toBe(idleSize);
		expect(editor?.lastElementChild).toBe(input);
		expect(editor?.childElementCount).toBe(2);
		fireEvent.pointerDown(screen.getByRole("heading", { name: "Layers" }));
		expect(
			screen.queryByRole("textbox", { name: "Edit Name 17" }),
		).not.toBeInTheDocument();
		expect(screen.queryByText("Discard changes?")).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Edit Name 17" }));
		input = screen.getByRole("textbox", { name: "Edit Name 17" });
		fireEvent.change(input, { target: { value: "Front Wash" } });
		fireEvent.pointerDown(screen.getByRole("heading", { name: "Layers" }));
		expect(await screen.findByText("Discard changes?")).toBeInTheDocument();
		expect(input).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
		expect(screen.getByRole("textbox", { name: "Edit Name 17" })).toHaveValue(
			"Front Wash",
		);
		fireEvent.pointerDown(screen.getByRole("heading", { name: "Layers" }));
		fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
		expect(
			screen.queryByRole("textbox", { name: "Edit Name 17" }),
		).not.toBeInTheDocument();
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();
	});

	it("overlays configure before the patch pencil without changing field geometry", () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		const fixture = splitFixture();
		const mode = fixture.definition.profile_snapshot?.modes[0];
		if (!mode) throw new Error("fixture mode is missing");
		mode.splits = [{ number: 1, footprint: 4 }];
		fixture.split_patches = [{ split: 1, universe: 1, address: 101 }];
		server.patch.fixtures = [fixture];
		render(<FixturePatchSetup />);

		const patch = screen.getByRole("textbox", { name: "Patch 17" });
		const shell = patch.closest(".patch-inline-value");
		const actions = shell?.querySelectorAll("button");
		expect(actions).toHaveLength(2);
		expect(actions?.[0]).toHaveAccessibleName(
			"Open patch settings for fixture 17",
		);
		expect(actions?.[1]).toHaveAccessibleName("Edit Patch 17");
		expect(shell?.lastElementChild).toBe(patch);
	});

	it("opens advanced modal editing from context click and a patch settings action", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		render(<FixturePatchSetup />);

		fireEvent.contextMenu(screen.getByRole("textbox", { name: "Name 17" }));
		expect(await screen.findByText("Set fixture name")).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Cancel fixture name" }),
		);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Open patch settings for fixture 17",
			}),
		);
		expect(
			await screen.findByRole("dialog", { name: "Fixture Address" }),
		).toBeInTheDocument();
	});

	it("keeps THRU spreading available for a contextual multi-selection", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		const second = splitFixture();
		second.fixture_id = "fixture-18";
		second.fixture_number = 18;
		second.name = "Split Wash 18";
		const third = splitFixture();
		third.fixture_id = "fixture-19";
		third.fixture_number = 19;
		third.name = "Split Wash 19";
		server.patch.fixtures = [splitFixture(), second, third];
		programming.selection.selected = [
			"fixture-19",
			"fixture-split",
			"fixture-18",
		];
		render(<FixturePatchSetup />);

		fireEvent.contextMenu(
			screen.getByRole("textbox", { name: "Fixture ID 17" }),
		);
		expect(
			await screen.findByRole("dialog", { name: "Fixture ID" }),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "THRU" })).toBeInTheDocument();
		for (const key of ["2", "0", "THRU", "2", "2", "ENTER"])
			fireEvent.click(screen.getByRole("button", { name: key }));

		await waitFor(() => {
			expect(patchFeature.updateFixture).toHaveBeenNthCalledWith(
				1,
				"fixture-19",
				{
					fixture_number: 20,
					virtual_fixture_number: null,
				},
			);
			expect(patchFeature.updateFixture).toHaveBeenNthCalledWith(
				2,
				"fixture-split",
				{
					fixture_number: 21,
					virtual_fixture_number: null,
				},
			);
			expect(patchFeature.updateFixture).toHaveBeenNthCalledWith(
				3,
				"fixture-18",
				{
					fixture_number: 22,
					virtual_fixture_number: null,
				},
			);
		});
	});

	it("shows the selected location spread and preserves it when submitted unchanged", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		const fixtures = [-3800, -3150, -2500, 2500].map((x, index) => {
			const fixture = splitFixture();
			fixture.fixture_id = `fixture-${index + 1}`;
			fixture.fixture_number = index + 1;
			fixture.name = `Fixture ${index + 1}`;
			fixture.location = { x, y: 0, z: 0 };
			return fixture;
		});
		server.patch.fixtures = fixtures;
		programming.selection.selected = fixtures.map(
			(fixture) => fixture.fixture_id,
		);
		render(<FixturePatchSetup />);

		fireEvent.contextMenu(screen.getByRole("button", { name: "-3.800 m" }));
		expect(
			screen.getByRole("textbox", { name: "Location X · 4 fixtures value" }),
		).toHaveValue("-3.8 THRU 2.5");
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Location X" }),
			).not.toBeInTheDocument(),
		);
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();

		fireEvent.contextMenu(screen.getByRole("button", { name: "-3.800 m" }));
		for (const key of ["0", "THRU", "3", "ENTER"])
			fireEvent.click(screen.getByRole("button", { name: key }));
		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledTimes(4),
		);
		expect(
			patchFeature.updateFixture.mock.calls.map(
				([, changes]) => (changes as PatchedFixture).location?.x,
			),
		).toEqual([0, 1000, 2000, 3000]);
	});

	it("offers discard only after a contextual number value actually changes", async () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		render(<FixturePatchSetup />);

		const value = screen.getByRole("textbox", { name: "Fixture ID 17" });
		fireEvent.contextMenu(value);
		fireEvent.click(screen.getByRole("button", { name: "Close Fixture ID" }));
		expect(
			screen.queryByRole("button", { name: "Discard changes" }),
		).not.toBeInTheDocument();

		fireEvent.contextMenu(value);
		fireEvent.click(screen.getByRole("button", { name: "2" }));
		fireEvent.click(screen.getByRole("button", { name: "Close Fixture ID" }));
		expect(
			screen.getByRole("button", { name: "Discard changes" }),
		).toBeInTheDocument();
	});

	it("selects an inclusive desktop row range by dragging", () => {
		state.patchSetArmed = true;
		state.desktopEditing = true;
		const second = splitFixture();
		second.fixture_id = "fixture-18";
		second.fixture_number = 18;
		second.name = "Split Wash 18";
		const third = splitFixture();
		third.fixture_id = "fixture-19";
		third.fixture_number = 19;
		third.name = "Split Wash 19";
		server.patch.fixtures = [splitFixture(), second, third];
		render(<FixturePatchSetup />);

		fireEvent.mouseDown(screen.getByRole("row", { name: /17 Split Wash 17/ }), {
			button: 0,
			buttons: 1,
		});
		fireEvent.mouseEnter(
			screen.getByRole("row", { name: /19 Split Wash 19/ }),
			{
				buttons: 1,
			},
		);
		expect(programming.actions.replace).toHaveBeenLastCalledWith({
			resolvedFixtures: ["fixture-split", "fixture-18", "fixture-19"],
		});
	});

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
			"is-active",
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				split_patches: [
					{ split: 1, universe: 1, address: 101 },
					{ split: 3, universe: 4, address: 401 },
				],
				universe: 1,
				address: 101,
			}),
		);
		expect(setEditArmed).toHaveBeenCalledWith(false);
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
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();
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

		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		expect(
			patchFeature.patchFixtures.mock.calls[0][0].map(
				(candidate: { fixture: PatchedFixture }) =>
					candidate.fixture.fixture_number,
			),
		).toEqual([100, 102, 103, 104]);
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
	it("opens the Universe menu above the placement modal and switches universes", () => {
		const placement = openDimmerPlacement();
		const modal = placement.closest(
			'.ui-modal-stack-layer[data-modal-top="true"]',
		);
		fireEvent.click(
			within(placement).getByRole("button", { name: "Universe" }),
		);
		const listbox = screen.getByRole("listbox");
		expect(listbox.closest("[data-modal-id]")).toBe(modal);
		fireEvent.click(within(listbox).getByRole("option", { name: "2" }));
		expect(
			within(placement).getByRole("grid", { name: "DMX universe 2" }),
		).toBeInTheDocument();
	});

	it("releases concrete preview reservations for Empty and reacquires them for Address", () => {
		const placement = openDimmerPlacement();
		const choice = within(placement).getByRole("group", {
			name: "Fixture placement address",
		});
		expect(within(choice).getByRole("button", { name: "Address" })).toHaveClass(
			"is-active",
		);
		expect(
			within(placement).getByRole("grid", { name: "DMX universe 1" }),
		).toBeInTheDocument();
		fireEvent.change(
			within(placement).getByRole("textbox", {
				name: "Address (universe.address)",
			}),
			{ target: { value: "2.100" } },
		);

		fireEvent.click(within(choice).getByRole("button", { name: "Empty" }));

		expect(within(placement).getByText("Placement: Empty")).toBeInTheDocument();
		expect(within(placement).queryByRole("grid")).not.toBeInTheDocument();
		expect(
			within(placement).queryByRole("textbox", {
				name: "Address (universe.address)",
			}),
		).not.toBeInTheDocument();

		fireEvent.click(within(choice).getByRole("button", { name: "Address" }));

		expect(
			within(placement).getByRole("textbox", {
				name: "Address (universe.address)",
			}),
		).toHaveValue("2.100");
		expect(
			within(placement)
				.getByRole("grid", { name: "DMX universe 2" })
				.querySelector('[data-dmx-address="100"]'),
		).toHaveClass("proposed");
	});

	it("adds every requested fixture unpatched without address progression", async () => {
		const placement = openDimmerPlacement();
		fireEvent.change(
			within(placement).getByRole("textbox", { name: "Count" }),
			{
				target: { value: "3" },
			},
		);
		fireEvent.click(within(placement).getByRole("button", { name: "Empty" }));
		fireEvent.click(
			within(placement).getByRole("button", { name: "Add 3 fixtures" }),
		);

		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		const [candidates, placements] = patchFeature.patchFixtures.mock.calls[0];
		expect(placements).toEqual([]);
		expect(
			candidates.map(
				(candidate: { fixture: PatchedFixture }) =>
					candidate.fixture.fixture_number,
			),
		).toEqual([1, 2, 3]);
		expect(
			candidates.map((candidate: { fixture: PatchedFixture }) => ({
				universe: candidate.fixture.universe,
				address: candidate.fixture.address,
				split_patches: candidate.fixture.split_patches,
			})),
		).toEqual([
			{
				universe: null,
				address: null,
				split_patches: [{ split: 1, universe: null, address: null }],
			},
			{
				universe: null,
				address: null,
				split_patches: [{ split: 1, universe: null, address: null }],
			},
			{
				universe: null,
				address: null,
				split_patches: [{ split: 1, universe: null, address: null }],
			},
		]);
	});

	it("adds one deliberately Empty fixture with its normal identity and profile", async () => {
		const placement = openDimmerPlacement();
		fireEvent.click(within(placement).getByRole("button", { name: "Empty" }));
		fireEvent.click(
			within(placement).getByRole("button", { name: "Add 1 fixtures" }),
		);

		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		const candidate = patchFeature.patchFixtures.mock.calls[0][0][0].fixture;
		expect(candidate).toMatchObject({
			fixture_number: 1,
			name: "Fixture 1",
			universe: null,
			address: null,
			split_patches: [{ split: 1, universe: null, address: null }],
			layer_id: "default",
		});
		expect(candidate.fixture_id).toEqual(expect.any(String));
		expect(candidate.definition.name).toBe("Dimmer");
	});

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
		const count = within(placement).getByRole("textbox", { name: "Count" });
		fireEvent.change(count, { target: { value: "1" } });
		fireEvent.change(count, { target: { value: "3" } });
		expect(grid.querySelector('[data-dmx-address="2"]')).toHaveAccessibleName(
			/Fixture 2/,
		);
		expect(
			grid.querySelector('[data-dmx-address="50"]'),
		).not.toHaveAccessibleName(/Fixture 2/);

		Object.defineProperty(document, "elementFromPoint", {
			configurable: true,
			value: vi.fn(() => destination),
		});
		try {
			fireEvent.pointerDown(
				grid.querySelector('[data-dmx-address="2"]') as HTMLElement,
				{ pointerId: 10, clientX: 10, clientY: 10 },
			);
			fireEvent.pointerMove(grid, {
				pointerId: 10,
				clientX: 100,
				clientY: 100,
			});
			fireEvent.pointerUp(grid, { pointerId: 10 });
		} finally {
			Object.defineProperty(document, "elementFromPoint", {
				configurable: true,
				value: originalElementFromPoint,
			});
		}
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
		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		expect(
			patchFeature.patchFixtures.mock.calls[0][0].map(
				(candidate: { fixture: PatchedFixture }) => candidate.fixture.address,
			),
		).toEqual([1, 50, 3]);
		const candidates = patchFeature.patchFixtures.mock.calls[0][0] as Array<{
			fixture: PatchedFixture;
		}>;
		expect(patchFeature.patchFixtures.mock.calls[0][1]).toEqual([
			{
				fixtureIds: candidates.map((candidate) => candidate.fixture.fixture_id),
				splits: [
					{
						split: 1,
						universe: 1,
						address: 1,
						mode: {
							type: "operator_overrides",
							overrides: [
								{
									fixtureId: candidates[1].fixture.fixture_id,
									universe: 1,
									address: 50,
								},
							],
						},
					},
				],
			},
		]);
		expect(programming.actions.replace).toHaveBeenCalledWith({
			resolvedFixtures: ["last-head-left", "last-head-right"],
		});
	});
});

describe("fixture browser filtering", () => {
	it("patches a Media Server through the shared add flow and reports its fixture", async () => {
		const mediaServer = blankFixtureProfile();
		mediaServer.id = "media-server-profile";
		mediaServer.manufacturer = "ToskLight";
		mediaServer.name = "Media Server";
		mediaServer.short_name = "Media Server";
		mediaServer.fixture_type = "media_server";
		server.fixtureProfiles = [mediaServer];
		server.patch.fixtures = [];
		const onFixturesAdded = vi.fn();

		render(
			<FixtureAddFlow
				addRequest={1}
				scope="media"
				initialTypeFilter="media_server"
				onFixturesAdded={onFixturesAdded}
			/>,
		);
		fireEvent.click(
			await screen.findByRole("button", { name: /^Add fixture$/ }),
		);
		const placement = screen
			.getByRole("heading", { name: "Patch Media Server" })
			.closest("section") as HTMLElement;
		fireEvent.click(
			within(placement).getByRole("button", { name: "Add 1 fixtures" }),
		);

		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		const fixture = patchFeature.patchFixtures.mock.calls[0][0][0].fixture;
		expect(fixture.definition.device_type).toBe("media_server");
		expect(onFixturesAdded).toHaveBeenCalledWith([
			{ fixtureId: fixture.fixture_id, name: fixture.name },
		]);
	});

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

		await waitFor(() =>
			expect(patchFeature.patchFixtures).toHaveBeenCalledOnce(),
		);
		expect(patchFeature.patchFixtures.mock.calls[0][0][0].fixture).toEqual(
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
	it("fits whole touch cells into the available width", () => {
		expect(dmxGridColumnCount(360)).toBe(7);
		expect(dmxGridColumnCount(826)).toBe(16);
		expect(dmxGridColumnCount(1200)).toBe(24);
	});

	it("switches universes from the visible universe control", () => {
		const onUniverse = vi.fn();
		render(
			<UniverseMap
				fixtures={[]}
				universe={1}
				proposed={10}
				footprint={4}
				proposedLabel="Fixture 10 · Test"
				onAddress={vi.fn()}
				onUniverse={onUniverse}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Universe" }));
		fireEvent.click(screen.getByRole("option", { name: "2" }));
		expect(onUniverse).toHaveBeenCalledWith(2);
	});

	it("moves the proposed footprint with mouse or touch pointer events while preserving the grabbed offset", () => {
		const onAddress = vi.fn();
		function ControlledMap() {
			const [proposed, setProposed] = useState(10);
			return (
				<UniverseMap
					fixtures={[]}
					universe={1}
					proposed={proposed}
					footprint={4}
					proposedLabel="Fixture 10 · Test"
					onAddress={(address) => {
						onAddress(address);
						setProposed(address);
					}}
					onUniverse={vi.fn()}
				/>
			);
		}
		render(<ControlledMap />);
		const grid = screen.getByRole("grid", { name: "DMX universe 1" });
		const grabbed = grid.querySelector(
			'[data-dmx-address="10"]',
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
			expect(onAddress).toHaveBeenLastCalledWith(50);
			fireEvent.pointerUp(grid, { pointerId: 7 });
			fireEvent.click(grabbed);
			expect(onAddress).toHaveBeenCalledOnce();
			expect(
				grid.querySelector('[data-dmx-address="50"]'),
			).toHaveAccessibleName(/proposed patch for Fixture 10/);
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
		fireEvent.click(within(fixtureRow.cells[12]).getByRole("button"));

		const modal = screen
			.getByRole("heading", { name: "Set fixture location X" })
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				location: { x: 1000, y: 0, z: 0 },
			}),
		);
	});

	it("edits exactly one axis and never resubmits the sibling axes", async () => {
		const { current } = fixturesWithConflict();
		current.location = { x: 111, y: 222, z: 333 };
		current.rotation = { x: 10, y: 20, z: 30 };
		server.patch.fixtures = [current];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		fireEvent.click(within(fixtureRow.cells[16]).getByRole("button"));

		const modal = screen
			.getByRole("heading", { name: "Set fixture rotation Y" })
			.closest("section") as HTMLElement;
		expect(within(modal).queryByRole("textbox", { name: "X (°)" })).toBeNull();
		fireEvent.change(within(modal).getByRole("textbox", { name: "Y (°)" }), {
			target: { value: "45" },
		});
		fireEvent.click(within(modal).getByRole("button", { name: "Set" }));
		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				rotation: { x: 10, y: 45, z: 30 },
			}),
		);
	});

	it("edits exactly one multi-patch instance axis without touching its siblings", async () => {
		const { current } = fixturesWithConflict();
		current.multipatch = [
			{
				...(current.multipatch?.[0] as MultiPatchInstance),
				location: { x: 111, y: 222, z: 333 },
				rotation: { x: 10, y: 20, z: 30 },
			},
		];
		server.patch.fixtures = [current];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		const instanceRow = document.querySelector(
			"tr.multipatch-row",
		) as HTMLTableRowElement;
		fireEvent.click(within(instanceRow.cells[16]).getByRole("button"));

		const modal = screen
			.getByRole("heading", { name: "Set multi-patch rotation Y" })
			.closest("section") as HTMLElement;
		fireEvent.change(within(modal).getByRole("textbox", { name: "Y (°)" }), {
			target: { value: "45" },
		});
		fireEvent.click(within(modal).getByRole("button", { name: "Set" }));
		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				multipatch: [
					expect.objectContaining({
						id: "current-mp",
						location: { x: 111, y: 222, z: 333 },
						rotation: { x: 10, y: 45, z: 30 },
					}),
				],
			}),
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith(
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith(
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
		expect(patchFeature.deleteFixture).not.toHaveBeenCalled();
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
			expect(patchFeature.deleteFixture).toHaveBeenCalledWith("fixture-split"),
		);
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
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
			}),
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
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();
		expect(confirm).toHaveBeenCalledOnce();
	});
});
