import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { PreloadStoreModal } from "./PreloadStoreModal";

const CUE_LIST_ID = "11111111-1111-4111-8111-111111111111";

const mocks = vi.hoisted(() => ({
	state: { preloadStoreOpen: true },
	fixtures: [] as PatchedFixture[],
	topology: {
		ready: true,
		playbacks: [] as Array<{
			body: {
				number: number;
				name: string;
				target?: { type: string; cue_list_id: string };
			};
		}>,
		pages: [] as Array<{
			body: {
				number: number;
				assignments: Record<string, { name: string }>;
			};
		}>,
	},
	dispatch: vi.fn(),
	storePreload: vi.fn(),
	recordCue: vi.fn(),
	views: vi.fn(),
	removeDynamic: vi.fn().mockResolvedValue({ status: "changed" }),
	removeGroupRelease: vi.fn().mockResolvedValue({ status: "changed" }),
	removePlayback: vi.fn().mockResolvedValue({ status: "changed" }),
	releaseFixture: vi.fn().mockResolvedValue({ status: "changed" }),
	releaseGroup: vi.fn().mockResolvedValue({ status: "changed" }),
	values: {
		revision: 8,
		fixtureValues: [
			{
				fixtureId: "fixture-a",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
				programmerOrder: 2,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		],
		groupValues: [
			{
				groupId: "3",
				attribute: "pan",
				value: { kind: "normalized", value: 0.25 },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		],
	},
	queue: {
		revision: 12,
		actions: [
			{
				address: { page: 1, position: 4 },
				action: "go",
				controlSource: { surface_id: "physical:test", control_id: 4 },
			},
			{
				address: { page: 1, position: 4 },
				action: "go",
				controlSource: { surface_id: "physical:test", control_id: 4 },
			},
		],
	},
	presets: [
		{
			kind: "preset",
			id: "1",
			revision: 4,
			updated_at: "",
			body: { name: "Blue", number: 1, family: "Color", values: {} },
		},
	],
	cueLists: [
		{
			kind: "cue_list",
			id: "11111111-1111-4111-8111-111111111111",
			revision: 7,
			updated_at: "",
			body: { name: "Main", cues: [] },
		},
	],
}));

vi.mock(
	"../../features/programmerActions/ProgrammerActionsContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useProgrammerActions: () => ({ storePreload: mocks.storePreload }),
	}),
);
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePresets: () => mocks.presets,
	useCueLists: () => mocks.cueLists,
	usePortableGroups: () => [{ id: "3", body: { number: 3, name: "Front" } }],
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: mocks.views,
}));

vi.mock("../../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackTopologyView: () => mocks.topology,
}));

vi.mock("../../features/patch/PatchState", () => ({
	usePatchedFixturesView: () => mocks.fixtures,
}));
vi.mock(
	"../../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({
		useProgrammerPreloadInspectionValuesView: () => mocks.values,
		useProgrammerPreloadValuesActions: () => ({
			releaseFixtureValue: mocks.releaseFixture,
			releaseGroupValue: mocks.releaseGroup,
		}),
	}),
);
vi.mock(
	"../../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView",
	() => ({ useProgrammerPreloadPlaybackQueueView: () => mocks.queue }),
);
vi.mock(
	"../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView",
	() => ({
		useProgrammerPreloadLifecycleView: () => ({
			ready: true,
			pending: false,
			error: null,
			actions: {
				removePendingFixtureValue: mocks.releaseFixture,
				removePendingGroupValue: mocks.releaseGroup,
				removePendingDynamic: mocks.removeDynamic,
				removePendingGroupRelease: mocks.removeGroupRelease,
				removePendingPlayback: mocks.removePlayback,
			},
		}),
	}),
);

const originalValues = structuredClone(mocks.values);
beforeEach(() => {
	for (const key of Object.keys(mocks.values))
		delete (mocks.values as Record<string, unknown>)[key];
	Object.assign(mocks.values, structuredClone(originalValues));
	mocks.fixtures = [
		{
			fixture_id: "fixture-a",
			fixture_number: 7,
			name: "Wash",
			logical_heads: [],
			definition: { name: "Wash", model: "Wash", heads: [] },
		} as unknown as PatchedFixture,
	];
	mocks.topology = { ready: true, playbacks: [], pages: [] };
	mocks.queue.actions = [
		{
			address: { page: 2, position: 4 },
			action: "go",
			controlSource: { surface_id: "physical:test", control_id: 4 },
		},
		{
			address: { page: 2, position: 4 },
			action: "go",
			controlSource: { surface_id: "physical:test", control_id: 4 },
		},
	];
	mocks.removeDynamic.mockClear();
	mocks.removeGroupRelease.mockClear();
	mocks.state.preloadStoreOpen = true;
	mocks.dispatch.mockClear();
	mocks.storePreload.mockReset();
	mocks.storePreload.mockResolvedValue(true);
	mocks.recordCue.mockReset();
	mocks.recordCue.mockResolvedValue({ status: "changed" });
	mocks.views.mockClear();
	mocks.removePlayback.mockClear();
	mocks.releaseFixture.mockClear();
	mocks.releaseGroup.mockClear();
});

afterEach(cleanup);

describe("PreloadStoreModal", () => {
	it("shows one plain list with Remove actions and only the title close", () => {
		render(<PreloadStoreModal />);
		expect(screen.getAllByRole("list")).toHaveLength(1);
		expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(4);
		for (const button of screen.getAllByRole("button", { name: /^Remove / }))
			expect(button).toHaveTextContent(/^Remove$/);
		expect(screen.queryByText("Programmer changes")).not.toBeInTheDocument();
		expect(screen.queryByText("Playback actions")).not.toBeInTheDocument();
		expect(screen.queryByText(/Inspect or remove pending work/)).not.toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Record pending…" })).not.toBeInTheDocument();
		expect(screen.queryByRole("button", { name: /^Close$/ })).not.toBeInTheDocument();
	});
	it("closes through the existing title close without changing pending authority", () => {
		render(<PreloadStoreModal />);
		fireEvent.click(screen.getByRole("button", { name: /^Close modal$/ }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL", modal: "preloadStoreOpen", value: false,
		});
		expect(mocks.removePlayback).not.toHaveBeenCalled();
		expect(mocks.releaseFixture).not.toHaveBeenCalled();
		expect(mocks.releaseGroup).not.toHaveBeenCalled();
	});
	it("shows exactly No pending changes when both authorities are empty", () => {
		mocks.values.fixtureValues = [];
		mocks.values.groupValues = [];
		mocks.queue.actions = [];
		render(<PreloadStoreModal />);
		expect(screen.getByText("No pending changes.")).toBeInTheDocument();
		expect(screen.queryByRole("listitem")).not.toBeInTheDocument();
		expect(screen.queryByRole("button", { name: /^Remove / })).not.toBeInTheDocument();
	});
	it("opens pending inspection without recording on the hold entry", () => {
		render(<PreloadStoreModal />);
		expect(screen.getByText("Pending Preload")).toBeInTheDocument();
		expect(
			screen.queryByText("Record Pending Preload"),
		).not.toBeInTheDocument();
		expect(mocks.storePreload).not.toHaveBeenCalled();
		expect(mocks.recordCue).not.toHaveBeenCalled();
	});
	it("lists authored order and removes only the clicked duplicate at the displayed revision", async () => {
		render(<PreloadStoreModal />);
		const labels = screen
			.getAllByRole("listitem")
			.map((row) => row.textContent);
		expect(labels[0]).toContain("Group 3 · Front · pan · 25%");
		expect(labels[1]).toContain("Fixture 7 · Wash · intensity · 50%");
		expect(labels[2]).toContain("Page 2 Playback 4 · GO");
		expect(labels.join(" ")).not.toContain("physical:test");
		fireEvent.click(
			screen.getByRole("button", { name: "Remove playback action 2" }),
		);
		await waitFor(() =>
			expect(mocks.removePlayback).toHaveBeenCalledWith(1, 12),
		);
		expect(mocks.recordCue).not.toHaveBeenCalled();
		expect(mocks.storePreload).not.toHaveBeenCalled();
	});

	it("labels the captured exact address without substituting its cue target", () => {
		mocks.topology.playbacks = [
			{
				body: {
					number: 11,
					name: "White base",
					target: { type: "cue_list", cue_list_id: "cue-99" },
				},
			},
		];
		mocks.topology.pages = [
			{ body: { number: 1, assignments: { "11": { name: "White base" } } } },
		];
		mocks.queue.actions = [
			{
				address: { page: 1, position: 11 },
				action: "on",
				controlSource: { surface_id: "physical:test", control_id: 11 },
			},
		];
		render(<PreloadStoreModal />);
		expect(
			screen.getByText(
				"Page 1 Playback 11 · current name: White base · ON",
			),
		).toBeInTheDocument();
		expect(
			screen.queryByText(/Cuelist 11|Playback 2\.1|cue-99/),
		).not.toBeInTheDocument();
	});

	it("resolves names only from the captured page and preserves address fallback", () => {
		mocks.topology.playbacks = [
			{ body: { number: 1001, name: "Wrong pool name" } },
		];
		mocks.topology.pages = [
			{
				body: {
					number: 1,
					assignments: { "401": { name: "Blue look" } },
				},
			},
		];
		mocks.queue.actions = [
			{
				address: { page: 1, position: 401 },
				action: "go",
				controlSource: { surface_id: "virtual:test", control_id: 1001 },
			},
			{
				address: { page: 1, position: 12 },
				action: "on",
				controlSource: { surface_id: "physical:test", control_id: 12 },
			},
			{
				address: { page: 1, position: 402 },
				action: "on",
				controlSource: { surface_id: "virtual:test", control_id: 1002 },
			},
		];
		render(<PreloadStoreModal />);
		expect(
			screen.getByText(
				"Page 1 Playback 401 · current name: Blue look · GO",
			),
		).toBeInTheDocument();
		expect(
			screen.getByText("Page 1 Playback 12 · ON"),
		).toBeInTheDocument();
		expect(
			screen.getByText("Page 1 Playback 402 · ON"),
		).toBeInTheDocument();
		expect(screen.queryByText(/Wrong pool name/)).not.toBeInTheDocument();
	});

	it("keeps number fallback while topology names are not authoritative", () => {
		mocks.topology.ready = false;
		mocks.topology.playbacks = [{ body: { number: 11, name: "Stale name" } }];
		mocks.queue.actions = [
			{
				address: { page: 1, position: 11 },
				action: "on",
				controlSource: { surface_id: "physical:test", control_id: 11 },
			},
		];
		render(<PreloadStoreModal />);
		expect(
			screen.getByText("Page 1 Playback 11 · ON"),
		).toBeInTheDocument();
		expect(screen.queryByText(/Stale name/)).not.toBeInTheDocument();
	});

	it("removes fixture and group pending changes through exact displayed lifecycle authority", async () => {
		render(<PreloadStoreModal />);
		fireEvent.click(
			screen.getByRole("button", { name: "Remove programmer change 1" }),
		);
		await waitFor(() =>
			expect(mocks.releaseGroup).toHaveBeenCalledWith("3", "pan", 8),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Remove programmer change 2" }),
			).not.toBeDisabled(),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Remove programmer change 2" }),
		);
		await waitFor(() =>
			expect(mocks.releaseFixture).toHaveBeenCalledWith(
				"fixture-a",
				"intensity",
				8,
			),
		);
	});

	it("keeps both scoped views dormant while closed", () => {
		mocks.state.preloadStoreOpen = false;
		render(<PreloadStoreModal />);

		expect(screen.queryByText("Pending Preload")).not.toBeInTheDocument();
		expect(mocks.views).not.toHaveBeenCalled();
		expect(mocks.recordCue).not.toHaveBeenCalled();
		expect(mocks.storePreload).not.toHaveBeenCalled();
	});


});

it("shows Fixture and Group Release distinctly and removes original unsorted row index", async () => {
	const extra = {
		dynamicValues: [
			{
				fixtureId: "fixture-a",
				attribute: "intensity",
				programmerOrder: 9,
				changedAtMillis: 0,
				value: { type: "release" },
			},
			{
				fixtureId: "fixture-a",
				attribute: "color",
				programmerOrder: 0,
				changedAtMillis: 0,
				value: { type: "release" },
			},
		],
		groupReleaseValues: [
			{
				groupId: "3",
				attribute: "intensity",
				programmerOrder: 8,
				changedAtMillis: 0,
			},
			{
				groupId: "3",
				attribute: "color",
				programmerOrder: 3,
				changedAtMillis: 0,
			},
		],
	};
	Object.assign(mocks.values, extra);
	try {
		render(<PreloadStoreModal />);
		expect(screen.getAllByRole("listitem")[0]).toHaveTextContent(
			"Fixture 7 · Wash · color · Release fixture attribute",
		);
		expect(screen.getAllByRole("listitem")[3]).toHaveTextContent(
			"Group 3 · Front · color · Release group attribute",
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Remove programmer change 0" }),
		);
		await waitFor(() => expect(mocks.removeDynamic).toHaveBeenCalledWith(1, 8));
		fireEvent.click(
			screen.getByRole("button", { name: "Remove programmer change 3" }),
		);
		await waitFor(() =>
			expect(mocks.removeGroupRelease).toHaveBeenCalledWith(1, 8),
		);
		expect(mocks.storePreload).not.toHaveBeenCalled();
	} finally {
		delete (mocks.values as Partial<typeof extra>).dynamicValues;
		delete (mocks.values as Partial<typeof extra>).groupReleaseValues;
	}
});

it("shows pending native controls and static/dynamic release values rather than JSON or counts", () => {
	const original = mocks.values.fixtureValues;
	Object.assign(mocks.values, {
		fixtureValues: [
			{
				...original[0],
				value: {
					kind: "color_program",
					value: {
						kind: "direct",
						recipe: {
							channels: [
								{
									channel_id: "red-channel",
									function_id: "red-function",
									raw: 193,
								},
								{
									channel_id: "white-channel",
									function_id: "white-function",
									raw: 62,
								},
							],
						},
					},
				},
			},
		],
		dynamicValues: [
			{
				fixtureId: "fixture-a",
				attribute: "intensity",
				programmerOrder: 10,
				changedAtMillis: 0,
				value: {
					type: "static",
					value: { kind: "normalized", value: 0.4 },
					timing: { fade_millis: 250 },
				},
			},
			{
				fixtureId: "fixture-a",
				attribute: "color",
				programmerOrder: 11,
				changedAtMillis: 0,
				value: {
					type: "programming_release",
					component: { kind: "color", component: "red" },
				},
			},
		],
	});
	try {
		render(<PreloadStoreModal />);
		expect(
			screen.getByText(/red-channel\/red-function: DMX 193/),
		).toBeInTheDocument();
		expect(
			screen.getByText(/white-channel\/white-function: DMX 62/),
		).toBeInTheDocument();
		expect(screen.getByText(/Static · 40% · Fade 250 ms/)).toBeInTheDocument();
		expect(screen.getByText(/Release Color red/)).toBeInTheDocument();
	} finally {
		mocks.values.fixtureValues = original;
		delete (mocks.values as { dynamicValues?: unknown }).dynamicValues;
	}
});

it("shows semantic spread units, allocation, pinned wheels and Zoom convention", () => {
	const row = mocks.values.fixtureValues[0];
	Object.assign(mocks.values, {
		fixtureValues: [
			{
				...row,
				attribute: "color",
				value: {
					kind: "color_program",
					value: {
						kind: "semantic",
						intent: {
							recipe: { rgb: [1, 0, 0], amber: 0 },
							relative_output: 1,
							white_blend: 0,
							white_target: { kelvin: 6500, duv: 0 },
							uv: { amount: 0 },
							base_xyz: { x: 1, y: 0, z: 0 },
							allocation: "prefer_white",
							spreads: [
								{ component: "red", points: [1, 0] },
								{ component: "hue", points: [350, 10] },
								{ component: "temperature", points: [2700, 6500] },
								{ component: "duv", points: [-0.01, 0.02] },
							],
							wheel_constraints: [
								{
									source: {
										profile_id: "source",
										profile_revision: 2,
										mode_id: "mode",
										head_id: "head",
									},
									value: { channel_id: "wheel", function_id: "slot", raw: 47 },
								},
							],
						},
					},
				},
			},
			{
				...row,
				attribute: "zoom",
				programmerOrder: 3,
				value: {
					kind: "zoom",
					value: {
						opening_degrees: { kind: "value", value: 20 },
						convention: "field",
					},
				},
			},
		],
	});
	render(<PreloadStoreModal />);
	const text = screen
		.getAllByRole("listitem")
		.map((item) => item.textContent)
		.join(" ");
	expect(text).toContain("Spread Color red 100% → 0%");
	expect(text).toContain("Spread Color hue 350° → 10°");
	expect(text).toContain("Spread Color temperature 2700 K → 6500 K");
	expect(text).toContain("Spread Color duv -0.01 → 0.02");
	expect(text).toContain("Allocation prefer white");
	expect(text).toContain("Pinned wheel wheel/slot: DMX 47");
	expect(text).toContain("20° field opening");
});

it("uses ordered operator subfixture IDs and reserved virtual numbering for pending rows", async () => {
	mocks.fixtures = [
		{
			fixture_id: "root",
			fixture_number: 100,
			name: "Bar",
			logical_heads: [
				{ fixture_id: "left", head_index: 2 },
				{ fixture_id: "right", head_index: 7 },
			],
			definition: {
				name: "Bar",
				model: "Bar",
				heads: [
					{ index: 8, shared: true, name: "Base" },
					{ index: 2, shared: false, name: "Left" },
					{ index: 7, shared: false, name: "Right" },
				],
			},
		},
		{
			fixture_id: "virtual",
			fixture_number: null,
			virtual_fixture_number: 3,
			name: "Venue",
			logical_heads: [],
			definition: { name: "Venue", model: "Venue", heads: [] },
		},
	] as unknown as PatchedFixture[];
	const row = mocks.values.fixtureValues[0];
	Object.assign(mocks.values, {
		fixtureValues: ["left", "right", "virtual"].map((fixtureId, index) => ({
			...row,
			fixtureId,
			programmerOrder: index + 4,
		})),
	});
	render(<PreloadStoreModal />);
	expect(screen.getByText(/Fixture 100.1 · Bar · Left/)).toBeInTheDocument();
	expect(screen.getByText(/Fixture 100.2 · Bar · Right/)).toBeInTheDocument();
	expect(screen.getByText(/Fixture 0.3 · Venue/)).toBeInTheDocument();
	fireEvent.click(
		screen.getByRole("button", { name: "Remove programmer change 4" }),
	);
	await waitFor(() =>
		expect(mocks.releaseFixture).toHaveBeenCalledWith("left", "intensity", 8),
	);
});
