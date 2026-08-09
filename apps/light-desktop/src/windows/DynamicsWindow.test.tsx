import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppProvider, useApp } from "../state/AppContext";
import {
	createDefaultDynamicDefinition,
	createDefaultDynamicLane,
	DynamicsWindow,
} from "./DynamicsWindow";
import { lanePreview } from "./dynamics/DynamicsEditor";

let dynamics: Array<Record<string, unknown>> = [];
let deleteArmed = false;
const deleteDynamic = vi.fn();
const updateDynamic = vi.fn();
const resetCommand = vi.fn();
const replaceCommand = vi.fn();
let commandText = "";
let selected: string[] = [];
let selectedGroupId: string | null = null;
const speedGroupAction = vi.fn();
const showObjectsStore = {
	subscribe: () => () => undefined,
	getSnapshot: () => ({ dynamics, authorityGeneration: 1, showRevision: 1 }),
	beginOptimistic: vi.fn(() => crypto.randomUUID()),
	settlePending: vi.fn(),
	installObject: vi.fn(),
	rollback: vi.fn(),
};

vi.mock("../features/showObjects/ShowObjectsState", () => ({
	useDynamics: () => dynamics,
	usePresets: () => [],
	useShowObjectsStore: () => showObjectsStore,
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: () => undefined,
}));
vi.mock("../components/control/useSoundToLight", () => ({
	useSoundToLight: () => ({
		action: speedGroupAction,
	}),
}));
vi.mock("../features/speedGroupRuntime/SpeedGroupRuntimeView", () => ({
	useSpeedGroupRuntimeView: () => ({
		projection: null,
	}),
}));
vi.mock("../features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShowId: () => "show-test",
	useAttributeRegistry: () => [
		{
			id: "intensity",
			label: "Intensity",
			family: "Intensity",
			recordable: true,
			value_type: "continuous",
			normalized_min: 0,
			normalized_max: 1,
		},
		{
			id: "pan",
			label: "Pan",
			family: "Position",
			recordable: true,
			value_type: "continuous",
			normalized_min: 0,
			normalized_max: 1,
		},
	],
	useHardwareConnected: () => false,
}));
vi.mock("../features/dynamics/DynamicsActionsContext", () => ({
	useDynamicsActions: () => ({
		dynamics: {
			runtime: vi.fn().mockResolvedValue({
				global_paused: false,
				instances: [],
				definitions: [],
			}),
			toggle: vi.fn(),
		},
		showObjects: {
			deleteDynamic,
			updateDynamic,
		},
	}),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({
		selected,
		selectedGroupId,
		read: () => ({ text: commandText }),
		reset: resetCommand,
		replace: replaceCommand,
	}),
}));
vi.mock(
	"../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingCommandLineActions: () => ({ reset: resetCommand }),
		useProgrammingDeleteCommandActive: () => deleteArmed,
	}),
);

function renderWindow() {
	return render(
		<AppProvider>
			<TestShiftLatch />
			<DynamicsWindow compact={false} />
		</AppProvider>,
	);
}

function TestShiftLatch() {
	const { state, dispatch } = useApp();
	return (
		<button
			type="button"
			aria-pressed={state.shiftArmed}
			onClick={() => dispatch({ type: "SET_SHIFT_ARMED", value: true })}
		>
			Arm software Shift
		</button>
	);
}

function dynamicObject({
	multipleLanes = false,
	mode,
}: {
	multipleLanes?: boolean;
	mode?: "keyframes" | "max_min" | "middle_amplitude";
} = {}) {
	const body = createDefaultDynamicDefinition(1, "intensity", {
		definition: "dynamic-1",
		lane: "lane-1",
	});
	if (mode) body.lanes[0].mode = mode;
	if (multipleLanes) body.lanes.push(createDefaultDynamicLane("pan", "lane-2"));
	return {
		kind: "dynamic",
		id: "dynamic-1",
		revision: 3,
		updated_at: "2026-07-27T00:00:00.000Z",
		body: { ...body, name: "Pulse" },
	};
}

describe("DynamicsWindow", () => {
	afterEach(cleanup);
	beforeEach(() => {
		Object.defineProperty(window, "localStorage", {
			configurable: true,
			value: {
				getItem: vi.fn(() => null),
				setItem: vi.fn(),
				removeItem: vi.fn(),
				clear: vi.fn(),
			},
		});
		dynamics = [];
		deleteArmed = false;
		commandText = "";
		selected = [];
		selectedGroupId = null;
		replaceCommand.mockReset().mockResolvedValue(true);
		deleteDynamic.mockReset().mockResolvedValue(undefined);
		updateDynamic
			.mockReset()
			.mockImplementation(
				async (_showId: string, id: string, expectedRevision: number) => {
					const object = dynamics.find((candidate) => candidate.id === id);
					return {
						request_id: crypto.randomUUID(),
						replayed: false,
						show_id: "show-test",
						show_revision: expectedRevision + 1,
						event_sequence: expectedRevision + 1,
						object: { ...object, revision: expectedRevision + 1 },
					};
				},
			);
		resetCommand.mockReset().mockResolvedValue(true);
		speedGroupAction.mockReset().mockResolvedValue({});
	});

	it("does not manufacture an icon for a new or iconless Dynamic", () => {
		const body = createDefaultDynamicDefinition(7, "intensity", {
			definition: "dynamic-7",
			lane: "lane-7",
		});
		expect(body.icon).toBeUndefined();
		dynamics = [
			{
				kind: "dynamic",
				id: "dynamic-7",
				revision: 1,
				updated_at: "",
				body,
			},
		];
		const { container } = renderWindow();
		expect(
			container.querySelector(".dynamic-pool-card .pool-card-icon"),
		).not.toBeInTheDocument();
	});

	it("uses the shared pool without pagination or implementation legend", () => {
		renderWindow();

		expect(screen.getByText("Dynamics")).toBeInTheDocument();
		expect(screen.queryByText(/Immediate, server-authoritative/i)).toBeNull();
		expect(screen.queryByLabelText(/Dynamic pool page/i)).toBeNull();
		expect(screen.getAllByText("Empty").length).toBeGreaterThan(0);
	});

	it("keeps Shift-click as the populated Dynamic editor path", () => {
		dynamics = [dynamicObject()];
		renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		expect(
			screen.getByRole("button", { name: /Back to Pool/ }),
		).toBeInTheDocument();
		expect(
			screen.getAllByText("Intensity", { selector: "strong" }),
		).toHaveLength(1);
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(
			screen.getByRole("dialog", { name: "Dynamic Settings" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Name")).toHaveValue("Pulse");
		expect(screen.queryByRole("button", { name: "Take Selection" })).toBeNull();
		fireEvent.click(screen.getByRole("tab", { name: "Targets" }));
		expect(
			screen.getByRole("button", { name: "Take Selection" }),
		).toBeInTheDocument();
	});

	it("can take a selected Group even when no fixture selection is expanded", async () => {
		dynamics = [dynamicObject()];
		selectedGroupId = "front";
		renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), {
			shiftKey: true,
		});
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		fireEvent.click(screen.getByRole("tab", { name: "Targets" }));
		const takeSelection = screen.getByRole("button", {
			name: "Take Selection",
		});
		expect(takeSelection).toBeEnabled();
		fireEvent.click(takeSelection);
		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "set_target_binding",
					target_binding: { type: "live_group", group_id: "front" },
				}),
				undefined,
			),
		);
	});

	it("opens a populated Dynamic for editing from Shift-click", () => {
		dynamics = [dynamicObject()];
		renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), {
			shiftKey: true,
		});

		expect(
			screen.getByRole("button", { name: /Back to Pool/ }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("list", { name: "Dynamic lanes" }),
		).toBeInTheDocument();
	});

	it("uses the production selection preview and Phase footer", () => {
		dynamics = [dynamicObject()];
		const { container } = renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });
		fireEvent.click(screen.getByRole("button", { name: "Phase" }));

		expect(
			container.querySelector('aside[aria-label="Selection preview"]'),
		).toBeInTheDocument();
		expect(
			container.querySelector(
				'[role="img"][aria-label="Front-end-only preview of 400 virtual fixtures"]',
			),
		).toBeInTheDocument();
		const quickControls = screen.getByRole("group", {
			name: "Phase quick controls",
		});
		expect(
			within(quickControls).getByRole("radio", { name: "Linear" }),
		).toBeInTheDocument();
		expect(
			within(quickControls).getByRole("button", { name: "Take Selection" }),
		).toBeDisabled();
	});

	it("sends speed-group taps to the selected group", async () => {
		const object = dynamicObject();
		object.body.speed = {
			type: "speed_group",
			group: "A",
			beats_per_cycle: { numerator: 4, denominator: 1 },
		};
		dynamics = [object];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });
		fireEvent.click(screen.getByRole("button", { name: "Speed" }));

		fireEvent.click(
			screen.getByRole("button", {
				name: "Tap Speed Group A tempo, 120 BPM",
			}),
		);
		await vi.waitFor(() =>
			expect(speedGroupAction).toHaveBeenCalledWith("A", {
				action: "learn",
				captured_at_millis: expect.any(Number),
			}),
		);
	});

	it("shows the defensive first-lane action for a zero-lane projection", async () => {
		const object = dynamicObject();
		object.body.lanes = [];
		dynamics = [object];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		fireEvent.click(screen.getByRole("button", { name: "Add first lane" }));
		const dialog = screen.getByRole("dialog", {
			name: "Select lane attribute",
		});
		fireEvent.click(within(dialog).getByRole("button", { name: "Pan" }));

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "add_lane",
					lane: expect.objectContaining({ attribute: "pan" }),
				}),
				undefined,
			),
		);
	});

	it("uses the lane attribute picker when creating a Dynamic", () => {
		renderWindow();

		fireEvent.click(screen.getAllByRole("button", { name: /Empty/i })[0]);

		const dialog = screen.getByRole("dialog", {
			name: "Select lane attribute",
		});
		expect(dialog).toBeInTheDocument();
		expect(within(dialog).getByText("Create Dynamic 1")).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: "Intensity" }),
		).toBeInTheDocument();
		expect(
			within(dialog).getByRole("button", { name: "Pan" }),
		).toBeInTheDocument();
		expect(within(dialog).queryByLabelText("First lane")).toBeNull();
		expect(
			within(dialog).queryByRole("button", { name: "Create and edit" }),
		).toBeNull();
	});

	it("uses the lane attribute picker before adding a lane", async () => {
		dynamics = [dynamicObject()];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		fireEvent.click(screen.getByRole("button", { name: "+ Add Lane" }));
		const dialog = screen.getByRole("dialog", {
			name: "Select lane attribute",
		});
		fireEvent.click(within(dialog).getByRole("button", { name: "Pan" }));

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "add_lane",
					lane: expect.objectContaining({ attribute: "pan" }),
				}),
				undefined,
			),
		);
	});

	it("deletes an occupied Dynamic when Delete is armed", async () => {
		dynamics = [dynamicObject()];
		deleteArmed = true;
		renderWindow();

		const tile = screen.getByRole("button", { name: /Pulse/i });
		expect(tile).toHaveClass("delete-target");
		expect(tile).toHaveTextContent("Delete");
		fireEvent.click(tile);

		expect(deleteDynamic).toHaveBeenCalledWith("show-test", "dynamic-1", 3);
		await vi.waitFor(() => expect(resetCommand).toHaveBeenCalled());
	});

	it("outlines and targets an occupied Dynamic from the bare SET command", () => {
		dynamics = [dynamicObject()];
		commandText = "SET";
		renderWindow();

		const tile = screen.getByRole("button", { name: /Pulse/i });
		expect(tile).toHaveClass("set-target");
		expect(tile).toHaveTextContent("Set");
		fireEvent.click(tile);
		expect(replaceCommand).toHaveBeenCalledWith("SET DYNAMIC 1");
	});

	it("uses the SET Dynamic action for right-click and suppresses the native menu", () => {
		dynamics = [dynamicObject()];
		renderWindow();

		const tile = screen.getByRole("button", { name: /Pulse/i });
		const contextMenu = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});
		tile.dispatchEvent(contextMenu);

		expect(contextMenu.defaultPrevented).toBe(true);
		expect(replaceCommand).toHaveBeenCalledWith("SET DYNAMIC 1");
		expect(
			screen.queryByRole("button", { name: /Back to Pool/ }),
		).toBeNull();
	});

	it("selects one lane normally and adds a lane with Shift-click", () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const laneList = screen.getByRole("list", { name: "Dynamic lanes" });
		const laneOneIdentity = within(laneList).getByRole("button", {
			name: "Select lane 1, Intensity",
		});
		const laneOneCurve = within(laneList).getByRole("button", {
			name: "Select Intensity lane from curve",
		});
		const laneTwoIdentity = within(laneList).getByRole("button", {
			name: "Select lane 2, Pan",
		});
		const laneTwoCurve = within(laneList).getByRole("button", {
			name: "Select Pan lane from curve",
		});
		expect(laneOneIdentity).toHaveAttribute("aria-pressed", "true");
		expect(laneOneCurve).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoIdentity).toHaveAttribute("aria-pressed", "false");
		expect(laneTwoCurve).toHaveAttribute("aria-pressed", "false");

		fireEvent.click(laneTwoCurve);
		expect(laneOneIdentity).toHaveAttribute("aria-pressed", "false");
		expect(laneOneCurve).toHaveAttribute("aria-pressed", "false");
		expect(laneTwoIdentity).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoCurve).toHaveAttribute("aria-pressed", "true");

		fireEvent.click(laneOneIdentity, { shiftKey: true });
		expect(laneOneIdentity).toHaveAttribute("aria-pressed", "true");
		expect(laneOneCurve).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoIdentity).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoCurve).toHaveAttribute("aria-pressed", "true");
		expect(laneList.querySelector("button button")).toBeNull();
	});

	it("keeps keyframe pointer interaction isolated from lane background selection", () => {
		const object = dynamicObject({ multipleLanes: true, mode: "keyframes" });
		object.body.lanes[1].mode = "keyframes";
		dynamics = [object];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const laneList = screen.getByRole("list", { name: "Dynamic lanes" });
		const laneOneIdentity = within(laneList).getByRole("button", {
			name: "Select lane 1, Intensity",
		});
		const laneTwoIdentity = within(laneList).getByRole("button", {
			name: "Select lane 2, Pan",
		});
		const laneTwoCurve = within(laneList).getByRole("button", {
			name: "Select Pan lane from curve",
		});
		const keyframe = within(laneList).getByRole("button", {
			name: "Pan keyframe B",
		});
		const setPointerCapture = vi.fn();
		Object.defineProperty(keyframe, "setPointerCapture", {
			configurable: true,
			value: setPointerCapture,
		});

		fireEvent.pointerDown(keyframe, { pointerId: 17 });

		expect(setPointerCapture).toHaveBeenCalledWith(17);
		expect(laneOneIdentity).toHaveAttribute("aria-pressed", "false");
		expect(laneTwoIdentity).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoCurve).toHaveAttribute("aria-pressed", "true");
		expect(laneList.querySelector("button button")).toBeNull();
	});

	it("maps a dragged keyframe center to the cursor without horizontal drift", async () => {
		dynamics = [dynamicObject({ mode: "keyframes" })];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const keyframe = screen.getByRole("button", {
			name: "Intensity keyframe B",
		});
		const timeline = keyframe.parentElement;
		expect(timeline).not.toBeNull();
		Object.defineProperty(keyframe, "setPointerCapture", {
			configurable: true,
			value: vi.fn(),
		});
		Object.defineProperty(keyframe, "getBoundingClientRect", {
			configurable: true,
			value: () => ({
				left: 190,
				width: 50,
				right: 240,
				top: 0,
				bottom: 50,
				height: 50,
				x: 190,
				y: 0,
				toJSON: () => undefined,
			}),
		});
		Object.defineProperty(timeline, "getBoundingClientRect", {
			configurable: true,
			value: () => ({
				left: 100,
				width: 1000,
				right: 1100,
				top: 0,
				bottom: 120,
				height: 120,
				x: 100,
				y: 0,
				toJSON: () => undefined,
			}),
		});

		fireEvent.pointerDown(keyframe, { pointerId: 9, clientX: 215 });
		fireEvent.pointerMove(keyframe, { pointerId: 9, clientX: 500 });

		await vi.waitFor(() => {
			const intent = updateDynamic.mock.calls.at(-1)?.[3];
			expect(intent).toEqual(
				expect.objectContaining({
					type: "replace_lane",
					lane: expect.objectContaining({
						keyframes: expect.objectContaining({
							points: [
								expect.objectContaining({ position: 0 }),
								expect.objectContaining({ position: expect.closeTo(0.398, 3) }),
							],
						}),
					}),
				}),
			);
		});
	});

	it("uses and clears the software Shift latch for additive lane selection", () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const shiftLatch = screen.getByRole("button", {
			name: "Arm software Shift",
		});
		const laneTwoIdentity = screen.getByRole("button", {
			name: "Select lane 2, Pan",
		});
		fireEvent.click(shiftLatch);
		expect(shiftLatch).toHaveAttribute("aria-pressed", "true");

		fireEvent.click(laneTwoIdentity);

		expect(
			screen.getByRole("button", { name: "Select lane 1, Intensity" }),
		).toHaveAttribute("aria-pressed", "true");
		expect(laneTwoIdentity).toHaveAttribute("aria-pressed", "true");
		expect(shiftLatch).toHaveAttribute("aria-pressed", "false");
	});

	it("uses an ordering-mode toggle without exposing uniform or per-lane scope", async () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });
		fireEvent.click(screen.getByRole("button", { name: "Phase" }));

		expect(
			screen.getByRole("radiogroup", { name: "Ordering mode" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Uniform" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Per lane" })).toBeNull();
		expect(screen.queryByLabelText("Dynamic phase lane")).toBeNull();

		fireEvent.click(screen.getByRole("radio", { name: "Grid" }));

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "set_phase",
					phase: expect.objectContaining({
						ordering: expect.objectContaining({ type: "grid_linear" }),
					}),
				}),
				undefined,
			),
		);
	});

	it("selects which lane is edited in per-lane phase spread mode", () => {
		const object = dynamicObject({ multipleLanes: true });
		object.body.phase_mode = "per_lane";
		object.body.lanes[0].phase = {
			...object.body.phase,
			span_degrees: 180,
		};
		object.body.lanes[1].phase = {
			...object.body.phase,
			span_degrees: 720,
		};
		dynamics = [object];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });
		fireEvent.click(screen.getByRole("button", { name: "Phase" }));

		const lanePicker = screen.getByLabelText("Dynamic phase lane");
		expect(lanePicker).toHaveTextContent("Lane 1 · intensity");
		expect(screen.getByLabelText("Span")).toHaveValue("180");

		fireEvent.click(lanePicker);
		fireEvent.click(screen.getByRole("option", { name: "Lane 2 · pan" }));

		expect(screen.getByLabelText("Dynamic phase lane")).toHaveTextContent(
			"Lane 2 · pan",
		);
		expect(screen.getByLabelText("Span")).toHaveValue("720");
	});

	it("shows every keyframe as a directly selectable card", () => {
		dynamics = [dynamicObject({ mode: "keyframes" })];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const choices = screen.getByRole("group", { name: "Selected keyframe" });
		const first = within(choices).getByRole("button", { name: /^A,/ });
		const second = within(choices).getByRole("button", { name: /^B,/ });
		expect(first).toHaveAttribute("aria-pressed", "true");
		expect(second).toHaveAttribute("aria-pressed", "false");
		expect(within(choices).getByRole("note")).toHaveAccessibleName(
			"A prime, 100%, alias of A",
		);

		fireEvent.click(second);

		expect(first).toHaveAttribute("aria-pressed", "false");
		expect(second).toHaveAttribute("aria-pressed", "true");
	});

	it("deletes the selected non-closing keyframe", async () => {
		const object = dynamicObject({ mode: "keyframes" });
		const lane = object.body.lanes[0];
		lane.keyframes.points.push({
			...lane.keyframes.points[1],
			position: 0.75,
		});
		dynamics = [object];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		expect(
			screen.getByRole("button", { name: "Delete keyframe A" }),
		).toBeDisabled();
		fireEvent.click(
			screen.getByRole("button", {
				name: /^B,/,
			}),
		);
		const deleteButton = screen.getByRole("button", {
			name: "Delete keyframe B",
		});
		expect(deleteButton).toBeEnabled();
		fireEvent.click(deleteButton);

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "replace_lane",
					lane_id: "lane-1",
					lane: expect.objectContaining({
						keyframes: expect.objectContaining({
							points: [
								expect.objectContaining({ position: 0 }),
								expect.objectContaining({ position: 0.75 }),
							],
						}),
					}),
				}),
				undefined,
			),
		);
	});

	it("cycles the vertically stacked Curve Composer method", async () => {
		dynamics = [dynamicObject()];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		const composer = screen.getByRole("region", { name: "Curve Composer" });
		expect(within(composer).queryByText("Curve")).not.toBeInTheDocument();
		expect(within(composer).queryByText("Composer")).not.toBeInTheDocument();
		const method = within(composer).getByRole("button", {
			name: "Curve method: Max / min. Press to select Middle / amplitude.",
		});
		expect(within(method).getByText("Max / min")).toHaveClass("is-active");
		expect(within(method).getByText("Keyframes")).not.toHaveClass("is-active");

		fireEvent.click(method);

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "replace_lane",
					lane_id: "lane-1",
					lane: expect.objectContaining({ mode: "middle_amplitude" }),
				}),
				undefined,
			),
		);
	});

	it("chooses a curve function from the grouped function modal", async () => {
		dynamics = [dynamicObject()];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		fireEvent.click(
			screen.getByRole("button", { name: "Curve function: Sinus" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Choose curve function",
		});
		expect(dialog.querySelectorAll(".dynamic-function-icon")).toHaveLength(6);
		expect(within(dialog).getByText(/Shaped pulse/)).toBeInTheDocument();
		fireEvent.click(
			within(dialog).getByRole("button", {
				name: /^PWM/,
			}),
		);

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "replace_lane",
					lane_id: "lane-1",
					lane: expect.objectContaining({
						mode: "max_min",
						width: 1,
						max_min: expect.objectContaining({ function: "pwm" }),
					}),
				}),
				undefined,
			),
		);
	});

	it("opens Change Attribute from the lane menu and submits the selected attribute", async () => {
		dynamics = [dynamicObject()];
		renderWindow();
		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }), { shiftKey: true });

		fireEvent.click(
			screen.getByRole("button", { name: "Intensity lane settings" }),
		);
		const menu = screen.getByRole("menu", { name: "Intensity lane menu" });
		expect(
			within(menu).getByRole("menuitem", { name: "Delete lane" }),
		).toBeDisabled();
		fireEvent.click(
			within(menu).getByRole("menuitem", { name: "Change attribute" }),
		);

		const dialog = screen.getByRole("dialog", {
			name: "Change lane attribute",
		});
		expect(dialog).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole("button", { name: "Pan" }));

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				expect.objectContaining({
					type: "replace_lane",
					lane_id: "lane-1",
					lane: expect.objectContaining({ attribute: "pan" }),
				}),
				undefined,
			),
		);
	});
});

it("mirrors a migrated middle-amplitude waveform in the curve preview", () => {
	const lane = createDefaultDynamicLane("intensity", "lane-preview");
	lane.mode = "middle_amplitude";
	lane.middle_amplitude.middle = { type: "value", value: 0.5 };
	lane.middle_amplitude.amplitude = 0.25;
	const ordinary = lanePreview(lane, [lane]);

	lane.middle_amplitude.invert_waveform = true;
	const inverted = lanePreview(lane, [lane]);

	expect(inverted.primaryPath).not.toBe(ordinary.primaryPath);
});
