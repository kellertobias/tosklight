import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
} from "../../features/programmerValues/contracts";
import { ParameterControls } from "./ParameterControls";

const state = {
	stageMode: "select",
	builtIn: null as string | null,
	desks: [],
	activeDeskId: "programming",
	preload: "idle",
	shiftArmed: false,
};
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
		userId: "operator",
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
}));
const preloadValuesActions = vi.hoisted(() => ({
	batch: vi.fn(async () => null),
}));
const legacyProgrammerValuesAccess = vi.fn();
const server = {
	selectedFixtures: [] as string[],
	selectedGroupId: null as string | null,
	groups: [] as Array<Record<string, unknown>>,
	patch: { fixtures: [] as Array<Record<string, unknown>> },
	bootstrap: { hardware_connected: false } as {
		hardware_connected: boolean;
		readonly active_programmers: unknown[];
	},
	session: { session_id: "session-1", user: { id: "operator" } },
	readVisualization: vi.fn().mockResolvedValue({ values: [] }),
	alignSelection: vi.fn(),
	setProgrammer: vi.fn(),
	setProgrammerMany: vi.fn(),
	setProgrammerValue: vi.fn(),
	controlFixtureAction: vi.fn(),
	generateFixturePresets: vi.fn().mockResolvedValue({ created: [] }),
	setGroupValue: vi.fn(),
	releaseProgrammer: vi.fn(),
	releaseGroupValue: vi.fn(),
};
Object.defineProperty(server.bootstrap, "active_programmers", {
	get() {
		legacyProgrammerValuesAccess();
		return [];
	},
});

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch }),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
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
	}),
);
vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => server.groups,
}));
vi.mock("./parameterControls/useParameterProgrammerValues", () => ({
	useParameterProgrammerValues: (
		_fixtureIds: readonly string[],
		_groupId: string | null,
		enabled: boolean,
	) => (enabled ? programmerValues.view : null),
}));

afterEach(() => {
	cleanup();
	state.stageMode = "select";
	state.builtIn = null;
	state.desks = [];
	state.shiftArmed = false;
	server.selectedFixtures = [];
	server.selectedGroupId = null;
	server.groups = [];
	server.patch.fixtures = [];
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
	vi.clearAllMocks();
});

describe("ParameterControls projection lifecycle", () => {
	it("does not mount the visualization projection behind Stage command controls", () => {
		state.stageMode = "setup";
		state.builtIn = "stage";
		server.selectedFixtures = ["fixture-1"];

		render(<ParameterControls />);

		expect(server.readVisualization).not.toHaveBeenCalled();
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
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);

		expect(
			screen.getByRole("button", { name: "Dots indexed value" }),
		).toBeDisabled();
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
		const fader = screen.getByRole("slider", { name: "Enc 1 · Dimmer" });
		expect(fader).toHaveValue("25");
		fireEvent.input(fader, { target: { value: "50" } });
		fireEvent.pointerUp(fader);

		expect(preloadValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "set_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.5 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
		expect(normalValuesActions.batch).not.toHaveBeenCalled();
		expect(server.setProgrammer).not.toHaveBeenCalled();
	});
});

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
								],
							},
						],
						control_actions: [
							{
								id: "action-1",
								name: "Lamp reset",
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

	it("keeps six numbered hardware slots and accumulates fine and coarse turns", async () => {
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
				detail: { control: "encode/1", value: "up" },
			}),
		);
		expect(normalValuesActions.batch).toHaveBeenLastCalledWith({
			requestId: expect.any(String),
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
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "right" },
			}),
		);
		await vi.waitFor(() =>
			expect(normalValuesActions.batch).toHaveBeenLastCalledWith({
				requestId: expect.any(String),
				mutations: [
					{
						action: "set_fixture",
						fixtureId: "fixture-1",
						attribute: "intensity",
						value: { kind: "normalized", value: 0.11 },
						timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
					},
				],
			}),
		);

		captureMode.projection.blind = true;
		rendered.rerender(<ParameterControls />);
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "up" },
			}),
		);
		expect(preloadValuesActions.batch).toHaveBeenLastCalledWith({
			requestId: expect.any(String),
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

		expect(normalValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "set_fixture",
					fixtureId: "fixture-3",
					attribute: "intensity",
					value: { kind: "normalized", value: 0 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
				{
					action: "set_fixture",
					fixtureId: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.25 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
				{
					action: "set_fixture",
					fixtureId: "fixture-2",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.5 },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
	});

	it("clears all physical encoder mappings in Direct mode", () => {
		server.bootstrap.hardware_connected = true;
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];
		render(<ParameterControls />);
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);
		for (let slot = 1; slot <= 6; slot += 1)
			expect(
				screen.getByLabelText(`Encoder ${slot} unassigned`),
			).toBeInTheDocument();
		expect(screen.queryByLabelText(/Encoder 1:/)).not.toBeInTheDocument();
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
							parameters: [{ attribute: "control.reset", capabilities: [] }],
						},
					],
				},
			},
		];
		programmerValues.view.fixtureValues = [
			{
				fixtureId: "fixture-1",
				attribute: "control.reset",
				value: { kind: "discrete", value: "fixture.reset.safe" },
				programmerOrder: 1,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		];
		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Control" }));
		expect(
			screen.getByLabelText("Encoder 1: control reset, fixture.reset.safe"),
		).not.toHaveTextContent("Built-in");
		expect(
			screen.queryByRole("button", { name: "Set value for control reset" }),
		).not.toBeInTheDocument();
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
		server.readVisualization.mockResolvedValue({
			values: [
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
			],
		});

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
		fireEvent.click(screen.getByRole("button", { name: "Release Dimmer" }));
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
		expect(server.releaseProgrammer).not.toHaveBeenCalled();
		expect(server.releaseGroupValue).not.toHaveBeenCalled();
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
		server.readVisualization.mockResolvedValue({
			values: [
				{
					fixture_id: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0 },
				},
			],
		});

		render(<ParameterControls />);

		expect(await screen.findByText("100%")).toBeInTheDocument();
	});
});

describe("ParameterControls Group targets and alignment", () => {
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
		server.readVisualization.mockResolvedValue({
			values: [
				{
					fixture_id: "fixture-1",
					attribute: "intensity",
					value: { kind: "normalized", value: 0 },
				},
			],
		});

		render(<ParameterControls />);

		expect(await screen.findByText("75%")).toBeInTheDocument();
	});

	it("starts off, cycles Out, Center, Left, Right, and Shift+Align turns it off", () => {
		render(<ParameterControls />);
		fireEvent.click(screen.getByRole("button", { name: "Position" }));
		const align = screen.getByRole("button", { name: "Align Off" });
		expect(align).toHaveClass("align-off");

		for (const mode of ["out", "center", "left", "right"] as const) {
			fireEvent.click(align);
			expect(server.alignSelection).toHaveBeenLastCalledWith("pan", mode);
			expect(align).toHaveAccessibleName(
				`Align ${mode[0].toUpperCase()}${mode.slice(1)}`,
			);
			expect(align).toHaveClass("align-active");
		}

		state.shiftArmed = true;
		fireEvent.click(align);
		expect(align).toHaveAccessibleName("Align Off");
		expect(align).toHaveClass("align-off");
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
		expect(server.alignSelection).toHaveBeenCalledTimes(4);
	});
});

describe("ParameterControls schema-v2 direct picker", () => {
	it("programs indexed values by stable semantic ID", () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];

		render(<ParameterControls />);
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Dots indexed value" }));

		expect(normalValuesActions.batch).toHaveBeenCalledWith({
			requestId: expect.any(String),
			mutations: [
				{
					action: "set_fixture",
					fixtureId: "fixture-1",
					attribute: "gobo.1",
					value: { kind: "discrete", value: "gobo.dots" },
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
		expect(server.setProgrammerValue).not.toHaveBeenCalled();
	});

	it("holds and releases every assignment through one typed momentary action", () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];

		render(<ParameterControls />);
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);
		const action = screen.getByRole("button", {
			name: "Lamp reset momentary control action",
		});
		fireEvent.pointerDown(action, { pointerId: 7 });
		fireEvent.pointerUp(action, { pointerId: 7 });

		expect(server.controlFixtureAction.mock.calls).toEqual([
			["fixture-1", "action-1", true],
			["fixture-1", "action-1", false],
		]);
	});

	it("toggles latched actions and lets the server own timed-pulse release", () => {
		const fixture = schemaV2Fixture();
		fixture.definition.profile_snapshot.modes[0].control_actions = [
			{
				id: "action-latched",
				name: "Lamp power",
				kind: "latched",
				duration_millis: null,
				assignments: [
					{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 },
				],
			},
			{
				id: "action-pulse",
				name: "Fixture reset",
				kind: "timed_pulse",
				duration_millis: 750,
				assignments: [
					{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 },
				],
			},
		];
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [fixture];

		render(<ParameterControls />);
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);
		const latched = screen.getByRole("button", {
			name: "Lamp power latched control action",
		});
		fireEvent.click(latched);
		fireEvent.click(latched);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Fixture reset timed_pulse control action",
			}),
		);

		expect(server.controlFixtureAction.mock.calls).toEqual([
			["fixture-1", "action-latched", true],
			["fixture-1", "action-latched", false],
			["fixture-1", "action-pulse", true],
		]);
	});

	it("creates portable presets only after the explicit operator action", async () => {
		server.selectedFixtures = ["fixture-1"];
		server.patch.fixtures = [schemaV2Fixture()];
		server.generateFixturePresets.mockResolvedValueOnce({
			created: [
				{
					address: { family: "Beam", number: 1 },
					number: 1,
					name: "Dots",
					family: "Beam",
				},
			],
		});

		render(<ParameterControls />);
		expect(server.generateFixturePresets).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Direct values and actions" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Generate portable presets" }),
		);

		expect(server.generateFixturePresets).toHaveBeenCalledWith(["fixture-1"]);
		expect(await screen.findByRole("status")).toHaveTextContent(
			"Created 1 portable preset",
		);
	});
});
