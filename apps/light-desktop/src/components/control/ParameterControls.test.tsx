import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import type { PropsWithChildren } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	AttributeDescriptor,
	VisualizationSnapshot,
} from "../../api/types";
import { DynamicEditorSessionProvider } from "../../features/dynamics/DynamicEditorSessionContext";
import { selectFixturesForSelection } from "../../features/patch/selectors";
import type {
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
} from "../../features/programmerValues/contracts";
import {
	clearTimecodeEncoderDeck,
	publishTimecodeEncoderDeck,
} from "../../features/timecode/timecodeEncoderBridge";
import { ParameterControls } from "./ParameterControls";
import { VisibleEncoderCountProvider } from "./parameterControls/VisibleEncoderCount";

function TestProviders({ children }: PropsWithChildren) {
	return (
		<ModalProvider>
			<DynamicEditorSessionProvider>{children}</DynamicEditorSessionProvider>
		</ModalProvider>
	);
}

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: TestProviders });

const state = {
	stageMode: "select",
	stageView: "2d",
	builtIn: null as string | null,
	desks: [],
	activeDeskId: "programming",
	preload: "idle",
	shiftArmed: false,
};
const timecodeEncoderOwner = Symbol("parameter-controls-test");
const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
	if (action.type === "SET_SHIFT_ARMED")
		state.shiftArmed = Boolean(action.value);
});
const programmerValues = vi.hoisted(() => ({
	view: {
		ready: true,
		fixtureValues: [] as ProgrammerFixtureValue[],
		groupValues: [] as ProgrammerGroupValue[],
	},
}));
const captureMode = vi.hoisted(() => ({
	ready: true,
	projection: {
		sessionId: "operator",
		revision: 1,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: true,
	},
}));
const preloadProgrammerValues = vi.hoisted(() => ({
	view: {
		ready: true,
		fixtureValues: [] as ProgrammerFixtureValue[],
		groupValues: [] as ProgrammerGroupValue[],
	},
}));
const normalValuesActions = vi.hoisted(() => ({
	batch: vi.fn(async () => null),
	applyIntent: vi.fn(async (_intent: unknown) => null),
	applyIndexedPreset: vi.fn(async () => null),
}));
const commandLine = vi.hoisted(() => ({
	text: "FIXTURE",
	reset: vi.fn(async () => true),
}));
const preloadValuesActions = vi.hoisted(() => ({
	batch: vi.fn(async () => null),
}));
const visualization = vi.hoisted(() => ({
	snapshot: null as VisualizationSnapshot | null,
}));
const patchFixtures = vi.hoisted(() => ({
	current: [] as Array<Record<string, unknown>>,
}));
const attributeRegistry = vi.hoisted(() => ({
	current: [] as AttributeDescriptor[],
}));
vi.mock("../../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	useSelectedPatchedFixtures: (
		selectedFixtureIds: readonly string[],
		enabled = true,
	) =>
		enabled
			? selectFixturesForSelection(
					{ fixtures: patchFixtures.current } as never,
					new Set(selectedFixtureIds),
				)
			: [],
}));
const legacyProgrammerValuesAccess = vi.fn();
const legacyPlaybackAccess = vi.fn();
const server = {
	selectedFixtures: [] as string[],
	selectedGroupId: null as string | null,
	groups: [] as Array<Record<string, unknown>>,
	patch: {
		get fixtures() {
			return patchFixtures.current;
		},
		set fixtures(value: Array<Record<string, unknown>>) {
			patchFixtures.current = value;
		},
	},
	bootstrap: { hardware_connected: false } as {
		hardware_connected: boolean;
		readonly active_programmers: unknown[];
	},
	session: { session_id: "session-1", user: { id: "operator" } },
	readVisualization: vi.fn().mockResolvedValue({ values: [] }),
	alignSelection: vi.fn(),
	controlFixtureAction: vi.fn(),
	controlFixtureActions: vi.fn().mockResolvedValue(undefined),
	generateFixturePresets: vi.fn().mockResolvedValue({ created: [] }),
};
Object.defineProperty(server.bootstrap, "active_programmers", {
	get() {
		legacyProgrammerValuesAccess();
		return [];
	},
});
Object.defineProperty(server, "playbacks", {
	get() {
		legacyPlaybackAccess();
		return null;
	},
});

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch }),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => Boolean(server.bootstrap?.hardware_connected),
	useAttributeRegistry: () => attributeRegistry.current,
}));
vi.mock(
	"../../features/programmerActions/ProgrammerActionsContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useProgrammerActions: () => ({
			undoProgrammer: vi.fn(),
			clearProgrammer: vi.fn(),
			controlFixtureAction: server.controlFixtureAction,
			controlFixtureActions: server.controlFixtureActions,
			generateFixturePresets: server.generateFixturePresets,
			alignSelection: server.alignSelection,
			storePreload: vi.fn(),
		}),
	}),
);
vi.mock("../../features/visualizationRuntime/VisualizationRuntimeView", () => ({
	useVisualizationRuntimeSnapshot: ({ enabled = true }) =>
		enabled ? visualization.snapshot : null,
}));
vi.mock(
	"../../features/programmerCaptureMode/ProgrammerCaptureModeView",
	() => ({
		useProgrammerCaptureModeView: (enabled = true) =>
			enabled && captureMode.ready ? captureMode.projection : null,
	}),
);
vi.mock("../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => normalValuesActions,
}));
vi.mock(
	"../../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({
		useProgrammerPreloadValuesActions: () => preloadValuesActions,
		useProgrammerPreloadValuesSelector: (
			_selector: unknown,
			_equal: unknown,
			enabled = true,
		) => (enabled ? preloadProgrammerValues.view : null),
	}),
);
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: (active = true) =>
			active
				? {
						selected: server.selectedFixtures,
						expression: server.selectedGroupId
							? {
									type: "live_group",
									groupId: server.selectedGroupId,
									rule: { type: "all" },
								}
							: { type: "static" },
						revision: 1,
						gestureOpen: false,
					}
				: null,
		useProgrammingSelectionActions: () => null,
		useProgrammingDeleteCommandActive: () =>
			commandLine.text.trim().toUpperCase() === "DELETE",
		useProgrammingCommandLineActions: () => ({
			reset: commandLine.reset,
		}),
	}),
);
vi.mock(
	"./parameterControls/useSelectedPortableGroup",
	async (importOriginal) => {
		const actual =
			await importOriginal<
				typeof import("./parameterControls/useSelectedPortableGroup")
			>();
		return {
			...actual,
			useSelectedPortableGroup: (groupId: string | null, active = true) =>
				active && groupId
					? (server.groups.find((group) => group.id === groupId) ?? null)
					: undefined,
		};
	},
);
vi.mock("./parameterControls/useParameterProgrammerValues", () => ({
	useParameterProgrammerValues: (
		_fixtureIds: readonly string[],
		_groupId: string | null,
		enabled: boolean,
	) => (enabled ? programmerValues.view : null),
}));

afterEach(() => {
	cleanup();
	clearTimecodeEncoderDeck(timecodeEncoderOwner);
	state.stageMode = "select";
	state.stageView = "2d";
	state.builtIn = null;
	state.desks = [];
	state.shiftArmed = false;
	server.selectedFixtures = [];
	server.selectedGroupId = null;
	server.groups = [];
	server.patch.fixtures = [];
	attributeRegistry.current = [];
	server.bootstrap.hardware_connected = false;
	programmerValues.view.ready = true;
	programmerValues.view.fixtureValues = [];
	programmerValues.view.groupValues = [];
	preloadProgrammerValues.view.ready = true;
	preloadProgrammerValues.view.fixtureValues = [];
	preloadProgrammerValues.view.groupValues = [];
	captureMode.ready = true;
	captureMode.projection.revision = 1;
	captureMode.projection.blind = false;
	captureMode.projection.preview = false;
	captureMode.projection.preloadCaptureProgrammer = true;
	visualization.snapshot = null;
	commandLine.text = "FIXTURE";
	vi.clearAllMocks();
});

describe("ParameterControls projection lifecycle", () => {
	it("gives an active Timecode both encoder groups instead of generic controls", () => {
		const timelineSet = vi.fn();
		const keyframeSet = vi.fn();
		publishTimecodeEncoderDeck(timecodeEncoderOwner, {
			timeline: [
				{
					id: "zoom",
					label: "Timeline zoom",
					display: "100%",
					value: 1,
					minimum: 1,
					maximum: 4,
					fineStep: 0.05,
					coarseStep: 0.25,
					set: timelineSet,
				},
			],
			keyframe: [
				{
					id: "value",
					label: "Volume",
					display: "80%",
					value: 80,
					minimum: 0,
					maximum: 100,
					fineStep: 1,
					coarseStep: 10,
					set: keyframeSet,
				},
			],
		});
		state.builtIn = "timecode";

		render(<ParameterControls />);

		expect(
			screen.getByRole("button", { name: "Selected Keyframe" }),
		).toHaveClass("is-active");
		expect(
			screen.getByRole("group", { name: "Enc 1 · Volume" }),
		).toHaveTextContent("80%");
		expect(screen.queryByRole("button", { name: "Intensity" })).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Timecode Timeline" }));
		expect(
			screen.getByRole("group", { name: "Enc 1 · Timeline zoom" }),
		).toHaveTextContent("100%");
	});

	it("routes hardware encoder turns to the active Timecode deck", () => {
		const set = vi.fn();
		publishTimecodeEncoderDeck(timecodeEncoderOwner, {
			timeline: [],
			keyframe: [
				{
					id: "position",
					label: "Keyframe position",
					display: "00:00:01.00",
					value: 1,
					minimum: 0,
					maximum: 440,
					fineStep: 1,
					coarseStep: 44,
					set,
				},
			],
		});
		state.builtIn = "timecode";
		server.bootstrap.hardware_connected = true;
		render(<ParameterControls />);

		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "up" },
			}),
		);

		expect(set).toHaveBeenCalledWith(2);
	});

	it("labels the selected encoder page for a marker", () => {
		publishTimecodeEncoderDeck(timecodeEncoderOwner, {
			timeline: [],
			keyframe: [
				{
					id: "marker-position",
					label: "Marker position",
					display: "00:00:02.00",
					value: 88,
					minimum: 0,
					maximum: 440,
					fineStep: 1,
					coarseStep: 44,
					set: vi.fn(),
				},
			],
			selectionLabel: "Selected Marker",
		});
		state.builtIn = "timecode";
		render(<ParameterControls />);

		expect(
			screen.getByRole("button", { name: "Selected Marker" }),
		).toHaveClass("is-active");
		expect(
			screen.getByRole("group", { name: "Enc 1 · Marker position" }),
		).toHaveTextContent("00:00:02.00");
	});

	it("shows fixture-profile Color defaults before a programmer value exists", () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							index: 0,
							shared: true,
							parameters: [
								{ attribute: "color.red", default: 1, capabilities: [] },
								{ attribute: "color.green", default: 1, capabilities: [] },
								{ attribute: "color.blue", default: 1, capabilities: [] },
							],
						},
					],
				},
			},
		];

		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Color" }));

		expect(
			screen.getByRole("group", { name: "Enc 1 · color red" }),
		).toHaveTextContent("100%");
		expect(
			screen.getByRole("group", { name: "Enc 2 · color green" }),
		).toHaveTextContent("100%");
		expect(
			screen.getByRole("group", { name: "Enc 3 · color blue" }),
		).toHaveTextContent("100%");
	});

	it("does not mount the visualization projection behind Stage command controls", () => {
		state.stageMode = "setup";
		state.builtIn = "stage";
		server.selectedFixtures = ["fixture-1"];

		render(<ParameterControls />);

		expect(server.readVisualization).not.toHaveBeenCalled();
	});

	it("routes built-in 3D Viz navigation to the renderer camera encoders", () => {
		state.stageMode = "navigate";
		state.stageView = "3d-viz";
		state.builtIn = "stage";

		render(<ParameterControls />);

		expect(screen.getByRole("button", { name: "Position" })).toHaveClass(
			"active",
		);
		expect(
			screen.getByRole("button", { name: "Direction" }),
		).toBeInTheDocument();
	});

	it("never reads legacy bootstrap Programmer values while scoped authority loads or is ready", () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.ready = false;
		const rendered = render(<ParameterControls />);

		expect(legacyProgrammerValuesAccess).not.toHaveBeenCalled();

		programmerValues.view.ready = true;
		rendered.rerender(<ParameterControls />);

		expect(legacyProgrammerValuesAccess).not.toHaveBeenCalled();
	});

	it("keeps value controls inert while capture authority is loading", () => {
		captureMode.ready = false;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];

		render(<ParameterControls />);

		expect(
			screen.getAllByRole("img", { name: /Encoder \d unassigned/ }),
		).toHaveLength(6);
		expect(normalValuesActions.batch).not.toHaveBeenCalled();
		expect(preloadValuesActions.batch).not.toHaveBeenCalled();
		expect(legacyProgrammerValuesAccess).not.toHaveBeenCalled();
	});

	it("routes active capture values and writes only through pending Preload", () => {
		captureMode.projection.blind = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		preloadProgrammerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.25 },
				programmerOrder: 1,
				fade: true,
				fadeMillis: 3_000,
				delayMillis: null,
			},
		];

		render(<ParameterControls />);
		const encoder = screen.getByRole("group", { name: "Enc 1 · Dimmer" });
		expect(encoder).toHaveTextContent("25%");
		fireEvent.keyDown(encoder, { key: "ArrowUp" });

		expect(preloadValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "set_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.251 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
		expect(normalValuesActions.batch).not.toHaveBeenCalled();
	});

	it("uses show-owned encoder pages without shifting sparse slots", () => {
		attributeRegistry.current = [
			attributeDescriptor("color.red", "Red", 1, 1),
			attributeDescriptor("color.wheel.1", "Color Wheel", 3, 3),
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [
								{ attribute: "color.red", capabilities: [] },
								{ attribute: "color.wheel.1", capabilities: [] },
							],
						},
					],
				},
			},
		];

		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Color 1 of 2" }));

		expect(
			screen.getByRole("group", { name: "Enc 1 · Red" }),
		).toBeInTheDocument();
		expect(
			screen.getAllByRole("img", { name: /Encoder \d unassigned/ }),
		).toHaveLength(5);

		fireEvent.click(screen.getByRole("button", { name: "Color 1 of 2" }));

		expect(screen.getByRole("button", { name: "Color 2 of 2" })).toHaveClass(
			"is-active",
		);
		expect(
			screen.getByRole("group", { name: "Enc 3 · Color Wheel" }),
		).toBeInTheDocument();
		expect(
			screen.getAllByRole("img", { name: /Encoder \d unassigned/ }),
		).toHaveLength(5);
		expect(
			screen.queryByRole("group", { name: "Enc 1 · Color Wheel" }),
		).not.toBeInTheDocument();
	});

	it("restores the anchored logical page after encoder width changes", () => {
		attributeRegistry.current = [
			attributeDescriptor("color.red", "Red", 1, 1),
			attributeDescriptor("color.green", "Green", 1, 2),
			attributeDescriptor("color.blue", "Blue", 1, 3),
			attributeDescriptor("color.white", "White", 1, 4),
			attributeDescriptor("color.amber", "Amber", 1, 5),
			attributeDescriptor("color.uv", "UV", 1, 6),
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: attributeRegistry.current.map(({ id }) => ({
								attribute: id,
								capabilities: [],
							})),
						},
					],
				},
			},
		];

		const rendered = render(
			<VisibleEncoderCountProvider count={4}>
				<ParameterControls />
			</VisibleEncoderCountProvider>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Color 1 of 2" }));
		fireEvent.click(screen.getByRole("button", { name: "Color 1 of 2" }));
		expect(screen.getByRole("button", { name: "Color 2 of 2" })).toHaveClass(
			"is-active",
		);

		rendered.rerender(
			<VisibleEncoderCountProvider count={6}>
				<ParameterControls />
			</VisibleEncoderCountProvider>,
		);
		expect(screen.getByRole("button", { name: "Color" })).toHaveClass(
			"is-active",
		);

		rendered.rerender(
			<VisibleEncoderCountProvider count={4}>
				<ParameterControls />
			</VisibleEncoderCountProvider>,
		);
		expect(screen.getByRole("button", { name: "Color 2 of 2" })).toHaveClass(
			"is-active",
		);
	});

	it("renders and routes a Prism rotation as the push-turn value of one encoder", async () => {
		attributeRegistry.current = [
			{
				...attributeDescriptor("prism.1", "Prism 1", 1, 5),
				family: "beam",
				encoder_group: "beam",
			},
			{
				...attributeDescriptor("prism.1.rotation", "Prism 1 Rotation", 1, 6),
				family: "beam",
				encoder_group: "beam",
				push_turn_of: "prism.1",
			},
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [
								{ attribute: "prism.1", capabilities: [] },
								{ attribute: "prism.1.rotation", capabilities: [] },
							],
						},
					],
				},
			},
		];

		const rendered = render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Beam" }));
		expect(
			screen.getByRole("button", { name: "Turn · Prism 1" }),
		).toBeVisible();
		expect(
			screen.getByRole("group", { name: "Enc 5 · Prism 1" }),
		).toBeVisible();
		expect(
			screen.queryByRole("group", { name: "Enc 6 · Prism 1 Rotation" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Turn · Prism 1" }));
		expect(
			screen.getByRole("group", { name: "Enc 5 · Prism 1 Rotation" }),
		).toBeVisible();

		rendered.unmount();
		server.bootstrap.hardware_connected = true;
		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Beam" }));
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/5", value: "up", request_id: "turn" },
			}),
		);
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: {
					control: "encode/5",
					value: "right",
					request_id: "push-turn",
				},
			}),
		);

		await vi.waitFor(() =>
			expect(normalValuesActions.applyIntent).toHaveBeenCalledTimes(2),
		);
		expect(
			normalValuesActions.applyIntent.mock.calls.map(([intent]) => intent),
		).toEqual([
			expect.objectContaining({
				requestId: "turn",
				attribute: "prism.1",
			}),
			expect.objectContaining({
				requestId: "push-turn",
				attribute: "prism.1.rotation",
			}),
		]);
	});

	it("keeps the operator Dimmer label when the registry exposes canonical Intensity metadata", () => {
		attributeRegistry.current = [
			{
				...attributeDescriptor("intensity", "Intensity", 1, 1),
				family: "intensity",
				encoder_group: "intensity",
			},
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];

		render(<ParameterControls />);

		expect(
			screen.getByRole("group", { name: "Enc 1 · Dimmer" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("group", { name: "Enc 1 · Intensity" }),
		).not.toBeInTheDocument();
	});

	it("keeps legacy family attributes when another family uses registry placement", () => {
		attributeRegistry.current = [
			attributeDescriptor("intensity", "Intensity", 1, 1),
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "gobo", capabilities: [] }],
						},
					],
				},
			},
		];

		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Beam" }));

		expect(
			screen.getByRole("group", { name: "Enc 1 · Gobo 1" }),
		).toBeInTheDocument();
	});

	it("keeps Direct input and Indexed Presets under the semantic encoder", () => {
		attributeRegistry.current = [
			{
				...attributeDescriptor("gobo.1", "Gobo 1", 1, 1),
				family: "beam",
				encoder_group: "beam",
				value_type: "indexed",
			},
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];

		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Beam" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Gobo 1 value" }),
		);

		expect(screen.getByRole("tab", { name: "Direct input" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		expect(
			screen.getByRole("tab", { name: "Indexed Presets" }),
		).toHaveAttribute("aria-selected", "false");
		fireEvent.click(screen.getByRole("tab", { name: "Indexed Presets" }));
		fireEvent.click(screen.getByRole("button", { name: /Dots/ }));

		expect(normalValuesActions.applyIndexedPreset).toHaveBeenCalledWith({
			requestId: expect.any(String),
			expectedSelectionRevision: 1,
			attribute: "gobo.1",
			targets: [
				{
					fixtureId: "fixture-1",
					functionId: "function-1",
					expectedProfileRevision: 1,
				},
			],
		});
	});

	it("sends a combined momentary control row through one authoritative action", async () => {
		attributeRegistry.current = [
			{
				...attributeDescriptor("gobo.1", "Gobo 1", 1, 1),
				family: "beam",
				encoder_group: "beam",
				value_type: "indexed",
			},
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];

		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Beam" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Gobo 1 value" }),
		);
		fireEvent.click(screen.getByRole("tab", { name: "Indexed Presets" }));
		fireEvent.click(screen.getByRole("button", { name: /Lamp reset/ }));
		await Promise.resolve();

		expect(server.controlFixtureActions.mock.calls).toEqual([
			[
				[
					{
						fixtureId: "fixture-1",
						actionId: "action-1",
						expectedProfileRevision: 1,
					},
				],
				1,
				true,
			],
			[
				[
					{
						fixtureId: "fixture-1",
						actionId: "action-1",
						expectedProfileRevision: 1,
					},
				],
				1,
				false,
			],
		]);
	});
});

function attributeDescriptor(
	id: string,
	label: string,
	encoderPage: number,
	encoderSlot: number,
): AttributeDescriptor {
	return {
		id,
		label,
		family: "color",
		value_type: "continuous",
		default_unit: null,
		encoder_group: "color",
		encoder_page: encoderPage,
		encoder_slot: encoderSlot,
		built_in: true,
		retired: false,
		activation_group_id: null,
	};
}

function schemaV2Fixture() {
	return {
		fixture_id: "fixture-1",
		logical_heads: [],
		definition: {
			mode_id: "mode-1",
			heads: [
				{
					shared: true,
					parameters: [{ attribute: "gobo.1", capabilities: [] }],
				},
			],
			profile_snapshot: {
				id: "profile-1",
				revision: 1,
				modes: [
					{
						id: "mode-1",
						heads: [{ id: "head-1", master_shared: true }],
						channels: [
							{
								id: "channel-1",
								head_id: "head-1",
								attribute: "gobo.1",
								functions: [
									{
										id: "function-1",
										attribute: "gobo.1",
										behavior: {
											type: "indexed",
											semantic_id: "gobo.dots",
											label: "Dots",
											raw_value: 93,
										},
									},
									{
										id: "function-control-1",
										attribute: "gobo.1",
										behavior: {
											type: "control",
											action_id: "action-1",
										},
									},
								],
							},
						],
						control_actions: [
							{
								id: "action-1",
								name: "Lamp reset",
								semantic: "reset",
								kind: "momentary",
								duration_millis: null as number | null,
								assignments: [
									{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 },
								],
							},
						],
					},
				],
			},
		},
	};
}

describe("ParameterControls hardware encoders", () => {
	it("refuses hardware edits while capture authority is loading", () => {
		captureMode.ready = false;
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];

		render(<ParameterControls />);
		expect(screen.getByLabelText("Encoder 1: Dimmer, 0%").tagName).toBe(
			"SECTION",
		);
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "up" },
			}),
		);

		expect(normalValuesActions.batch).not.toHaveBeenCalled();
		expect(preloadValuesActions.batch).not.toHaveBeenCalled();
	});

	it("keeps six numbered hardware slots and sends fine and coarse relative steps", async () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		const rendered = render(<ParameterControls />);

		expect(screen.getByLabelText("Encoder 1: Dimmer, 0%")).toBeInTheDocument();
		for (let slot = 2; slot <= 6; slot += 1)
			expect(
				screen.getByLabelText(`Encoder ${slot} unassigned`),
			).toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();

		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: {
					control: "encode/1",
					value: "up",
					request_id: "hardware-fine",
				},
			}),
		);
		expect(normalValuesActions.applyIntent).toHaveBeenLastCalledWith({
			requestId: "hardware-fine",
			fixtureIds: ["fixture-1"],
			attribute: "intensity",
			operation: { type: "relative_step", delta: 0.01 },
			timing: { fade: false, fadeMillis: null, delayMillis: null },
			undoGroup: expect.any(String),
		});
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: {
					control: "encode/1",
					value: "right",
					request_id: "hardware-coarse",
				},
			}),
		);
		await vi.waitFor(() =>
			expect(normalValuesActions.applyIntent).toHaveBeenLastCalledWith({
				requestId: "hardware-coarse",
				fixtureIds: ["fixture-1"],
				attribute: "intensity",
				operation: { type: "relative_step", delta: 0.1 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
				undoGroup: expect.any(String),
			}),
		);

		captureMode.projection.blind = true;
		rendered.rerender(<ParameterControls />);
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: {
					control: "encode/1",
					value: "up",
					request_id: "hardware-preload",
				},
			}),
		);
		expect(preloadValuesActions.batch).toHaveBeenLastCalledWith({
			requestId: "hardware-preload",
			mutations: [
				{
					action: "set_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.01 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
	});

	it("uses the hardware encoder card itself as the set-value target", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		render(<ParameterControls />);

		expect(
			screen.queryByRole("button", { name: "Set value for Dimmer" }),
		).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Encoder 1: Dimmer, 0%" }),
		);

		expect(
			screen.getByRole("dialog", { name: "Encoder 1 value" }),
		).toBeInTheDocument();
	});

	it("cycles all family cells with NAV and wraps in both directions", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		render(<ParameterControls />);

		const navigate = (value: string) =>
			fireEvent(
				window,
				new CustomEvent("light:encoder-action", {
					detail: { control: "nav", value },
				}),
			);
		for (const family of [
			"Color",
			"Position",
			"Beam",
			"Shapers",
			"Focus",
			"Control",
			"Media",
			"Intensity",
		]) {
			navigate("down");
			expect(screen.getByRole("button", { name: family })).toHaveClass(
				"is-active",
			);
		}
		navigate("up");
		expect(screen.getByRole("button", { name: "Media" })).toHaveClass(
			"is-active",
		);
		navigate("left");
		expect(screen.getByRole("button", { name: "Control" })).toHaveClass(
			"is-active",
		);
		navigate("right");
		expect(screen.getByRole("button", { name: "Media" })).toHaveClass(
			"is-active",
		);
	});

	it("accepts the first routed hardware event before the attachment snapshot arrives", async () => {
		server.bootstrap.hardware_connected = false;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		render(<ParameterControls />);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: {
					control: "encode/1",
					value: "up",
					request_id: "first-hardware-event",
				},
			}),
		);
		await vi.waitFor(() =>
			expect(normalValuesActions.applyIntent).toHaveBeenLastCalledWith({
				requestId: "first-hardware-event",
				fixtureIds: ["fixture-1"],
				attribute: "intensity",
				operation: { type: "relative_step", delta: 0.01 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
				undoGroup: expect.any(String),
			}),
		);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "nav", value: "down" },
			}),
		);
		expect(screen.getByRole("button", { name: "Color" })).toHaveClass(
			"is-active",
		);
	});

	it("uses encoder press as the same cell action and coarse-turns a non-first slot", async () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [
								{ attribute: "pan", capabilities: [] },
								{ attribute: "tilt", capabilities: [] },
							],
						},
					],
				},
			},
		];
		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Position" }));

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/2", value: "left" },
			}),
		);
		await vi.waitFor(() =>
			expect(normalValuesActions.applyIntent).toHaveBeenLastCalledWith({
				requestId: expect.any(String),
				fixtureIds: ["fixture-1"],
				attribute: "tilt",
				operation: { type: "relative_step", delta: -0.1 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
				undoGroup: expect.any(String),
			}),
		);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/2", value: "press" },
			}),
		);
		expect(
			screen.getByRole("dialog", { name: "Encoder 2 value" }),
		).toBeInTheDocument();
		expect(screen.getByRole("heading", { name: "Tilt" })).toBeInTheDocument();
	});

	it("releases an owned value on exact DELETE plus hardware encoder press and resets DELETE", async () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		commandLine.text = "DELETE";
		render(<ParameterControls />);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "press" },
			}),
		);

		await vi.waitFor(() => expect(commandLine.reset).toHaveBeenCalledOnce());
		expect(normalValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "release_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
				},
			],
		});
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("does not release, create zero, reset DELETE, or open Set Value without scoped ownership", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		commandLine.text = "DELETE";
		render(<ParameterControls />);

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "press" },
			}),
		);

		expect(normalValuesActions.batch).not.toHaveBeenCalled();
		expect(normalValuesActions.applyIntent).not.toHaveBeenCalled();
		expect(commandLine.reset).not.toHaveBeenCalled();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("spreads a typed hardware encoder range over the ordered fixture selection", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-3", "fixture-1", "fixture-2"];
		server.patch.fixtures = server.selectedFixtures.map((fixture_id) => ({
			fixture_id,
			logical_heads: [],
			definition: {
				heads: [
					{
						shared: true,
						parameters: [{ attribute: "intensity", capabilities: [] }],
					},
				],
			},
		}));
		render(<ParameterControls />);

		fireEvent.click(
			screen.getByRole("button", { name: "Encoder 1: Dimmer, 0%" }),
		);
		for (const key of ["0", "THRU", "5", "0", "ENTER"]) {
			fireEvent.click(screen.getByRole("button", { name: key }));
		}

		// The application service resolves the ordered fan-out and any linked captures.
		expect(normalValuesActions.applyIntent).toHaveBeenCalledWith({
			requestId: expect.any(String),
			fixtureIds: ["fixture-3", "fixture-1", "fixture-2"],
			attribute: "intensity",
			operation: {
				type: "absolute_set",
				value: { kind: "spread", value: [0, 0.5] },
			},
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		});
	});
});

describe("ParameterControls hardware feedback values", () => {
	it("formats a discrete hardware target as a semantic value instead of a percentage", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "media.play_mode", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "media.play_mode",
				value: { kind: "discrete", value: "loop" },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Control" }));
		expect(
			screen.getByLabelText("Encoder 1: Play Mode, loop"),
		).not.toHaveTextContent("Built-in");
		expect(
			screen.queryByRole("button", { name: "Set value for Play Mode" }),
		).not.toBeInTheDocument();
		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "right" },
			}),
		);
		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "press" },
			}),
		);
		expect(normalValuesActions.applyIntent).not.toHaveBeenCalled();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("shows a hardware encoder percentage range for mixed selected fixture values", async () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1", "fixture-2"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
			{
				fixture_id: "fixture-2",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		visualization.snapshot = visualizationSnapshot([
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.25 },
			},
			{
				fixture_id: "fixture-2",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.75 },
			},
		]);

		render(<ParameterControls />);

		const encoder = await screen.findByLabelText(
			"Encoder 1: Dimmer, 25%...75%",
		);
		expect(encoder).toHaveTextContent("Dimmer");
		expect(encoder).toHaveTextContent("Enc 1");
		expect(encoder).not.toHaveTextContent("Turn");
		expect(encoder).not.toHaveTextContent("Intensity");
	});
});

describe("ParameterControls programmer targets and alignment", () => {
	it("releases only the visible fixture-scoped attribute", () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 1 },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		render(<ParameterControls />);
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Dimmer value" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Release" }));
		expect(normalValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "release_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
				},
			],
		});
	});

	it("shows the fixture programmer target while visualization is still fading", async () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 1 },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		visualization.snapshot = visualizationSnapshot([
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0 },
			},
		]);

		render(<ParameterControls />);

		expect(await screen.findByText("100%")).toBeInTheDocument();
	});
});

describe("ParameterControls Group targets and alignment", () => {
	it("takes supported attributes from the selected portable Group", () => {
		server.selectedGroupId = "3";
		server.groups = [
			{
				id: "3",
				body: { programming: { pan: {} }, fixtures: [] },
			},
		];

		render(<ParameterControls />);

		expect(
			screen.getByRole("group", { name: "Enc 1 · Dimmer" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Position" }));
		expect(
			screen.getByRole("group", { name: "Enc 1 · Pan" }),
		).toBeInTheDocument();
		expect(legacyPlaybackAccess).not.toHaveBeenCalled();
	});

	it("shows the Group programmer target while its members are still fading", async () => {
		server.selectedFixtures = ["fixture-1"];
		server.selectedGroupId = "3";
		server.groups = [
			{ id: "3", body: { programming: {}, fixtures: ["fixture-1"] } },
		];
		server.patch.fixtures = [
			{
				fixture_id: "fixture-1",
				logical_heads: [],
				definition: {
					heads: [
						{
							shared: true,
							parameters: [{ attribute: "intensity", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.groupValues = [
			{
				groupId: "3",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.75 },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		visualization.snapshot = visualizationSnapshot([
			{
				fixture_id: "fixture-1",
				attribute: "intensity",
				value: { kind: "normalized", value: 0 },
			},
		]);

		render(<ParameterControls />);

		expect(await screen.findByText("75%")).toBeInTheDocument();
	});

	it("cycles the authoritative Left, Right, Out, In, Off modifier in every encoder family", async () => {
		render(<ParameterControls />);
		const align = screen.getByRole("button", { name: "Align Off" });
		expect(align).toHaveClass("align-off");

		for (const mode of ["left", "right", "out", "in"] as const) {
			fireEvent.click(align);
			await waitFor(() =>
				expect(server.alignSelection).toHaveBeenLastCalledWith(mode),
			);
			expect(align).toHaveAccessibleName(
				`Align ${mode[0].toUpperCase()}${mode.slice(1)}`,
			);
			expect(align).toHaveClass("align-active");
		}
		fireEvent.click(align);
		await waitFor(() =>
			expect(server.alignSelection).toHaveBeenLastCalledWith("off"),
		);
		expect(align).toHaveAccessibleName("Align Off");

		fireEvent.click(screen.getByRole("button", { name: "Color" }));
		expect(
			screen.getByRole("button", { name: "Align Off" }),
		).toBeInTheDocument();

		fireEvent.click(align);
		await waitFor(() => expect(align).toHaveAccessibleName("Align Left"));
		state.shiftArmed = true;
		fireEvent.click(align);
		await waitFor(() =>
			expect(align).toHaveAccessibleName("Align Off, Shift: Off"),
		);
		expect(align).toHaveTextContent("AlignOffOff");
		expect(align.querySelector(".shift-action-label")).toHaveTextContent("Off");
		expect(align).toHaveClass("align-off");
		expect(dispatch).not.toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
		expect(server.alignSelection).toHaveBeenLastCalledWith("off");
	});

	it("keeps Align Off when the authoritative activation is rejected", async () => {
		server.alignSelection.mockRejectedValueOnce(new Error("Align unavailable"));
		render(<ParameterControls />);
		const align = screen.getByRole("button", { name: "Align Off" });

		fireEvent.click(align);

		await waitFor(() =>
			expect(server.alignSelection).toHaveBeenCalledWith("left"),
		);
		expect(align).toHaveAccessibleName("Align Off");
		expect(align).toHaveClass("align-off");
	});

	it("routes the attached-hardware Align gesture through the same authoritative mode cycle", async () => {
		server.alignSelection.mockResolvedValueOnce(undefined);
		render(<ParameterControls />);
		const align = screen.getByRole("button", { name: "Align Off" });

		window.dispatchEvent(
			new CustomEvent("light:align-action", {
				detail: { action: "align", request_id: "hardware-align-1" },
			}),
		);

		await waitFor(() =>
			expect(server.alignSelection).toHaveBeenCalledWith("left"),
		);
		expect(align).toHaveAccessibleName("Align Left");
	});
});

function visualizationSnapshot(
	values: VisualizationSnapshot["values"],
): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 1,
		blackout: false,
		preload: false,
		values,
		profile_output_values: [],
	};
}
