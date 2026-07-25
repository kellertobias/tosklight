import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	SelectionActionOutcome,
	SelectionActionRequest,
	SelectionProjection,
	ProgrammingSnapshot,
} from "../features/programmingInteraction/contracts";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	FIXTURE_3,
	programmingSnapshot,
	selection,
	selectionChange,
	SHOW_ID,
} from "../features/programmingInteraction/testFixtures";
import type { StageSelectionModel } from "./stageWindow/useStageSelection";
import { StageWindow } from "./StageWindow";

const legacyServer = vi.hoisted(() => ({ use: vi.fn() }));

vi.mock("../api/ServerContext", () => ({ useServer: legacyServer.use }));
vi.mock("./stageWindow/useStageLayout", () => ({
	useStageLayout: () => ({
		positions: {},
		positions3d: {},
		updatePosition2d: vi.fn(),
		updatePosition3d: vi.fn(),
		save: vi.fn(),
		savePosition3d: vi.fn(),
	}),
}));
vi.mock("./stageWindow/useStageOptions", () => ({
	useStageOptions: () => ({
		mode: "select",
		setMode: vi.fn(),
		view: "2d",
		setView: vi.fn(),
		followPreload: false,
		toggleFollowPreload: vi.fn(),
		groupsVisible: false,
		showSelection: true,
		showFloorGrid: true,
		showBeamGuides: true,
		environmentBrightness: 1,
	}),
}));
vi.mock("./stageWindow/useStageVisualization", () => ({
	useStageVisualization: () => ({
		visualization: null,
		fixtures: [],
		fixtures3d: [],
		patchPreviewFixtures: [],
	}),
}));
vi.mock("./stageWindow/Stage2dView", () => ({
	Stage2dView: ({ selection: stageSelection }: { selection: StageSelectionModel }) => (
		<div data-testid="stage-selection" data-selection={stageSelection.fixtureIds.join(",")}>
			<button
				type="button"
				onClick={() => void stageSelection.applyFixtureGesture(FIXTURE_2)}
			>
				Gesture fixture 2
			</button>
			<button
				type="button"
				onClick={() => void stageSelection.applyFixtureGesture(FIXTURE_3)}
			>
				Gesture fixture 3
			</button>
			<button type="button" onClick={() => void stageSelection.clear()}>
				Clear Stage selection
			</button>
		</div>
	),
}));
vi.mock("./stageWindow/Stage3dView", () => ({ Stage3dView: () => null }));

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

function openSelection(
	revision: number,
	selected: readonly string[],
): SelectionProjection {
	return {
		selected,
		expression: {
			type: "sources",
			items: selected.map((fixtureId) => ({ type: "fixture", fixtureId })),
		},
		revision,
		gestureOpen: true,
	};
}

function outcome(
	request: SelectionActionRequest,
	selected: SelectionProjection,
): SelectionActionOutcome {
	return {
		requestId: request.requestId,
		correlationId: request.requestId,
		action:
			request.action.type === "replace" ? "replaced" : "gesture_applied",
		applied: selected.selected.length,
		selection: selected,
		eventSequence: selected.revision + 10,
		replayed: false,
		warning: null,
	};
}

function renderStage({
	active = true,
	store = new ProgrammingInteractionStore(),
	transport = new FakeProgrammingTransport(),
	loadSnapshot = vi.fn(async () => programmingSnapshot()),
	applySelection,
}: {
	active?: boolean;
	store?: ProgrammingInteractionStore;
	transport?: FakeProgrammingTransport;
	loadSnapshot?: () => Promise<ProgrammingSnapshot>;
	applySelection?: (
		deskId: string,
		request: SelectionActionRequest,
	) => Promise<SelectionActionOutcome>;
} = {}) {
	const view = (enabled: boolean) => (
		<ProgrammingInteractionViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
			applySelection={applySelection}
		>
			<StageWindow active={enabled} />
		</ProgrammingInteractionViewProvider>
	);
	return {
		...render(view(active)),
		view,
		store,
		transport,
		loadSnapshot,
	};
}

afterEach(() => {
	cleanup();
	legacyServer.use.mockReset();
});

describe("Stage selection projection", () => {
	it("updates ordered Stage selection from peer or OSC events without legacy reloads", async () => {
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const { transport } = renderStage({ loadSnapshot });

		await screen.findByText("1 selected");
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: false,
			selection: true,
		});

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({
					revision: 2,
					selected: [FIXTURE_3, FIXTURE_1],
				}),
			}),
		);

		await screen.findByText("2 selected");
		expect(screen.getByTestId("stage-selection")).toHaveAttribute(
			"data-selection",
			`${FIXTURE_3},${FIXTURE_1}`,
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(legacyServer.use).not.toHaveBeenCalled();
	});

	it("keeps consecutive Stage gestures optimistic, ordered, and FIFO", async () => {
		const firstResponse = deferred<SelectionActionOutcome>();
		const secondResponse = deferred<SelectionActionOutcome>();
		const applySelection = vi
			.fn()
			.mockReturnValueOnce(firstResponse.promise)
			.mockReturnValueOnce(secondResponse.promise);
		const { store } = renderStage({ applySelection });
		await screen.findByText("1 selected");

		fireEvent.click(screen.getByRole("button", { name: "Gesture fixture 2" }));
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);
		fireEvent.click(screen.getByRole("button", { name: "Gesture fixture 3" }));
		expect(store.getSnapshot().selection?.selected).toEqual([
			FIXTURE_2,
			FIXTURE_3,
		]);
		expect(screen.getByTestId("stage-selection")).toHaveAttribute(
			"data-selection",
			`${FIXTURE_2},${FIXTURE_3}`,
		);
		await waitFor(() => expect(applySelection).toHaveBeenCalledOnce());
		expect(applySelection.mock.calls[0]?.[1]).toMatchObject({
			action: {
				type: "gesture",
				source: { type: "fixture", fixtureId: FIXTURE_2 },
				remove: false,
			},
		});

		const firstRequest = applySelection.mock
			.calls[0]?.[1] as SelectionActionRequest;
		firstResponse.resolve(outcome(firstRequest, openSelection(2, [FIXTURE_2])));
		await waitFor(() => expect(applySelection).toHaveBeenCalledTimes(2));
		const secondRequest = applySelection.mock
			.calls[1]?.[1] as SelectionActionRequest;
		secondResponse.resolve(
			outcome(
				secondRequest,
				openSelection(3, [FIXTURE_2, FIXTURE_3]),
			),
		);
		await waitFor(() =>
			expect(store.getSnapshot().pendingCapabilities).toEqual(new Set()),
		);
		expect(store.getSnapshot().selection?.selected).toEqual([
			FIXTURE_2,
			FIXTURE_3,
		]);
	});

	it("rolls a rejected Stage gesture back to authoritative selection", async () => {
		const response = deferred<SelectionActionOutcome>();
		const applySelection = vi.fn().mockReturnValue(response.promise);
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const { store } = renderStage({ applySelection, loadSnapshot });
		await screen.findByText("1 selected");

		fireEvent.click(screen.getByRole("button", { name: "Gesture fixture 2" }));
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);
		response.reject(
			Object.assign(new Error("fixture does not exist"), { status: 404 }),
		);

		await waitFor(() =>
			expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_1]),
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
	});

	it("uses an explicit optimistic replacement when Stage clears selection", async () => {
		const response = deferred<SelectionActionOutcome>();
		const applySelection = vi.fn().mockReturnValue(response.promise);
		const { store } = renderStage({ applySelection });
		await screen.findByText("1 selected");

		fireEvent.click(
			screen.getByRole("button", { name: "Clear Stage selection" }),
		);
		expect(store.getSnapshot().selection?.selected).toEqual([]);
		await waitFor(() => expect(applySelection).toHaveBeenCalledOnce());
		const request = applySelection.mock.calls[0]?.[1] as SelectionActionRequest;
		expect(request.action).toMatchObject({
			type: "replace",
			fixtures: [],
			expectedRevision: 1,
		});
		response.resolve(outcome(request, selection(2, [])));
		await waitFor(() =>
			expect(store.getSnapshot().pendingCapabilities).toEqual(new Set()),
		);
	});

	it("does not load or subscribe while the Stage pane is inactive", async () => {
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const rendered = renderStage({ active: false, loadSnapshot });

		expect(screen.getByText("0 selected")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(rendered.transport.subscriptions).toHaveLength(0);

		rendered.rerender(rendered.view(true));
		await screen.findByText("1 selected");
		expect(rendered.transport.subscriptions).toHaveLength(1);
	});
});
