import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../api/familyEncoderModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
	pagesSnapshot,
} from "../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import { resolveSpecialDialog } from "../registry/specialDialogRegistry";
import { PositionSpecialDialog } from "./PositionSpecialDialog";

const desk = vi.hoisted(() => ({
	normal: null as unknown,
	preload: null as unknown,
	preloadMode: false,
	ready: true,
	groupId: null as string | null,
	values: [] as unknown[],
}));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => desk.normal,
}));
vi.mock("../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView", () => ({
	useProgrammerPreloadValuesActions: () => desk.preload,
}));
vi.mock("../../../../features/programmerCaptureMode/ProgrammerCaptureModeView", () => ({
	useProgrammerCaptureModeView: () => ({ blind: desk.preloadMode, preloadCaptureProgrammer: desk.preloadMode }),
}));
vi.mock("../../../../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: () => ({ selected: [FIXTURE_A, FIXTURE_B], revision: 1 }),
}));
vi.mock("../../../../features/programmingInteraction/contracts", () => ({
	selectedGroupId: () => desk.groupId,
}));
vi.mock("../../../control/parameterControls/useParameterProgrammerValues", () => ({
	useParameterProgrammerValues: (_ids: unknown, _group: unknown, enabled: boolean) =>
		enabled ? { ready: desk.ready, fixtureValues: desk.values, groupValues: [], dynamicValues: [] } : null,
}));
vi.mock("../../../control/parameterControls/useParameterPreloadValues", () => ({
	useParameterPreloadValues: (_ids: unknown, _group: unknown, enabled: boolean) =>
		enabled ? { ready: desk.ready, fixtureValues: desk.values, groupValues: [], dynamicValues: [] } : null,
}));
vi.mock("../../../../features/configuration/ConfigurationState", async (original) => ({
	...(await original<object>()),
	useProgrammerFadeMillis: () => 2_000,
	useColorPresentation: () => "easy_rgbw",
}));

const LEASE = 41;
const JOY = 440;
const REACH = JOY / 2 - 26;
let frames: Map<number, FrameRequestCallback>;
let nextFrame: number;
let clock: number;
let captured: Set<number>;

function readout(lane: "normal" | "preload", pan: [number, number], tilt: [number, number]): OutputReadoutSnapshot {
	return {
		lane,
		scope: {},
		revision: 1,
		lease: LEASE,
		owners: [FIXTURE_A, FIXTURE_B].map((fixture_id, index) => ({
			fixture_id,
			position: {
				available: true,
				commands: [],
				common: { pan_degrees: pan[index] ?? 0, tilt_degrees: tilt[index] ?? 0 },
			},
		})),
	} as unknown as OutputReadoutSnapshot;
}

function context(semantic: boolean, snapshot: OutputReadoutSnapshot): FamilyEncodersContextValue {
	return {
		loadPages: async () => pagesSnapshot(semantic),
		readouts: new DisplayedSourceReadouts({ request: async () => snapshot }),
		session: null,
	};
}

const targetValue = (fixtureId: string) => ({
	fixtureId,
	attribute: "position",
	value: { kind: "position", value: { kind: "target", reference: { kind: "origin" }, offset_metres: [{ kind: "value", value: 0 }, { kind: "value", value: 0 }, { kind: "value", value: 0 }] } },
});
const angleValue = (fixtureId: string, pan: number, tilt: number) => ({
	fixtureId,
	attribute: "position",
	value: { kind: "position", value: { kind: "angles", pan_degrees: { kind: "value", value: pan }, tilt_degrees: { kind: "value", value: tilt } } },
});

beforeEach(() => {
	frames = new Map(); nextFrame = 1; clock = 1000; captured = new Set();
	vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { const id = nextFrame++; frames.set(id, callback); return id; });
	vi.stubGlobal("cancelAnimationFrame", (id: number) => { frames.delete(id); });
	vi.spyOn(performance, "now").mockImplementation(() => clock);
	Element.prototype.setPointerCapture = vi.fn(function (id: number) { captured.add(id); });
	Element.prototype.hasPointerCapture = vi.fn((id: number) => captured.has(id));
	Element.prototype.releasePointerCapture = vi.fn((id: number) => { captured.delete(id); });
	desk.normal = fakeWriter();
	desk.preload = fakeWriter();
	desk.preloadMode = false;
	desk.ready = true;
	desk.groupId = null;
	desk.values = [targetValue(FIXTURE_A), targetValue(FIXTURE_B)];
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function flush(ms = 16) {
	clock += ms;
	const pending = [...frames.values()];
	frames.clear();
	act(() => pending.forEach((callback) => callback(clock)));
}

type Writer = ReturnType<typeof fakeWriter>;
const normal = () => desk.normal as Writer;
const preload = () => desk.preload as Writer;
const edits = (writer: Writer) =>
	writer.applyIntent.mock.calls.map(([input]) => (input as { operation: { edits: unknown[] } }).operation.edits);

async function mount(options: { semantic?: boolean; snapshot?: OutputReadoutSnapshot; enabled?: boolean; value?: FamilyEncodersContextValue } = {}) {
	const close = vi.fn();
	const value = options.value ?? context(options.semantic ?? true, options.snapshot ?? readout("normal", [30, 30], [10, 10]));
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>{children}</FamilyEncodersContextProvider>
	);
	const view = render(<PositionSpecialDialog family="Position" selectedFixtureIds={[FIXTURE_A, FIXTURE_B]} close={close} />, { wrapper });
	flush(0);
	const pan = screen.getByTestId("pan-circle");
	if (options.enabled ?? true) await waitFor(() => expect(pan).not.toHaveAttribute("aria-disabled"));
	else await act(async () => undefined);
	const joystick = screen.getByTestId("position-joystick");
	vi.spyOn(joystick, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, right: JOY, bottom: JOY, width: JOY, height: JOY, toJSON: () => ({}) });
	return { close, view, pan, joystick };
}

const at = (x: number, y: number) => ({ pointerId: 7, clientX: JOY / 2 + x * REACH, clientY: JOY / 2 + y * REACH, button: 0, pointerType: "touch" });
function hold(joystick: HTMLElement, x = 1, y = 0) {
	fireEvent.pointerDown(joystick, at(x, y));
	flush(16);
	flush(16);
	flush(16);
}

describe("production Position Special Dialog", () => {
	it("is the semantic Position registry entry", () => {
		expect(resolveSpecialDialog("Position", true)).toMatchObject({ mode: "semantic", Component: PositionSpecialDialog });
		expect(resolveSpecialDialog("Position", false)?.mode).toBe("legacy");
	});

	it("shows the resolved angles while Target is active and sends nothing on open, focus or close", async () => {
		const { close, pan, joystick } = await mount();
		expect(pan).toHaveAttribute("aria-valuenow", "30");
		expect(screen.getByRole("slider", { name: "Tilt angle" })).toHaveAttribute("aria-valuenow", "10");
		expect(screen.getByTestId("pan-value-caption")).toHaveTextContent("Resolved");
		expect(screen.getByTestId("tilt-value-caption")).toHaveTextContent("Resolved");
		pan.focus(); joystick.focus();
		fireEvent.click(screen.getByRole("button", { name: "Close Position Special Dialog" }));
		expect(close).toHaveBeenCalled();
		for (const call of Object.values(normal())) expect(call).not.toHaveBeenCalled();
	});

	it("adopts the displayed pose once: the first joystick edit takes over Target with the displayed source", async () => {
		const { joystick } = await mount();
		hold(joystick);
		const sent = edits(normal());
		expect(sent.length).toBeGreaterThan(1);
		expect(sent[0]).toEqual([{ kind: "activate_angles" }, { kind: "scalar", component: { kind: "pan" }, operation: { kind: "set", value: { kind: "value", value: expect.any(Number) } } }]);
		for (const later of sent.slice(1)) expect(later).not.toContainEqual({ kind: "activate_angles" });
		for (const [input] of normal().applyIntent.mock.calls)
			expect(input).toMatchObject({ attribute: "position", fixtureIds: [FIXTURE_A, FIXTURE_B], displayedSource: { lane: "normal", lease: LEASE }, timing: { fade: false } });
		const undoGroups = new Set(normal().applyIntent.mock.calls.map(([input]) => (input as { undoGroup: string }).undoGroup));
		expect(undoGroups.size).toBe(1);
		fireEvent.pointerUp(joystick, at(1, 0));
		expect(normal().finishGesture).toHaveBeenCalledTimes(1);
		expect(normal().finishGesture.mock.calls[0]?.[0]).toMatchObject({ attribute: "position", undoGroup: [...undoGroups][0] });
	});

	it("keeps moving while held without pointer moves, stops on centre and keeps the gesture open", async () => {
		const { joystick } = await mount();
		hold(joystick);
		const moving = normal().applyIntent.mock.calls.length;
		flush(16); flush(16);
		expect(normal().applyIntent.mock.calls.length).toBeGreaterThan(moving);
		fireEvent.pointerMove(joystick, at(0, 0));
		const centred = normal().applyIntent.mock.calls.length;
		flush(500); flush(500);
		expect(normal().applyIntent.mock.calls.length).toBe(centred);
		expect(normal().finishGesture).not.toHaveBeenCalled();
		fireEvent.pointerUp(joystick, at(0, 0));
		expect(normal().finishGesture).toHaveBeenCalledTimes(1);
	});

	const stops: Array<[string, (joystick: HTMLElement, view: { unmount(): void }) => void]> = [
		["release", (joystick) => fireEvent.pointerUp(joystick, at(1, 0))],
		["pointer cancel", (joystick) => fireEvent.pointerCancel(joystick, at(1, 0))],
		["lost capture", (joystick) => fireEvent(joystick, new PointerEvent("lostpointercapture", { pointerId: 7, bubbles: true }))],
		["window blur", () => fireEvent.blur(window)],
		["document hidden", () => {
			Object.defineProperty(document, "hidden", { configurable: true, value: true });
			Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
			fireEvent(document, new Event("visibilitychange"));
		}],
		["close", () => fireEvent.click(screen.getByRole("button", { name: "Close Position Special Dialog" }))],
		["Escape", () => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" })],
		["unmount", (_joystick, view) => view.unmount()],
	];
	it.each(stops)("stops the held joystick on %s with exactly one Finish and no later edit", async (_name, stop) => {
		const { joystick, view } = await mount();
		hold(joystick);
		expect(normal().applyIntent).toHaveBeenCalled();
		act(() => stop(joystick, view));
		const after = normal().applyIntent.mock.calls.length;
		flush(100); flush(100); flush(100);
		expect(normal().applyIntent.mock.calls.length).toBe(after);
		expect(normal().cancelGesture).toHaveBeenCalledTimes(1);
		expect(normal().finishGesture).toHaveBeenCalledTimes(1);
		if (joystick.isConnected) expect(joystick).toHaveAttribute("data-active", "false");
		Reflect.deleteProperty(document, "hidden");
		Reflect.deleteProperty(document, "visibilityState");
	});

	it("authors requested Angles absolutely without a Target takeover: +90° is one complete gesture", async () => {
		desk.values = [angleValue(FIXTURE_A, 20, 5), angleValue(FIXTURE_B, 20, 5)];
		const { pan } = await mount();
		expect(pan).toHaveAttribute("aria-valuenow", "20");
		expect(screen.queryByTestId("pan-value-caption")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Increase pan by 90 degrees" }));
		expect(edits(normal())).toEqual([[{ kind: "scalar", component: { kind: "pan" }, operation: { kind: "set", value: { kind: "value", value: 110 } } }]]);
		expect(normal().finishGesture).toHaveBeenCalledTimes(1);
		expect(pan).toHaveAttribute("aria-valuenow", "110");
	});

	it("edits a Mixed selection relatively instead of averaging", async () => {
		desk.values = [angleValue(FIXTURE_A, 20, 5), angleValue(FIXTURE_B, 60, 5)];
		const { pan } = await mount();
		expect(screen.getByTestId("pan-value-caption")).toHaveTextContent("Relative");
		expect(pan).toHaveAttribute("aria-valuenow", "0");
		fireEvent.click(screen.getByRole("button", { name: "Decrease pan by 90 degrees" }));
		expect(edits(normal())).toEqual([[{ kind: "scalar", component: { kind: "pan" }, operation: { kind: "relative", value: -90 } }]]);
	});

	it("writes Preload through the Preload writer with the Programmer fade and its own lease", async () => {
		desk.preloadMode = true;
		const { joystick } = await mount({ snapshot: readout("preload", [30, 30], [10, 10]) });
		hold(joystick, 0, -1);
		fireEvent.pointerUp(joystick, at(0, -1));
		expect(normal().applyIntent).not.toHaveBeenCalled();
		expect(normal().finishGesture).not.toHaveBeenCalled();
		expect(preload().applyIntent.mock.calls[0]?.[0]).toMatchObject({
			displayedSource: { lane: "preload", lease: LEASE },
			timing: { fade: true, fadeMillis: 2_000 },
			operation: { edits: [{ kind: "activate_angles" }, { kind: "scalar", component: { kind: "tilt" } }] },
		});
		expect(preload().finishGesture).toHaveBeenCalledTimes(1);
	});

	it("addresses a selected group by group id", async () => {
		desk.groupId = "front";
		desk.values = [angleValue(FIXTURE_A, 20, 5), angleValue(FIXTURE_B, 20, 5)];
		await mount();
		fireEvent.click(screen.getByRole("button", { name: "Reset pan to zero" }));
		expect(normal().applyIntent.mock.calls[0]?.[0]).toMatchObject({ groupId: "front", fixtureIds: [] });
	});

	it("stays inert without the semantic Position pages or before the values are ready", async () => {
		desk.ready = false;
		const { pan, joystick } = await mount({ enabled: false });
		expect(pan).toHaveAttribute("aria-disabled", "true");
		expect(joystick).toHaveAttribute("aria-disabled", "true");
		fireEvent.click(screen.getByRole("button", { name: "Reset pan to zero" }));
		hold(joystick);
		expect(normal().applyIntent).not.toHaveBeenCalled();
		cleanup();
		desk.ready = true;
		const legacy = await mount({ semantic: false, enabled: false });
		expect(legacy.pan).toHaveAttribute("aria-disabled", "true");
		hold(legacy.joystick);
		expect(normal().applyIntent).not.toHaveBeenCalled();
	});

	it("shows a quiet Unsupported state and sends nothing for movers without Position physical data (TL-637)", async () => {
		desk.values = [];
		const unposed = {
			lane: "normal",
			scope: {},
			revision: 1,
			lease: LEASE,
			owners: [FIXTURE_A, FIXTURE_B].map((fixture_id) => ({
				fixture_id,
				position: { available: false, commands: [], common: null },
			})),
		} as unknown as OutputReadoutSnapshot;
		const { pan, joystick } = await mount({ snapshot: unposed, enabled: false });
		await waitFor(() => expect(screen.getByTestId("pan-value-caption")).toHaveTextContent("Unsupported"));
		expect(screen.getByTestId("tilt-value-caption")).toHaveTextContent("Unsupported");
		expect(pan).toHaveAttribute("aria-disabled", "true");
		expect(screen.getByRole("slider", { name: "Tilt angle" })).toBeDisabled();
		expect(joystick).toHaveAttribute("aria-disabled", "true");
		for (const name of ["Decrease pan by 90 degrees", "Reset pan to zero", "Increase pan by 90 degrees", "Return Home"])
			expect(screen.getByRole("button", { name })).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: "Return Home" }));
		fireEvent.keyDown(pan, { key: "ArrowRight" });
		hold(joystick);
		fireEvent.pointerUp(joystick, at(1, 0));
		for (const call of Object.values(normal())) expect(call).not.toHaveBeenCalled();
		expect(screen.queryByRole("alert")).toBeNull();
	});

	it("names the lease that delivered the dialog's fixtures, not another consumer's newer one", async () => {
		const value = context(true, readout("normal", [30, 30], [10, 10]));
		const { pan } = await mount({ value });
		await waitFor(() => expect(value.readouts.displayedSource("normal")).toEqual({ lane: "normal", lease: LEASE }));
		// Another surface then reads a different fixture and receives a newer lease.
		value.readouts.observe({
			...readout("normal", [0, 0], [0, 0]),
			lease: LEASE + 1,
			owners: [{ fixture_id: "33333333-3333-4333-8333-333333333333", position: { available: false, commands: [], common: null } }],
		} as unknown as OutputReadoutSnapshot);
		fireEvent.keyDown(pan, { key: "ArrowRight" });
		expect(normal().applyIntent).toHaveBeenCalledWith(
			expect.objectContaining({ attribute: "position", displayedSource: { lane: "normal", lease: LEASE } }),
		);
	});
});

describe("Return Home (POSITION-HOME-001)", () => {
	const HOME = [
		{ kind: "scalar", component: { kind: "pan" }, operation: { kind: "set", value: { kind: "value", value: 0 } } },
		{ kind: "scalar", component: { kind: "tilt" }, operation: { kind: "set", value: { kind: "value", value: 0 } } },
	];

	it("sends the home Angles of the ordered selection as one gesture with the Programmer fade", async () => {
		desk.values = [angleValue(FIXTURE_A, 120, 40), angleValue(FIXTURE_B, -60, -20)];
		const { pan } = await mount();
		fireEvent.click(screen.getByRole("button", { name: "Return Home" }));
		// Absolute on both axes even though the selection reads Mixed (never a relative offset).
		expect(edits(normal())).toEqual([HOME]);
		expect(normal().applyIntent.mock.calls[0]?.[0]).toMatchObject({
			attribute: "position",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			groupId: null,
			timing: { fade: true, fadeMillis: 2_000 },
		});
		expect(normal().finishGesture).toHaveBeenCalledTimes(1);
		expect(normal().finishGesture.mock.calls[0]?.[0]).toMatchObject({
			attribute: "position",
			undoGroup: (normal().applyIntent.mock.calls[0]?.[0] as { undoGroup: string }).undoGroup,
			keepAdmittedEdits: true,
		});
		expect(normal().cancelGesture).not.toHaveBeenCalled();
		expect(pan).toHaveAttribute("aria-valuenow", "0");
	});

	it("takes over an active Target once, writes Preload in Preload and addresses a selected Group", async () => {
		await mount();
		fireEvent.click(screen.getByRole("button", { name: "Return Home" }));
		expect(edits(normal())).toEqual([[{ kind: "activate_angles" }, ...HOME]]);
		cleanup();
		desk.preloadMode = true;
		desk.groupId = "front";
		desk.values = [angleValue(FIXTURE_A, 20, 5), angleValue(FIXTURE_B, 20, 5)];
		await mount({ snapshot: readout("preload", [20, 20], [5, 5]) });
		fireEvent.click(screen.getByRole("button", { name: "Return Home" }));
		expect(normal().applyIntent).toHaveBeenCalledTimes(1);
		expect(preload().applyIntent.mock.calls[0]?.[0]).toMatchObject({
			groupId: "front",
			fixtureIds: [],
			operation: { edits: HOME },
		});
		expect(preload().finishGesture).toHaveBeenCalledTimes(1);
	});

	it("stops a held joystick before homing and is disabled before the values are ready", async () => {
		desk.values = [angleValue(FIXTURE_A, 20, 5), angleValue(FIXTURE_B, 20, 5)];
		const { joystick } = await mount();
		hold(joystick);
		fireEvent.click(screen.getByRole("button", { name: "Return Home" }));
		const after = normal().applyIntent.mock.calls.length;
		expect(edits(normal()).at(-1)).toEqual(HOME);
		flush(100); flush(100);
		expect(normal().applyIntent.mock.calls.length).toBe(after);
		expect(normal().finishGesture).toHaveBeenCalledTimes(2);
		cleanup();
		desk.ready = false;
		desk.normal = fakeWriter();
		await mount({ enabled: false });
		expect(screen.getByRole("button", { name: "Return Home" })).toBeDisabled();
		expect(normal().applyIntent).not.toHaveBeenCalled();
	});
});
