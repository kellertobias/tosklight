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
import type { CueList, FixtureDefinition, PatchedFixture } from "../api/types";
import type {
	ShowObject,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import { ShowObjectsStore } from "../features/showObjects/store";
import { ShowObjectsViewProvider } from "../features/showObjects/ShowObjectsView";
import type {
	ShowObjectsEventObserver,
	ShowObjectsEventScope,
	ShowObjectsEventTransport,
} from "../features/showObjects/transport";
import { FixtureSheetWindow } from "./FixtureSheetWindow";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const REPLACEMENT_SHOW_ID = "22222222-2222-4222-8222-222222222222";

function patchedFixture(fixtureId: string, number: number): PatchedFixture {
	return {
		fixture_id: fixtureId,
		fixture_number: number,
		name: `Fixture ${number}`,
		universe: 1,
		address: number,
		definition: {
			schema_version: 1,
			id: "definition",
			revision: 1,
			manufacturer: "Test",
			device_type: "dimmer",
			name: "Test Par",
			model: "Test Par",
			mode: "1 channel",
			footprint: 1,
			heads: [
				{
					index: 0,
					name: "Base",
					shared: false,
					parameters: [
						{
							attribute: "intensity",
							components: [],
							default: 0,
							virtual_dimmer: false,
							capabilities: [],
						},
					],
				},
			],
			color_calibration: null,
			physical: {},
			icon_asset: null,
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
		} as FixtureDefinition,
		logical_heads: [],
	};
}

function cueListObject(
	id: string,
	name: string,
	fixtureIds: readonly string[],
): ShowObject<"cue_list"> {
	const body: CueList = {
		id,
		name,
		mode: "sequence",
		priority: 0,
		looped: false,
		cues: [
			{
				number: 1,
				name: "Opening",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: fixtureIds.map((fixture_id) => ({
					fixture_id,
					attribute: "intensity",
					value: { kind: "normalized" as const, value: 1 },
				})),
			},
		],
	};
	return { kind: "cue_list", id, revision: 1, updated_at: "", body };
}

/**
 * Delivered unsorted; both the server snapshot (`ORDER BY id`) and the scoped
 * Show Objects collection present Cuelists in storage-identity order, so the
 * picker order is unchanged by the migration.
 */
const CUE_LISTS = [
	cueListObject("cl-b", "Bows", ["fix-1"]),
	cueListObject("cl-a", "Act one", ["fix-2"]),
];

const broadReads = { pool: 0, cueLists: 0 };

const patchFixtures = vi.hoisted(
	() => ({ current: [] as Record<string, unknown>[] }),
);
vi.mock("../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	usePatchedFixturesView: (enabled = true) =>
		enabled ? patchFixtures.current : [],
}));

patchFixtures.current = [
	patchedFixture("fix-1", 1),
	patchedFixture("fix-2", 2),
] as never;
const server = {
	bootstrap: { active_programmers: [] },
	session: { session_id: "session-a" },
	patch: {
		get fixtures() {
			return patchFixtures.current;
		},
	},
	groups: [],
	playbacks: {
		get pool() {
			broadReads.pool += 1;
			return [];
		},
		get cue_lists() {
			broadReads.cueLists += 1;
			return [];
		},
		authoritative_controls: { groups: [] },
	},
	highlight: null,
	readVisualization: vi.fn().mockResolvedValue({ values: [] }),
	selectionGesture: vi.fn().mockResolvedValue(undefined),
};

const state = {
	fixtureGroupsVisible: false,
	fixtureSheetOrder: "fixture-id" as const,
	fixtureSheetActiveOnly: false,
	fixtureSheetCueListId: "",
	fixtureSheetColumns: ["id", "name"] as ("id" | "name")[],
	fixtureSheetShowType: false,
	fixtureSheetIncludedHeads: "all" as const,
};
const dispatch = vi.fn();

vi.mock("../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: () => ({ selected: [] }),
	useProgrammingSelectionActions: () => ({ gesture: vi.fn() }),
}));
vi.mock(
	"../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView",
	() => ({
		useProgrammerPreloadLifecycleView: () => ({
			ready: true,
			armed: false,
			active: false,
			pending: false,
			phase: "idle",
			error: null,
			actions: null,
		}),
	}),
);
vi.mock("../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));
vi.mock("../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));
vi.mock("../components/shared/GroupStrip", () => ({ GroupStrip: () => null }));
vi.mock("../components/shared/SourceLegend", () => ({ SourceLegend: () => null }));

class FakeTransport implements ShowObjectsEventTransport {
	readonly subscriptions: ShowObjectsEventScope[] = [];

	subscribe(
		_showId: string,
		scope: ShowObjectsEventScope,
		_afterSequence: number | null,
		_observer: ShowObjectsEventObserver,
	) {
		this.subscriptions.push(scope);
		return { close: vi.fn(), repair: vi.fn() };
	}
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

function harness(children: ReactNode) {
	const transport = new FakeTransport();
	const store = new ShowObjectsStore();
	const pending: Array<{
		kind: ShowObjectKind;
		resolve: (value: { objects: ShowObject[]; showRevision: number }) => void;
	}> = [];
	const loadCollection = vi.fn((_showId: string, kind: ShowObjectKind) => {
		const gate = deferred<{ objects: ShowObject[]; showRevision: number }>();
		pending.push({ kind, resolve: gate.resolve });
		return gate.promise;
	});
	const tree = (showId: string, content: ReactNode) => (
		<ShowObjectsViewProvider
			showId={showId}
			store={store}
			transport={transport}
			loadCollection={loadCollection}
			loadObject={vi.fn()}
		>
			{content}
		</ShowObjectsViewProvider>
	);
	const view = render(tree(SHOW_ID, children));
	const kinds = () => loadCollection.mock.calls.map(([, kind]) => kind);
	const settle = async (cueLists: ShowObject[] = CUE_LISTS) => {
		await waitFor(() => expect(pending.length).toBeGreaterThan(0));
		for (const run of pending.splice(0))
			run.resolve({
				objects: run.kind === "cue_list" ? cueLists : [],
				showRevision: 1,
			});
	};
	return {
		kinds,
		loadCollection,
		pending,
		rerender: (showId: string, content: ReactNode) =>
			view.rerender(tree(showId, content)),
		settle,
		transport,
		view,
	};
}

function fixtureRowIds(container: HTMLElement) {
	return [...container.querySelectorAll("[data-fixture-id]")].map((row) =>
		row.getAttribute("data-fixture-id"),
	);
}

function openSettings() {
	fireEvent.click(screen.getByRole("button", { name: "Settings" }));
}

const CUELIST_FILTER_LABEL = "Fixture sheet Cuelist filter";

function openCuelistFilter() {
	const label = [...document.querySelectorAll("label")].find(
		(candidate) => candidate.textContent === CUELIST_FILTER_LABEL,
	);
	const field = label?.closest(".ui-form-field") as HTMLElement;
	const trigger = within(field).getByRole("button");
	const selected = trigger.textContent?.replace("▼", "") ?? "";
	fireEvent.click(trigger);
	return {
		label: selected,
		options: within(
			screen.getByRole("listbox", { name: CUELIST_FILTER_LABEL }),
		)
			.getAllByRole("option")
			.map((option) => option.textContent),
	};
}

beforeEach(() => {
	dispatch.mockReset();
	server.readVisualization.mockClear().mockResolvedValue({ values: [] });
	state.fixtureSheetCueListId = "";
	broadReads.pool = 0;
	broadReads.cueLists = 0;
});

afterEach(cleanup);

describe("Fixture Sheet Cuelist authority", () => {
	it("requests no Cuelist snapshot or socket before a Fixture Sheet mounts", async () => {
		const { loadCollection, transport } = harness(null);

		await Promise.resolve();

		expect(loadCollection).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("stays dormant while the Fixture Sheet is inactive", async () => {
		const { kinds, transport } = harness(<FixtureSheetWindow active={false} />);

		await Promise.resolve();

		expect(kinds()).not.toContain("cue_list");
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("opens no Cuelist scope for a compact Fixture Sheet pane", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { kinds, transport, view } = harness(<FixtureSheetWindow compact />);

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-1", "fix-2"]),
		);

		expect(kinds()).not.toContain("cue_list");
		expect(
			transport.subscriptions.some((scope) =>
				scope.kinds.includes("cue_list"),
			),
		).toBe(false);
	});

	it("filters fixtures from the scoped Cuelist named by the saved setting", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { kinds, settle, view } = harness(<FixtureSheetWindow />);

		await settle();

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-2"]),
		);
		expect(kinds()).toContain("cue_list");
		expect(broadReads.cueLists).toBe(0);
		expect(broadReads.pool).toBe(0);
	});

	it("lists scoped Cuelist names in authoritative storage-identity order", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { settle } = harness(<FixtureSheetWindow />);
		await settle();
		await waitFor(() => expect(screen.getByRole("button", { name: "Settings" })).toBeInTheDocument());

		openSettings();

		expect(openCuelistFilter()).toEqual({
			label: "Act one",
			options: ["All fixtures", "Act one", "Bows"],
		});
	});

	it("treats a deleted selected Cuelist as All fixtures without rewriting the setting", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { settle, view } = harness(<FixtureSheetWindow />);

		await settle([cueListObject("cl-b", "Bows", ["fix-1"])]);

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-1", "fix-2"]),
		);
		openSettings();
		expect(openCuelistFilter().label).toBe("All fixtures");
		expect(dispatch).not.toHaveBeenCalled();
	});

	it("never exposes cached Cuelists while a replacement scope is loading", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { rerender, settle, view } = harness(<FixtureSheetWindow />);
		await settle();
		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-2"]),
		);

		rerender(REPLACEMENT_SHOW_ID, <FixtureSheetWindow />);

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-1", "fix-2"]),
		);
		openSettings();
		expect(openCuelistFilter()).toEqual({
			label: "All fixtures",
			options: ["All fixtures"],
		});
		expect(dispatch).not.toHaveBeenCalled();

		await settle();

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-2"]),
		);
	});

	it("keeps the saved choice and re-applies it once replacement authority arrives", async () => {
		state.fixtureSheetCueListId = "cl-a";
		const { rerender, settle, view } = harness(<FixtureSheetWindow />);
		await settle();
		rerender(REPLACEMENT_SHOW_ID, <FixtureSheetWindow />);

		await settle([cueListObject("cl-a", "Act one", ["fix-1"])]);

		await waitFor(() =>
			expect(fixtureRowIds(view.container)).toEqual(["fix-1"]),
		);
		expect(state.fixtureSheetCueListId).toBe("cl-a");
		expect(dispatch).not.toHaveBeenCalled();
	});
});
