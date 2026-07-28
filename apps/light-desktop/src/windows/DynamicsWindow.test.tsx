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

let dynamics: Array<Record<string, unknown>> = [];
let deleteArmed = false;
const deleteDynamic = vi.fn();
const updateDynamic = vi.fn();
const resetCommand = vi.fn();
const showObjectsStore = {
	getSnapshot: () => ({ dynamics, authorityGeneration: 1 }),
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
		selected: [],
		selectedGroupId: null,
		read: () => ({ text: "" }),
		reset: resetCommand,
		replace: vi.fn(),
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
		deleteDynamic.mockReset().mockResolvedValue(undefined);
		updateDynamic.mockReset().mockImplementation(
			async (
				_showId: string,
				id: string,
				expectedRevision: number,
			) => {
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
	});

	it("uses the shared pool without pagination or implementation legend", () => {
		renderWindow();

		expect(screen.getByText("Dynamics")).toBeInTheDocument();
		expect(screen.queryByText(/Immediate, server-authoritative/i)).toBeNull();
		expect(screen.queryByLabelText(/Dynamic pool page/i)).toBeNull();
		expect(screen.getAllByText("Empty").length).toBeGreaterThan(0);
	});

	it("opens a populated Dynamic for editing from right click", () => {
		dynamics = [dynamicObject()];
		renderWindow();

		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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

	it("opens the standard creation modal before choosing the first lane", () => {
		renderWindow();

		fireEvent.click(screen.getAllByRole("button", { name: /Empty/i })[0]);

		expect(
			screen.getByRole("dialog", { name: "Create Dynamic 1" }),
		).toBeInTheDocument();
		expect(screen.getByText("Choose the first lane")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Create and edit" }),
		).toBeInTheDocument();
	});

	it("deletes an occupied Dynamic when Delete is armed", async () => {
		dynamics = [dynamicObject()];
		deleteArmed = true;
		renderWindow();

		fireEvent.click(screen.getByRole("button", { name: /Pulse/i }));

		expect(deleteDynamic).toHaveBeenCalledWith("show-test", "dynamic-1", 3);
		await vi.waitFor(() => expect(resetCommand).toHaveBeenCalled());
	});

	it("selects one lane normally and adds a lane with Shift-click", () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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

	it("uses and clears the software Shift latch for additive lane selection", () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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

	it("switches phase spread between uniform and per-lane configuration", async () => {
		dynamics = [dynamicObject({ multipleLanes: true })];
		renderWindow();
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));
		fireEvent.click(screen.getByRole("button", { name: "Phase Spread" }));

		expect(
			screen.getByRole("group", { name: "Phase spread scope" }),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Uniform" })).toHaveAttribute(
			"aria-pressed",
			"true",
		);
		expect(screen.queryByLabelText("Dynamic phase lane")).toBeNull();

		fireEvent.click(screen.getByRole("button", { name: "Per lane" }));

		await vi.waitFor(() =>
			expect(updateDynamic).toHaveBeenCalledWith(
				"show-test",
				"dynamic-1",
				3,
				{ type: "set_phase_mode", phase_mode: "per_lane" },
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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));
		fireEvent.click(screen.getByRole("button", { name: "Phase Spread" }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

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
		fireEvent.contextMenu(screen.getByRole("button", { name: /Pulse/i }));

		fireEvent.click(
			screen.getByRole("button", { name: "Intensity lane actions" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Change attribute" }));

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
