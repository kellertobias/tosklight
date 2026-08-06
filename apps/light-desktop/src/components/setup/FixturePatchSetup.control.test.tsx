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
import type { MultiPatchInstance, PatchedFixture } from "../../api/types";
import {
	FixturePatchSetup,
	fixtureDisplayId,
	parseVirtualFixtureNumber,
	UniverseMap,
} from "./FixturePatchSetup";
import { dmxGridColumnCount } from "./fixturePatch/UniverseMap";
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
	refresh: vi.fn(),
	savePatchLayer: vi.fn(),
	gelCatalogs: vi.fn().mockResolvedValue([]),
	previewGelCatalogCsvImport: vi.fn(),
	confirmGelCatalogCsvImport: vi.fn(),
};
const patchFeature = {
	selectedPatchInstance: null as null | {
		fixtureId: string;
		multipatchInstanceId: string | null;
	},
	selectPatchInstance: vi.fn(),
	patchFixtures: vi.fn(),
	spreadFixtureVector: vi.fn().mockResolvedValue(true),
	updateFixture: vi.fn().mockResolvedValue(true),
	updatePolicy: vi.fn().mockResolvedValue(true),
	updateFixtureIntent: vi.fn().mockResolvedValue(true),
	deleteFixture: vi.fn().mockResolvedValue(true),
};

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
			selectedPatchInstance: patchFeature.selectedPatchInstance,
			selectPatchInstance: patchFeature.selectPatchInstance,
			pendingFixtureIds: new Set<string>(),
			error: null,
			patchFixtures: patchFeature.patchFixtures,
			spreadFixtureVector: patchFeature.spreadFixtureVector,
			updateFixture: patchFeature.updateFixture,
			updatePolicy: patchFeature.updatePolicy,
			updateFixtureIntent: patchFeature.updateFixtureIntent,
			deleteFixture: patchFeature.deleteFixture,
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

function appearanceFixture(): PatchedFixture {
	const fixture = policyFixture();
	const profile = fixture.definition.profile_snapshot;
	if (!profile) throw new Error("appearance fixture profile is missing");
	profile.physical.light_source = "LED engine";
	profile.physical.color_temperature_kelvin = 3_200;
	fixture.installed_appearance = {
		light_source: { type: "profile_default" },
		color_temperature_kelvin: null,
		gel: { type: "open_white" },
		shaper_angles_degrees: [1, 2, 3, 4],
	};
	const copy = fixture.multipatch?.[0];
	if (!copy) throw new Error("appearance fixture copy is missing");
	copy.installed_appearance = {
		light_source: { type: "led" },
		color_temperature_kelvin: 5_600,
		gel: {
			type: "custom",
			name: "Steel Blue",
			color_srgb: "#6699CC",
			note: "Front truss",
		},
		shaper_angles_degrees: [10, 20, 30, 40],
	};
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
	patchFeature.selectedPatchInstance = null;
	patchFeature.selectPatchInstance.mockReset();
	state.patchSetArmed = false;
	server.patch.fixtures = [splitFixture()];
	server.fixtureProfiles = [];
	server.selectedFixtures = [];
	programming.ready = true;
	programming.selection.selected = [];
	vi.clearAllMocks();
	programming.actions.replace.mockResolvedValue(null);
	programming.actions.gesture.mockResolvedValue(null);
	patchFeature.updateFixture.mockResolvedValue(true);
	patchFeature.updateFixtureIntent.mockResolvedValue(true);
	patchFeature.spreadFixtureVector.mockResolvedValue(true);
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

	it("shows independent effective values and atomically edits both masters through armed SET", async () => {
		server.patch.fixtures = [policyFixture()];
		render(<FixturePatchSetup />);

		expect(
			screen.getByRole("button", { name: "Masters 17" }),
		).toHaveTextContent("Both");
		expect(
			screen.getByRole("button", { name: "Pan and Tilt 17" }),
		).toHaveTextContent("none");

		fireEvent.click(screen.getByRole("button", { name: "Masters 17" }));
		expect(
			screen.queryByRole("heading", { name: "Set fixture Masters" }),
		).not.toBeInTheDocument();
		expect(patchFeature.updateFixture).not.toHaveBeenCalled();

		state.patchSetArmed = true;
		fireEvent.click(screen.getByRole("button", { name: "Masters 17" }));
		const dialog = (
			await screen.findByRole("heading", {
				name: "Set fixture Masters",
			})
		).closest("section") as HTMLElement;
		fireEvent.click(within(dialog).getByRole("button", { name: "Both" }));
		fireEvent.click(screen.getByRole("option", { name: "Grand Master" }));
		expect(
			within(dialog).getByText(
				"This fixture may remain live while an applicable master is reduced.",
			),
		).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_masters",
					groupMastersEnabled: false,
					grandMasterEnabled: true,
				},
			),
		);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PATCH_ARMED",
			value: false,
		});
	});

	it("atomically edits both axes on one physical multi-patch without changing shared policy", async () => {
		server.patch.fixtures = [policyFixture()];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		const multiPatchRow = screen.getByRole("row", {
			name: "Multi-patch Opposite hang",
		}) as HTMLTableRowElement;
		expect(multiPatchRow.cells[5]).toHaveTextContent(/^—$/);
		expect(
			screen.getByRole("button", {
				name: "Pan and Tilt Opposite hang",
			}),
		).toHaveTextContent("Invert Tilt");

		fireEvent.click(
			screen.getByRole("button", { name: "Pan and Tilt Opposite hang" }),
		);
		const dialog = (
			await screen.findByRole("heading", {
				name: "Set multi-patch Pan / Tilt",
			})
		).closest("section") as HTMLElement;
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Invert Tilt" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Invert Both" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				"physical-copy",
				{
					type: "set_pan_tilt",
					invertPan: true,
					invertTilt: true,
				},
			),
		);
	});

	it("keeps MIB Off distinct from an enabled zero-second delay", async () => {
		const fixture = policyFixture();
		fixture.move_in_black_enabled = false;
		fixture.move_in_black_delay_millis = 750;
		server.patch.fixtures = [fixture];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: "MIB 17: Off" }));
		const dialog = (
			await screen.findByRole("heading", { name: "Set fixture MIB" })
		).closest("section") as HTMLElement;
		const input = within(dialog).getByRole("textbox", {
			name: "MIB value: Off or non-negative seconds",
		});
		expect(input).toHaveValue("Off");
		fireEvent.change(input, { target: { value: "0" } });
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_move_in_black",
					enabled: true,
					delayMillis: 0,
				},
			),
		);
	});
});

describe("installed light-source appearance", () => {
	it("keeps an unavailable catalog gel's complete embedded fallback during unrelated edits", async () => {
		const fixture = appearanceFixture();
		fixture.installed_appearance = {
			light_source: { type: "profile_default" },
			color_temperature_kelvin: null,
			gel: {
				type: "built_in",
				catalog_id: "missing-catalog",
				entry_id: "missing-entry",
				embedded_fallback: {
					number: "G12",
					name: "Deep Violet",
					display_srgb: "#552288",
					visualizer_srgb: "#32105F",
				},
			},
			shaper_angles_degrees: [1, 2, 3, 4],
		};
		server.patch.fixtures = [fixture];
		server.gelCatalogs.mockResolvedValue([
			{
				id: "lookalike-catalog",
				revision: 1,
				name: "Lookalike filters",
				entries: [
					{
						id: "lookalike-entry",
						number: "G12",
						name: "Deep Violet",
						display_srgb: "#552288",
						visualizer_srgb: "#32105F",
					},
				],
			},
		]);
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		const dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		expect(await within(dialog).findByRole("alert")).toHaveTextContent(
			"Catalog unavailable",
		);
		expect(within(dialog).getByLabelText("Unavailable gel")).toHaveTextContent(
			"Stored reference: missing-catalog / missing-entry",
		);
		expect(within(dialog).getByLabelText("Unavailable gel")).toHaveTextContent(
			"Fallback: G12 · Deep Violet · display #552288 · visualizer #32105F",
		);
		expect(within(dialog).getByLabelText("Unavailable gel")).toHaveTextContent(
			"Selecting a result explicitly reconciles this fixture",
		);
		expect(
			within(dialog).getByText("Import gel catalog CSV"),
		).toBeInTheDocument();

		fireEvent.click(
			within(dialog).getByRole("button", { name: "Profile default" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Halogen" }));
		fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_installed_appearance",
					appearance: {
						lightSource: { type: "halogen" },
						colorTemperatureKelvin: null,
						gel: {
							type: "built_in",
							catalogId: "missing-catalog",
							entryId: "missing-entry",
							embeddedFallback: {
								number: "G12",
								name: "Deep Violet",
								displaySrgb: "#552288",
								visualizerSrgb: "#32105F",
							},
						},
						shaperAnglesDegrees: [1, 2, 3, 4],
					},
				},
			),
		);
	});

	it("identifies a missing entry and reconciles it only after explicit selection", async () => {
		const fixture = appearanceFixture();
		fixture.installed_appearance = {
			light_source: { type: "profile_default" },
			color_temperature_kelvin: null,
			gel: {
				type: "built_in",
				catalog_id: "catalog-blue",
				entry_id: "retired-entry",
				embedded_fallback: {
					number: "B1",
					name: "Retired Blue",
					display_srgb: "#1122AA",
					visualizer_srgb: "#0F1F99",
				},
			},
			shaper_angles_degrees: [1, 2, 3, 4],
		};
		server.patch.fixtures = [fixture];
		server.gelCatalogs.mockResolvedValue([
			{
				id: "catalog-blue",
				revision: 4,
				name: "Touring filters",
				entries: [
					{
						id: "replacement-entry",
						number: "B2",
						name: "Current Blue",
						display_srgb: "#2233BB",
						visualizer_srgb: "#1020A0",
					},
				],
			},
		]);
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		const dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		expect(await within(dialog).findByRole("alert")).toHaveTextContent(
			"Catalog entry unavailable",
		);
		expect(within(dialog).getByLabelText("Unavailable gel")).toHaveTextContent(
			"retired-entry in Touring filters",
		);
		expect(
			within(dialog).getByRole("button", { name: "Apply" }),
		).toBeDisabled();

		fireEvent.click(
			within(dialog).getByRole("button", {
				name: /Touring filters · B2 · Current Blue/,
			}),
		);
		expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				expect.objectContaining({
					type: "set_installed_appearance",
					appearance: expect.objectContaining({
						gel: {
							type: "built_in",
							catalogId: "catalog-blue",
							entryId: "replacement-entry",
							embeddedFallback: {
								number: "B2",
								name: "Current Blue",
								displaySrgb: "#2233BB",
								visualizerSrgb: "#1020A0",
							},
						},
					}),
				}),
			),
		);
	});

	it("searches installed catalogs and stores a portable embedded fallback", async () => {
		server.patch.fixtures = [appearanceFixture()];
		server.gelCatalogs.mockResolvedValue([
			{
				id: "catalog-blue",
				revision: 3,
				name: "Touring filters",
				entries: [
					{
						id: "entry-r80",
						number: "R80",
						name: "Primary Blue",
						display_srgb: "#1122AA",
						visualizer_srgb: "#0F1F99",
					},
				],
			},
		]);
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		const dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		fireEvent.click(within(dialog).getByRole("button", { name: "Open white" }));
		fireEvent.click(screen.getByRole("option", { name: "Catalog gel" }));
		fireEvent.change(
			within(dialog).getByLabelText("Search catalog, number, or name"),
			{ target: { value: "primary" } },
		);
		await waitFor(() =>
			expect(server.gelCatalogs).toHaveBeenLastCalledWith("primary"),
		);
		fireEvent.click(
			await within(dialog).findByRole("button", {
				name: /Touring filters · R80 · Primary Blue/,
			}),
		);
		fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_installed_appearance",
					appearance: {
						lightSource: { type: "profile_default" },
						colorTemperatureKelvin: null,
						gel: {
							type: "built_in",
							catalogId: "catalog-blue",
							entryId: "entry-r80",
							embeddedFallback: {
								number: "R80",
								name: "Primary Blue",
								displaySrgb: "#1122AA",
								visualizerSrgb: "#0F1F99",
							},
						},
						shaperAnglesDegrees: [1, 2, 3, 4],
					},
				},
			),
		);
	});

	it("shows strict CSV preview problems and requires explicit confirmation", async () => {
		server.patch.fixtures = [appearanceFixture()];
		server.gelCatalogs.mockResolvedValue([]);
		server.previewGelCatalogCsvImport.mockResolvedValue({
			catalog_id: "catalog-new",
			catalog_name: "Imported filters",
			catalog_name_changed: true,
			additions: [
				{
					entry: {
						id: "entry",
						number: "1",
						name: "Blue",
						display_srgb: "#0000FF",
						visualizer_srgb: "#0000EE",
					},
				},
			],
			replacements: [],
			unchanged: [],
			conflicts: [],
			invalid_rows: [],
			confirmable: true,
		});
		server.confirmGelCatalogCsvImport.mockResolvedValue({
			id: "catalog-new",
			revision: 1,
			name: "Imported filters",
			entries: [],
		});
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		const dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		fireEvent.click(within(dialog).getByRole("button", { name: "Open white" }));
		fireEvent.click(screen.getByRole("option", { name: "Catalog gel" }));
		fireEvent.click(within(dialog).getByText("Import gel catalog CSV"));
		fireEvent.change(within(dialog).getByLabelText("Catalog name"), {
			target: { value: "Imported filters" },
		});
		const file = new File(
			["number,name,display_rgb,visualizer_rgb\n1,Blue,#0000FF,#0000EE\n"],
			"filters.csv",
			{ type: "text/csv" },
		);
		Object.defineProperty(file, "arrayBuffer", {
			value: async () =>
				new TextEncoder().encode(
					"number,name,display_rgb,visualizer_rgb\n1,Blue,#0000FF,#0000EE\n",
				).buffer,
		});
		fireEvent.change(within(dialog).getByLabelText("Catalog CSV"), {
			target: { files: [file] },
		});
		await waitFor(() =>
			expect(
				within(dialog).getByRole("button", { name: "Preview import" }),
			).toBeEnabled(),
		);
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Preview import" }),
		);
		expect(
			await within(dialog).findByText(
				"1 additions · 0 replacements · 0 unchanged",
			),
		).toBeInTheDocument();
		expect(server.confirmGelCatalogCsvImport).not.toHaveBeenCalled();
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Confirm import" }),
		);
		await waitFor(() =>
			expect(server.confirmGelCatalogCsvImport).toHaveBeenCalledOnce(),
		);
		expect(await within(dialog).findByRole("status")).toHaveTextContent(
			"Imported Imported filters revision 1.",
		);
	});

	it("shows the exact root and multi-patch source, effective CCT, and gel", () => {
		server.patch.fixtures = [appearanceFixture()];
		render(<FixturePatchSetup />);

		const root = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		const copy = screen.getByRole("row", {
			name: "Multi-patch Opposite hang",
		}) as HTMLTableRowElement;
		expect(root.cells[8]).toHaveTextContent(
			"Profile default · 3,200 KOpen white",
		);
		expect(copy.cells[8]).toHaveTextContent("LED · 5,600 KSteel Blue");
	});

	it("retains independent installed appearances when changing to a newly embedded profile mode", async () => {
		const fixture = appearanceFixture();
		fixture.installed_appearance = {
			light_source: { type: "profile_default" },
			color_temperature_kelvin: null,
			gel: {
				type: "built_in",
				catalog_id: "catalog-tour",
				entry_id: "entry-r80",
				embedded_fallback: {
					number: "R80",
					name: "Primary Blue",
					display_srgb: "#1122AA",
					visualizer_srgb: "#0F1F99",
				},
			},
			shaper_angles_degrees: [1, 2, 3, 4],
		};
		const originalRootAppearance = structuredClone(
			fixture.installed_appearance,
		);
		const originalCopyAppearance = structuredClone(
			fixture.multipatch?.[0]?.installed_appearance,
		);
		const nextProfile = structuredClone(fixture.definition.profile_snapshot);
		if (!nextProfile) throw new Error("appearance fixture profile is missing");
		nextProfile.revision = 2;
		nextProfile.physical.color_temperature_kelvin = 6_500;
		nextProfile.modes[0].id = "mode-touring";
		nextProfile.modes[0].name = "Touring";
		nextProfile.modes[0].channels = [];
		server.fixtureProfiles = [nextProfile];
		server.patch.fixtures = [fixture];
		patchFeature.updateFixture.mockImplementation(
			async (fixtureId: string, changes: Partial<PatchedFixture>) => {
				const index = server.patch.fixtures.findIndex(
					(candidate) => candidate.fixture_id === fixtureId,
				);
				if (index < 0) return false;
				server.patch.fixtures[index] = {
					...server.patch.fixtures[index],
					...structuredClone(changes),
				};
				return true;
			},
		);
		state.patchSetArmed = true;
		const rendered = render(<FixturePatchSetup />);

		fireEvent.click(
			screen.getByRole("button", { name: /Fixture and mode 17:/ }),
		);
		const dialog = screen
			.getByRole("heading", { name: "Set fixture mode" })
			.closest("section") as HTMLElement;
		expect(
			within(dialog).getByRole("button", { name: "Touring · 4ch" }),
		).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Set" }));

		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledOnce(),
		);
		const changes = patchFeature.updateFixture.mock.calls[0]?.[1];
		expect(changes?.definition).toMatchObject({
			revision: 2,
			mode: "Touring",
			mode_id: "mode-touring",
			profile_snapshot: {
				revision: 2,
				physical: { color_temperature_kelvin: 6_500 },
			},
		});
		expect(changes).not.toHaveProperty("installed_appearance");
		expect(changes?.multipatch?.[0]?.installed_appearance).toEqual(
			originalCopyAppearance,
		);
		expect(server.patch.fixtures[0].installed_appearance).toEqual(
			originalRootAppearance,
		);
		expect(
			server.patch.fixtures[0].multipatch?.[0]?.installed_appearance,
		).toEqual(originalCopyAppearance);

		rendered.rerender(<FixturePatchSetup />);
		const root = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		const copy = screen.getByRole("row", {
			name: "Multi-patch Opposite hang",
		}) as HTMLTableRowElement;
		expect(root.cells[8]).toHaveTextContent(
			"Profile default · 6,500 KR80 · Primary Blue",
		);
		expect(copy.cells[8]).toHaveTextContent("LED · 5,600 KSteel Blue");
	});

	it("copies the primary installed appearance into a new independent physical copy", async () => {
		const fixture = appearanceFixture();
		fixture.bracket_angle = 35;
		fixture.shaper_angle = -15;
		server.patch.fixtures = [fixture];
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("row", { name: /17 Split Wash 17/ }));
		const add = screen.getByRole("button", { name: "+ Add multi-patch" });
		await waitFor(() => expect(add).toBeEnabled());
		fireEvent.click(add);

		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledWith(
				"fixture-split",
				expect.objectContaining({
					multipatch: expect.arrayContaining([
						expect.objectContaining({
							name: "multi-patch",
							bracket_angle: 35,
							shaper_angle: -15,
							installed_appearance: fixture.installed_appearance,
						}),
					]),
				}),
			),
		);
		const update = patchFeature.updateFixture.mock.calls.at(-1)?.[1];
		const created = update?.multipatch?.at(-1);
		expect(created?.installed_appearance).not.toBe(
			fixture.installed_appearance,
		);
	});

	it("updates only the addressed multi-patch and preserves its selected gel", async () => {
		server.patch.fixtures = [appearanceFixture()];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(
			screen.getByRole("button", {
				name: /Light source Opposite hang:/,
			}),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Set light source Opposite hang",
		});
		fireEvent.click(within(dialog).getByRole("button", { name: "LED" }));
		fireEvent.click(screen.getByRole("option", { name: "Tungsten" }));
		fireEvent.change(within(dialog).getByLabelText("Color temperature (K)"), {
			target: { value: "3200" },
		});
		fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				"physical-copy",
				{
					type: "set_installed_appearance",
					appearance: {
						lightSource: { type: "tungsten" },
						colorTemperatureKelvin: 3_200,
						gel: {
							type: "custom",
							name: "Steel Blue",
							colorSrgb: "#6699CC",
							note: "Front truss",
						},
						shaperAnglesDegrees: [10, 20, 30, 40],
					},
				},
			),
		);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PATCH_ARMED",
			value: false,
		});
	});

	it("requires whole in-range kelvin and addresses the primary physical instance", async () => {
		const fixture = appearanceFixture();
		const profile = fixture.definition.profile_snapshot;
		if (!profile) throw new Error("appearance fixture profile is missing");
		profile.physical.color_temperature_kelvin = null;
		server.patch.fixtures = [fixture];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		const dialog = screen.getByRole("dialog", {
			name: "Set light source 17",
		});
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Profile default" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Discharge" }));
		expect(
			within(dialog).getByRole("button", { name: "Apply" }),
		).toBeDisabled();
		expect(within(dialog).getByRole("alert")).toHaveTextContent(
			"requires a color temperature",
		);

		fireEvent.change(within(dialog).getByLabelText("Color temperature (K)"), {
			target: { value: "3200.5" },
		});
		expect(within(dialog).getByRole("alert")).toHaveTextContent("whole number");
		fireEvent.change(within(dialog).getByLabelText("Color temperature (K)"), {
			target: { value: "999" },
		});
		expect(within(dialog).getByRole("alert")).toHaveTextContent(
			"1,000 K to 25,000 K",
		);
		fireEvent.change(within(dialog).getByLabelText("Color temperature (K)"), {
			target: { value: "4200" },
		});
		expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
		expect(within(dialog).getByRole("button", { name: "Apply" })).toBeEnabled();
		fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_installed_appearance",
					appearance: {
						lightSource: { type: "discharge" },
						colorTemperatureKelvin: 4_200,
						gel: { type: "open_white" },
						shaperAnglesDegrees: [1, 2, 3, 4],
					},
				},
			),
		);
	});

	it("keeps Apply disabled for a no-op, discards Close changes, and marks emitterless fixtures unavailable", () => {
		const fixture = appearanceFixture();
		server.patch.fixtures = [fixture];
		state.patchSetArmed = true;
		const { rerender } = render(<FixturePatchSetup />);

		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		let dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		expect(
			within(dialog).getByRole("button", { name: "Apply" }),
		).toBeDisabled();
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Profile default" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Halogen" }));
		expect(within(dialog).getByRole("button", { name: "Apply" })).toBeEnabled();
		fireEvent.click(
			within(dialog).getByRole("button", {
				name: "Close light source editor",
			}),
		);
		expect(
			screen.queryByRole("dialog", { name: "Set light source 17" }),
		).not.toBeInTheDocument();
		expect(patchFeature.updateFixtureIntent).not.toHaveBeenCalled();

		state.patchSetArmed = true;
		fireEvent.click(screen.getByRole("button", { name: /Light source 17:/ }));
		dialog = screen.getByRole("dialog", { name: "Set light source 17" });
		expect(
			within(dialog).getByRole("button", { name: "Profile default" }),
		).toBeInTheDocument();
		fireEvent.click(
			within(dialog).getByRole("button", {
				name: "Close light source editor",
			}),
		);

		const profile = fixture.definition.profile_snapshot;
		if (!profile) throw new Error("appearance fixture profile is missing");
		profile.modes[0].geometry.emitters = [];
		rerender(<FixturePatchSetup />);
		const row = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		expect(row.cells[8]).toHaveTextContent("UnavailableNo geometry emitter");
		expect(within(row.cells[8]).queryByRole("button")).not.toBeInTheDocument();
	});
});

describe("selected split selection and SET editing", () => {
	it("orders title actions edit-first through Preview Stage and supports additive and range selection", () => {
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
				.slice(0, 7)
				.map((button) => button.textContent),
		).toEqual([
			"+ Add layer",
			"+ Add fixture",
			"+ Add multi-patch",
			"Delete",
			"Fixtures",
			"Media Servers",
			"Preview Stage",
		]);
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
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				split_patches: [
					{ split: 1, universe: 1, address: 101 },
					{ split: 3, universe: 4, address: 401 },
				],
				universe: 1,
				address: 101,
			}),
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
			'.stacked-modal-layer[data-modal-top="true"]',
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
		const noDmx = screen.getByText("No DMX");
		expect(noDmx.tagName).toBe("SMALL");
		expect(noDmx).toHaveClass("fixture-mode-no-dmx");
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
	it("spreads a right-clicked location axis across the ordered selection", async () => {
		const first = splitFixture();
		const second = splitFixture();
		second.fixture_id = "fixture-second";
		second.fixture_number = 18;
		second.name = "Split Wash 18";
		second.address = 105;
		second.split_patches = [
			{ split: 1, universe: 1, address: 105 },
			{ split: 3, universe: 2, address: 205 },
		];
		server.patch.fixtures = [first, second];
		programming.selection.selected = ["fixture-second", "fixture-split"];
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		fireEvent.contextMenu(within(fixtureRow.cells[9]).getByRole("button"));

		const modal = screen.getByRole("dialog", {
			name: "Location X (meter)",
		});
		for (const key of ["−", "4", "THRU", "−", "3"])
			fireEvent.click(within(modal).getByRole("button", { name: key }));
		fireEvent.click(within(modal).getByRole("button", { name: "ENTER" }));

		await waitFor(() =>
			expect(patchFeature.spreadFixtureVector).toHaveBeenCalledWith({
				fixtureIds: ["fixture-second", "fixture-split"],
				kind: "location",
				axis: "x",
				points: [-4000, -3000],
			}),
		);
	});

	it("applies one location value to every fixture in the ordered selection", async () => {
		const first = splitFixture();
		const second = splitFixture();
		second.fixture_id = "fixture-second";
		second.fixture_number = 18;
		second.name = "Split Wash 18";
		server.patch.fixtures = [first, second];
		programming.selection.selected = ["fixture-second", "fixture-split"];
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		fireEvent.contextMenu(within(fixtureRow.cells[10]).getByRole("button"));

		const modal = screen.getByRole("dialog", {
			name: "Location Y (meter)",
		});
		for (const key of ["3", "ENTER"])
			fireEvent.click(within(modal).getByRole("button", { name: key }));

		await waitFor(() =>
			expect(patchFeature.spreadFixtureVector).toHaveBeenCalledWith({
				fixtureIds: ["fixture-second", "fixture-split"],
				kind: "location",
				axis: "y",
				points: [3000, 3000],
			}),
		);
	});

	it("opens the location value pad directly and confirms unsaved input", async () => {
		const { current } = fixturesWithConflict();
		server.patch.fixtures = [current];
		state.patchSetArmed = true;
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		fireEvent.click(within(fixtureRow.cells[9]).getByRole("button"));

		const modal = screen.getByRole("dialog", {
			name: "Location X (meter)",
		});
		fireEvent.click(within(modal).getByRole("button", { name: "1" }));
		fireEvent.click(
			within(modal).getByRole("button", { name: "Close Location X (meter)" }),
		);
		const confirmation = screen.getByRole("dialog", {
			name: "Unsaved Location X (meter) changes",
		});
		fireEvent.click(
			within(
				confirmation.querySelector(".ui-input-unsaved-actions") as HTMLElement,
			).getByRole("button", { name: "Stay in modal" }),
		);
		fireEvent.click(within(modal).getByRole("button", { name: "ENTER" }));
		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_location_axis",
					axis: "x",
					millimetres: 1000,
				},
			),
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
		fireEvent.click(within(fixtureRow.cells[13]).getByRole("button"));

		const modal = screen.getByRole("dialog", {
			name: "Rotation Y (degree)",
		});
		expect(
			within(modal).queryByRole("dialog", { name: "Rotation X (degree)" }),
		).toBeNull();
		for (const key of ["4", "5", "ENTER"])
			fireEvent.click(within(modal).getByRole("button", { name: key }));
		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				null,
				{
					type: "set_rotation_axis",
					axis: "y",
					degrees: 45,
				},
			),
		);
	});

	it("uses the exact sixteen-column grid for primary and multi-patch rows", () => {
		server.patch.fixtures = [policyFixture()];
		render(<FixturePatchSetup />);
		expect(
			screen.getAllByRole("columnheader").map((header) => header.textContent),
		).toEqual([
			"Type",
			"Fixture ID",
			"Name",
			"Fixture / mode",
			"Patch",
			"Masters",
			"Pan / Tilt",
			"MIB",
			"Light source",
			"Location X",
			"Location Y",
			"Location Z",
			"Rotation X",
			"Rotation Y",
			"Rotation Z",
			"Layer",
		]);
		const primary = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		const multi = screen.getByRole("row", {
			name: "Multi-patch Opposite hang",
		}) as HTMLTableRowElement;
		expect(primary.cells).toHaveLength(16);
		expect(multi.cells).toHaveLength(16);
		expect(multi.cells[1]).toHaveTextContent(/^—$/);
		expect(multi.cells[2]).toHaveTextContent(/^—$/);
		expect(multi.cells[4]).toHaveTextContent("S1 3.1 · S3 4.1");
		fireEvent.click(primary);
		expect(patchFeature.selectPatchInstance).toHaveBeenLastCalledWith({
			fixtureId: "fixture-split",
			multipatchInstanceId: null,
		});
		fireEvent.click(multi);
		expect(patchFeature.selectPatchInstance).toHaveBeenLastCalledWith({
			fixtureId: "fixture-split",
			multipatchInstanceId: "physical-copy",
		});
		for (const retired of [
			"Manufacturer",
			"Product / mode",
			"Group Masters",
			"Grand Master",
			"Invert Pan",
			"Invert Tilt",
			"MIB Delay",
			"Bracket",
			"Shaper",
		])
			expect(
				screen.queryByRole("columnheader", { name: retired }),
			).not.toBeInTheDocument();
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
		fireEvent.click(within(instanceRow.cells[13]).getByRole("button"));

		const modal = screen.getByRole("dialog", {
			name: "Rotation Y (degree)",
		});
		for (const key of ["4", "5", "ENTER"])
			fireEvent.click(within(modal).getByRole("button", { name: key }));
		await waitFor(() =>
			expect(patchFeature.updateFixtureIntent).toHaveBeenCalledWith(
				"fixture-split",
				"current-mp",
				{
					type: "set_rotation_axis",
					axis: "y",
					degrees: 45,
				},
			),
		);
	});

	it("spreads a physical transform from the primary patch through selected multi-patches", async () => {
		const { current } = fixturesWithConflict();
		const first = current.multipatch?.[0] as MultiPatchInstance;
		current.rotation = { x: 10, y: 20, z: 30 };
		current.multipatch = [
			{ ...first, rotation: { x: 1, y: 2, z: 3 } },
			{
				...first,
				id: "second-mp",
				name: "Second duplicate",
				rotation: { x: 4, y: 5, z: 6 },
			},
		];
		server.patch.fixtures = [current];
		render(<FixturePatchSetup />);
		const fixtureRow = screen.getByRole("row", {
			name: /17 Split Wash 17/,
		}) as HTMLTableRowElement;
		const instances = [...document.querySelectorAll("tr.multipatch-row")];
		fireEvent.click(fixtureRow);
		fireEvent.click(instances[1], { shiftKey: true });
		fireEvent.contextMenu(
			within((instances[1] as HTMLTableRowElement).cells[13]).getByRole(
				"button",
			),
		);

		const modal = screen.getByRole("dialog", {
			name: "Rotation Y (degree)",
		});
		for (const key of ["−", "1", "8", "THRU", "1", "8", "ENTER"])
			fireEvent.click(within(modal).getByRole("button", { name: key }));

		await waitFor(() =>
			expect(patchFeature.updateFixture).toHaveBeenCalledWith("fixture-split", {
				rotation: { x: 10, y: -18, z: 30 },
				multipatch: [
					expect.objectContaining({
						id: "current-mp",
						rotation: { x: 1, y: 0, z: 3 },
					}),
					expect.objectContaining({
						id: "second-mp",
						rotation: { x: 4, y: 18, z: 6 },
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
