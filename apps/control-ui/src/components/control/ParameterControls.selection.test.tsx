import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProgrammingInteractionViewProvider } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../features/programmingInteraction/store";
import {
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	programmingSnapshot,
	SHOW_ID,
	selectionChange,
} from "../../features/programmingInteraction/testFixtures";
import { ParameterControls } from "./ParameterControls";

const mocks = vi.hoisted(() => {
	const legacySelectionAccess = vi.fn();
	const server = {
		patch: { fixtures: [] as Array<Record<string, unknown>> },
		playbacks: { cue_lists: [] },
		bootstrap: {
			active_programmers: [] as unknown[],
			hardware_connected: false,
		},
		session: { session_id: "session-a", user: { id: "operator" } },
		readVisualization: vi.fn(async () => ({ values: [] })),
		alignSelection: vi.fn(async () => undefined),
		controlFixtureAction: vi.fn(async () => undefined),
		generateFixturePresets: vi.fn(async () => ({ created: [] })),
	};
	Object.defineProperties(server, {
		selectedFixtures: {
			get() {
				legacySelectionAccess("selectedFixtures");
				return ["legacy-fixture"];
			},
		},
		selectedGroupId: {
			get() {
				legacySelectionAccess("selectedGroupId");
				return "legacy-group";
			},
		},
	});
	return {
		legacySelectionAccess,
		server,
		dispatch: vi.fn(),
		valuesActions: { batch: vi.fn(async () => null) },
	};
});

vi.mock("../../api/ServerContext", () => ({ useServer: () => mocks.server }));
vi.mock(
	"../../features/programmerCaptureMode/ProgrammerCaptureModeView",
	() => ({
		useProgrammerCaptureModeView: (enabled = true) =>
			enabled
				? {
						userId: "operator",
						revision: 1,
						blind: false,
						preview: false,
						preloadCaptureProgrammer: true,
					}
				: null,
	}),
);
vi.mock("../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => mocks.valuesActions,
}));
vi.mock(
	"../../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({
		useProgrammerPreloadValuesActions: () => null,
		useProgrammerPreloadValuesSelector: () => null,
	}),
);
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			stageMode: "select",
			builtIn: null,
			desks: [],
			activeDeskId: "programming",
			midiProfile: null,
			shiftArmed: false,
		},
		dispatch: mocks.dispatch,
	}),
}));
vi.mock(
	"./parameterControls/useSelectedPortableGroup",
	async (importOriginal) => {
		const actual =
			await importOriginal<
				typeof import("./parameterControls/useSelectedPortableGroup")
			>();
		return { ...actual, useSelectedPortableGroup: () => undefined };
	},
);
vi.mock("./parameterControls/useParameterProgrammerValues", () => ({
	useParameterProgrammerValues: (
		_fixtureIds: readonly string[],
		_groupId: string | null,
		enabled: boolean,
	) => (enabled ? { ready: true, fixtureValues: [], groupValues: [] } : null),
}));
vi.mock("./VerticalTouchFader", () => ({
	VerticalTouchFader: ({
		label,
		onChange,
	}: {
		label: string;
		onChange: (value: number) => void;
	}) => (
		<button type="button" onClick={() => onChange(50)}>
			{label}
		</button>
	),
}));

function fixture(fixtureId: string) {
	return {
		fixture_id: fixtureId,
		logical_heads: [],
		definition: {
			heads: [
				{
					shared: true,
					parameters: [{ attribute: "intensity", capabilities: [] }],
				},
			],
		},
	};
}

function selectionView(
	children: ReactNode,
	{
		store = new ProgrammingInteractionStore(),
		transport = new FakeProgrammingTransport(),
		loadSnapshot = vi.fn(async () => programmingSnapshot()),
	} = {},
) {
	return {
		body: (
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				{children}
			</ProgrammingInteractionViewProvider>
		),
		store,
		transport,
		loadSnapshot,
	};
}

beforeEach(() => {
	mocks.server.patch.fixtures = [fixture(FIXTURE_1), fixture(FIXTURE_2)];
	mocks.legacySelectionAccess.mockClear();
	vi.clearAllMocks();
});

afterEach(cleanup);

describe("ParameterControls selection projection", () => {
	it("targets parameter writes from the streamed ordered selection", async () => {
		const view = selectionView(<ParameterControls />);
		render(view.body);

		const encoder = await screen.findByRole("button", {
			name: "Enc 1 · Dimmer",
		});
		fireEvent.click(encoder);
		expect(mocks.valuesActions.batch).toHaveBeenLastCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "set_fixture",
					fixtureId: FIXTURE_1,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.5 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});

		mocks.valuesActions.batch.mockClear();
		act(() =>
			view.transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({ revision: 2, selected: [FIXTURE_2] }),
			}),
		);
		fireEvent.click(screen.getByRole("button", { name: "Enc 1 · Dimmer" }));

		await waitFor(() =>
			expect(mocks.valuesActions.batch).toHaveBeenLastCalledWith({
				requestId: expect.any(String),
				mutations: [
					{
						action: "set_fixture",
						fixtureId: FIXTURE_2,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.5 },
						timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
					},
				],
			}),
		);
		expect(view.loadSnapshot).toHaveBeenCalledOnce();
		expect(mocks.legacySelectionAccess).not.toHaveBeenCalled();
	});

	it("stays dormant until the parameter view becomes active", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const inactive = selectionView(<ParameterControls active={false} />, {
			store,
			transport,
			loadSnapshot,
		});
		const rendered = render(inactive.body);

		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
		expect(mocks.server.readVisualization).not.toHaveBeenCalled();

		const active = selectionView(<ParameterControls />, {
			store,
			transport,
			loadSnapshot,
		});
		rendered.rerender(active.body);

		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0]?.scope).toEqual({
			commandLine: false,
			selection: true,
		});
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(mocks.legacySelectionAccess).not.toHaveBeenCalled();
	});
});
