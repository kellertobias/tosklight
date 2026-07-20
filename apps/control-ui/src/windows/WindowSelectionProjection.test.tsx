import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { PropsWithChildren, ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	ProgrammingSnapshot,
	SelectionActionOutcome,
	SelectionActionRequest,
} from "../features/programmingInteraction/contracts";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	programmingSnapshot,
	SHOW_ID,
	selectionChange,
} from "../features/programmingInteraction/testFixtures";
import { ChannelsWindow } from "./ChannelsWindow";
import { FixtureSheetWindow } from "./FixtureSheetWindow";
import { PatchWindow } from "./PatchWindow";
import { PresetsWindow } from "./PresetsWindow";

const mocks = vi.hoisted(() => {
	const selectionAccess = vi.fn();
	const mutationQueue = {
		canWrite: true,
		route: "normal" as const,
		submitLatest: vi.fn(async () => undefined),
		submitBarrier: vi.fn(async () => undefined),
	};
	const server = {
		bootstrap: {
			active_show: true,
			active_programmers: [],
			hardware_connected: false,
		},
		session: { session_id: "session-a" },
		configuration: {
			patch_preview_highlight_dmx: true,
			programmer_fade_millis: 3_000,
		},
		patch: {
			fixtures: [
				{
					fixture_id: "22222222-2222-4222-8222-222222222222",
					logical_heads: [],
					definition: { name: "Fixture 1", model: "Fixture 1", heads: [] },
				},
				{
					fixture_id: "33333333-3333-4333-8333-333333333333",
					logical_heads: [],
					definition: { name: "Fixture 2", model: "Fixture 2", heads: [] },
				},
			],
		},
		playbacks: { cue_lists: [] },
		highlight: { active: false },
		readVisualization: vi.fn(async () => ({ values: [] })),
		setPatchPreviewHighlight: vi.fn(async () => undefined),
	};
	Object.defineProperty(server, "selectedFixtures", {
		get() {
			selectionAccess();
			return ["legacy-fixture"];
		},
	});
	return {
		server,
		selectionAccess,
		mutationQueue,
		mutationQueueUse: vi.fn(),
		dispatch: vi.fn(),
	};
});

vi.mock("../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock(
	"../features/programmerValues/useProgrammerValuesMutationQueue",
	async (importOriginal) => {
		const actual =
			await importOriginal<
				typeof import("../features/programmerValues/useProgrammerValuesMutationQueue")
			>();
		return {
			...actual,
			useProgrammerValuesMutationQueue: (active: boolean) => {
				mocks.mutationQueueUse(active);
				return mocks.mutationQueue;
			},
		};
	},
);
vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: {
			preload: "idle",
			midiProfile: null,
			fixtureGroupsVisible: false,
			fixtureSheetOrder: "fixture-id",
			fixtureSheetActiveOnly: false,
			fixtureSheetCueListId: "",
			fixtureSheetColumns: [],
			fixtureSheetShowType: false,
			fixtureSheetIncludedHeads: "all",
			presetFamily: "Intensity",
			presetPoolColors: true,
			presetGroupsVisible: false,
			updateArmed: false,
			presetSetArmed: false,
			storeArmed: false,
		},
		dispatch: mocks.dispatch,
	}),
}));
vi.mock("../components/control/VerticalTouchFader", () => ({
	VerticalTouchFader: ({
		label,
		disabled,
		onChange,
	}: {
		label: string;
		disabled?: boolean;
		onChange?: (value: number) => void;
	}) => (
		<button type="button" disabled={disabled} onClick={() => onChange?.(42)}>
			{label}
		</button>
	),
}));
vi.mock("../components/shared/GroupStrip", () => ({
	GroupStrip: () => null,
}));
vi.mock("../components/shared/SourceLegend", () => ({
	SourceLegend: () => null,
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: () => undefined,
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	usePresets: () => [],
}));
vi.mock("../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));
vi.mock("./fixtureSheetColumns", () => ({ fixtureSheetColumns: () => [] }));
vi.mock("./fixtureSheetProjection", () => ({
	useFixtureSheetRows: () => [],
	useFixtureSheetVisualizations: () => ({
		visualization: null,
		preloadVisualization: null,
	}),
}));
vi.mock("./fixtureSheetStep", () => ({
	createFixtureStepPresenter: () => () => ({}),
}));
vi.mock("./FixtureSheetTable", () => ({
	FixtureSheetTable: ({
		onActivate,
		selectedFixtureIds,
	}: {
		onActivate: (fixtureId: string) => void;
		selectedFixtureIds: ReadonlySet<string>;
	}) => (
		<div
			data-testid="fixture-sheet-selection"
			data-selection={[...selectedFixtureIds].join(",")}
		>
			<button type="button" onClick={() => onActivate(FIXTURE_2)}>
				Activate fixture 2
			</button>
		</div>
	),
}));
vi.mock("../components/setup/FixturePatchSetup", () => ({
	PatchFeatureBoundary: ({ children }: PropsWithChildren) => children,
	FixturePatchSetupContent: ({
		onStagePreview,
		onMedia,
	}: {
		onStagePreview: () => void;
		onMedia: () => void;
	}) => (
		<>
			<button type="button" onClick={onStagePreview}>
				Preview Stage
			</button>
			<button type="button" onClick={onMedia}>
				Media Servers
			</button>
		</>
	),
}));
vi.mock("../components/setup/MediaServerSetup", () => ({
	MediaServerSetup: () => <div>Media setup</div>,
}));
vi.mock("../features/patch/PatchContext", () => ({
	usePatch: () => ({ fixtures: mocks.server.patch.fixtures }),
}));
vi.mock("../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: false }),
}));
vi.mock("./StageWindow", () => ({
	StageWindow: ({ active }: { active?: boolean }) => (
		<div data-testid="patch-stage" data-active={String(active)} />
	),
}));

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

function renderSelectionView(
	children: ReactNode,
	{
		store = new ProgrammingInteractionStore(),
		transport = new FakeProgrammingTransport(),
		loadSnapshot = vi.fn(async () => programmingSnapshot()),
		applySelection,
	}: {
		store?: ProgrammingInteractionStore;
		transport?: FakeProgrammingTransport;
		loadSnapshot?: () => Promise<ProgrammingSnapshot>;
		applySelection?: (
			deskId: string,
			request: SelectionActionRequest,
		) => Promise<SelectionActionOutcome>;
	} = {},
) {
	const view = (body: ReactNode) => (
		<ProgrammingInteractionViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
			applySelection={applySelection}
		>
			{body}
		</ProgrammingInteractionViewProvider>
	);
	return {
		...render(view(children)),
		view,
		store,
		transport,
		loadSnapshot,
	};
}

function selectedChannel(index: number) {
	return document.querySelectorAll(".channel-fader")[index];
}

beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	mocks.selectionAccess.mockClear();
	mocks.server.readVisualization.mockClear();
	mocks.server.setPatchPreviewHighlight.mockClear();
	mocks.mutationQueue.canWrite = true;
	mocks.mutationQueue.submitLatest.mockClear();
	mocks.mutationQueue.submitBarrier.mockClear();
	mocks.mutationQueueUse.mockClear();
});

afterEach(cleanup);

describe("window selection projections", () => {
	it("updates Channels, Fixture Sheet, and Presets from scoped events", async () => {
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const { transport } = renderSelectionView(
			<>
				<ChannelsWindow compact />
				<FixtureSheetWindow />
				<PresetsWindow compact />
			</>,
			{ loadSnapshot },
		);

		await waitFor(() => expect(selectedChannel(0)).toHaveClass("selected"));
		expect(screen.getByTestId("fixture-sheet-selection")).toHaveAttribute(
			"data-selection",
			FIXTURE_1,
		);
		expect(screen.getByText("1 selected")).toBeInTheDocument();
		expect(screen.getAllByText("Tap to record programmer")).toHaveLength(200);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({ revision: 2, selected: [FIXTURE_2] }),
			}),
		);

		await waitFor(() => expect(selectedChannel(1)).toHaveClass("selected"));
		expect(selectedChannel(0)).not.toHaveClass("selected");
		expect(screen.getByTestId("fixture-sheet-selection")).toHaveAttribute(
			"data-selection",
			FIXTURE_2,
		);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 21,
				correlationId: null,
				change: selectionChange({ revision: 3, selected: [] }),
			}),
		);
		await waitFor(() =>
			expect(screen.getAllByText("Select fixtures to record")).toHaveLength(
				200,
			),
		);
		expect(screen.getByText("0 selected")).toBeInTheDocument();
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});

	it("optimistically replaces Channel selection with one ordered fixture", async () => {
		const response = deferred<SelectionActionOutcome>();
		const applySelection = vi.fn(
			(_deskId: string, _request: SelectionActionRequest) => response.promise,
		);
		const { store } = renderSelectionView(<ChannelsWindow compact />, {
			applySelection,
		});
		await waitFor(() => expect(selectedChannel(0)).toHaveClass("selected"));

		fireEvent.click(selectedChannel(1));

		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);
		await waitFor(() => expect(selectedChannel(1)).toHaveClass("selected"));
		expect(applySelection).toHaveBeenCalledOnce();
		expect(applySelection.mock.calls[0]?.[1].action).toMatchObject({
			type: "replace",
			fixtures: [FIXTURE_2],
			expectedRevision: 1,
		});
	});

	it("writes one typed latest intensity mutation from Channels", async () => {
		renderSelectionView(<ChannelsWindow compact />);
		const channel = await screen.findByRole("button", { name: "CH 1" });

		fireEvent.click(channel);

		await waitFor(() =>
			expect(mocks.mutationQueue.submitLatest).toHaveBeenCalledOnce(),
		);
		expect(mocks.mutationQueue.submitLatest).toHaveBeenCalledWith(
			expect.any(String),
			[
				{
					action: "set_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.42 },
					timing: {
						fade: true,
						fadeMillis: 3_000,
						delayMillis: null,
					},
				},
			],
		);
	});

	it("disables Channel value writes while scoped authority is loading", async () => {
		mocks.mutationQueue.canWrite = false;
		renderSelectionView(<ChannelsWindow compact />);

		expect(await screen.findByRole("button", { name: "CH 1" })).toBeDisabled();
		expect(mocks.mutationQueue.submitLatest).not.toHaveBeenCalled();
	});

	it("optimistically applies Fixture Sheet logical-target gestures", async () => {
		const response = deferred<SelectionActionOutcome>();
		const applySelection = vi.fn(
			(_deskId: string, _request: SelectionActionRequest) => response.promise,
		);
		const { store } = renderSelectionView(<FixtureSheetWindow compact />, {
			applySelection,
		});
		await waitFor(() =>
			expect(screen.getByTestId("fixture-sheet-selection")).toHaveAttribute(
				"data-selection",
				FIXTURE_1,
			),
		);

		fireEvent.click(screen.getByRole("button", { name: "Activate fixture 2" }));

		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);
		expect(screen.getByTestId("fixture-sheet-selection")).toHaveAttribute(
			"data-selection",
			FIXTURE_2,
		);
		await waitFor(() => expect(applySelection).toHaveBeenCalledOnce());
		expect(applySelection.mock.calls[0]?.[1].action).toMatchObject({
			type: "gesture",
			source: { type: "fixture", fixtureId: FIXTURE_2 },
			remove: false,
		});
	});

	it("streams scoped selection into Patch DMX preview only while relevant", async () => {
		const { transport } = renderSelectionView(<PatchWindow />);
		expect(transport.subscriptions).toHaveLength(0);

		fireEvent.click(screen.getByRole("button", { name: "Preview Stage" }));
		await waitFor(() =>
			expect(mocks.server.setPatchPreviewHighlight).toHaveBeenLastCalledWith(
				true,
				[FIXTURE_1],
			),
		);
		expect(screen.getByTestId("patch-stage")).toHaveAttribute(
			"data-active",
			"true",
		);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({
					revision: 2,
					selected: [FIXTURE_2, FIXTURE_1],
				}),
			}),
		);
		await waitFor(() =>
			expect(mocks.server.setPatchPreviewHighlight).toHaveBeenLastCalledWith(
				true,
				[FIXTURE_2, FIXTURE_1],
			),
		);
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});

	it("does not hydrate or subscribe any covered selection view", () => {
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const { transport } = renderSelectionView(
			<>
				<ChannelsWindow active={false} compact />
				<FixtureSheetWindow active={false} compact />
				<PatchWindow active={false} />
				<PresetsWindow active={false} compact />
			</>,
			{ loadSnapshot },
		);

		fireEvent.click(screen.getByRole("button", { name: "Preview Stage" }));

		expect(screen.getByTestId("patch-stage")).toHaveAttribute(
			"data-active",
			"false",
		);
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
		expect(mocks.mutationQueueUse).toHaveBeenCalledWith(false);
		expect(mocks.mutationQueue.submitLatest).not.toHaveBeenCalled();
		expect(mocks.mutationQueue.submitBarrier).not.toHaveBeenCalled();
		expect(mocks.selectionAccess).not.toHaveBeenCalled();
	});
});
