import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PositionProgrammingEditor } from "../../../../features/fixtureAbstractionMockup/PositionProgrammingEditor";
import { PositionDialog, type PositionDialogProps } from "./PositionDialog";

const panDescriptor = { value: 24, minimum: -720, maximum: 720, step: .1, keyStep: 1, largeKeyStep: 360 };
const tiltDescriptor = { value: 42, minimum: -135, maximum: 135, step: .1 };
const rates = { panDegreesPerSecond: 120, tiltDegreesPerSecond: 90 };

// Pan circle 220 × 220 at the origin; Tilt input 270 px wide; joystick 440 × 440 (usable radius 194).
const PAN_SIZE = 220, TILT_WIDTH = 270, JOY = 440, JOY_REACH = JOY / 2 - 26;
const rect = (width: number, height: number) => ({ x: 0, y: 0, left: 0, top: 0, right: width, bottom: height, width, height, toJSON: () => ({}) });
const panPoint = (degrees: number, radius = 80) => ({ clientX: PAN_SIZE / 2 + radius * Math.sin(degrees * Math.PI / 180), clientY: PAN_SIZE / 2 - radius * Math.cos(degrees * Math.PI / 180) });
const joyPoint = (x: number, y: number) => ({ clientX: JOY / 2 + x * JOY_REACH, clientY: JOY / 2 + y * JOY_REACH });
const tiltPoint = (value: number) => ({ clientX: (value + 135) / 270 * TILT_WIDTH, clientY: 30 });
const pointer = (pointerId: number, at: { clientX: number; clientY: number }) => ({ pointerId, ...at, button: 0, pointerType: "touch" });

let frames: Map<number, FrameRequestCallback>;
let nextFrame: number;
let clock: number;
let captured: Set<number>;

beforeEach(() => {
	frames = new Map(); nextFrame = 1; clock = 1000; captured = new Set();
	vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { const id = nextFrame++; frames.set(id, callback); return id; });
	vi.stubGlobal("cancelAnimationFrame", (id: number) => { frames.delete(id); });
	vi.spyOn(performance, "now").mockImplementation(() => clock);
	Element.prototype.setPointerCapture = vi.fn(function (id: number) { captured.add(id); });
	Element.prototype.hasPointerCapture = vi.fn((id: number) => captured.has(id));
	Element.prototype.releasePointerCapture = vi.fn((id: number) => { captured.delete(id); });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

/** Advance the clock and run every pending animation frame once. */
function flush(ms = 16) {
	clock += ms;
	const pending = [...frames.values()];
	frames.clear();
	act(() => pending.forEach(callback => callback(clock)));
}
const pendingFrames = () => frames.size;

function setup(overrides: Partial<Pick<PositionDialogProps, "pan" | "tilt" | "joystick">> = {}) {
	const props = {
		pan: panDescriptor, tilt: tiltDescriptor, joystick: rates as PositionDialogProps["joystick"],
		onChange: vi.fn<PositionDialogProps["onChange"]>(), onGestureStart: vi.fn<NonNullable<PositionDialogProps["onGestureStart"]>>(),
		onGestureEnd: vi.fn<NonNullable<PositionDialogProps["onGestureEnd"]>>(), onGestureCancel: vi.fn<NonNullable<PositionDialogProps["onGestureCancel"]>>(),
		onClose: vi.fn<PositionDialogProps["onClose"]>(),
		...overrides,
	} satisfies PositionDialogProps;
	const view = render(<PositionDialog {...props} />);
	flush(0); // ModalStack hands off focus on the first frame.
	const pan = screen.getByTestId("pan-circle");
	const tilt = screen.getByRole("slider", { name: "Tilt angle" });
	const joystick = screen.getByTestId("position-joystick");
	vi.spyOn(pan, "getBoundingClientRect").mockReturnValue(rect(PAN_SIZE, PAN_SIZE));
	vi.spyOn(tilt, "getBoundingClientRect").mockReturnValue(rect(TILT_WIDTH, 60));
	vi.spyOn(joystick, "getBoundingClientRect").mockReturnValue(rect(JOY, JOY));
	return { props, view, pan, tilt, joystick };
}
const allCallbacks = (props: PositionDialogProps) => [props.onChange, props.onGestureStart, props.onGestureEnd, props.onGestureCancel];

describe("PositionDialog", () => {
	it("keeps standard modal chrome, Pan above Tilt beside the joystick, and no aim reference or XYZ", () => {
		const { pan, tilt, joystick } = setup();
		const dialog = screen.getByRole("dialog", { name: "Position Special Dialog" });
		expect(within(dialog).getByText("Position", { selector: ".ui-modal-titlebar *" })).toBeInTheDocument();
		expect(within(dialog).getByRole("button", { name: "Close Position Special Dialog" })).toBeInTheDocument();
		expect(pan).toHaveAttribute("aria-valuemin", "-720");
		expect(pan).toHaveAttribute("aria-valuemax", "720");
		expect(pan).toHaveAttribute("aria-valuenow", "24");
		expect(pan).toHaveAttribute("aria-valuetext", "24.0 degrees, 0.07 turns");
		expect(tilt).toHaveAttribute("aria-valuemin", "-135");
		expect(tilt).toHaveAttribute("aria-valuemax", "135");
		expect(tilt).toHaveAttribute("aria-valuetext", "42.0°");
		expect(tilt.closest(".vertical-touch-fader")).not.toBeNull();
		expect(dialog.querySelectorAll("input")).toHaveLength(1);
		expect(within(dialog).getByText("−135°")).toBeInTheDocument();
		expect(within(dialog).getByText("+135°")).toBeInTheDocument();
		expect(within(dialog).getByRole("status", { name: "Pan turns" })).toHaveTextContent("+0.07 turns");
		for (const name of ["Decrease pan by 90 degrees", "Reset pan to zero", "Increase pan by 90 degrees"]) expect(within(dialog).getByRole("button", { name })).toBeEnabled();
		// Pan column (circle, actions, Tilt) precedes the joystick column.
		const angleControls = pan.parentElement!.parentElement!;
		expect(angleControls.contains(tilt)).toBe(true);
		expect(angleControls.nextElementSibling).toBe(joystick.parentElement);
		expect(within(dialog).queryByRole("button", { name: "Aim reference" })).toBeNull();
		expect(within(dialog).queryByText(/^(?:X|Y|Z|Point|Origin)$/)).toBeNull();
		expect(dialog.querySelector("canvas")).toBeNull();
	});

	it("is inert while opening, navigating, re-rendering and closing", () => {
		const { props, view, pan, tilt, joystick } = setup();
		pan.focus(); tilt.focus(); joystick.focus();
		fireEvent.keyDown(joystick, { key: "Tab" });
		view.rerender(<PositionDialog {...props} pan={{ ...panDescriptor, value: 100 }} tilt={{ ...tiltDescriptor, value: -20 }} />);
		expect(pan).toHaveAttribute("aria-valuenow", "100");
		expect(tilt).toHaveAttribute("aria-valuenow", "-20");
		fireEvent.click(screen.getByRole("button", { name: "Close Position Special Dialog" }));
		fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
		expect(props.onClose).toHaveBeenCalled();
		for (const callback of allCallbacks(props)) expect(callback).not.toHaveBeenCalled();
		expect(pendingFrames()).toBe(0);
	});

	it("turns Pan from the grab point through multiple turns, unwrapped, emitting only Pan", () => {
		const { props, view, pan } = setup({ pan: { ...panDescriptor, value: 0 } });
		let value = 0;
		props.onChange.mockImplementation(change => { value = change.pan ?? value; view.rerender(<PositionDialog {...props} pan={{ ...panDescriptor, value }} />); });
		// Grab at 90° while the handle is at 0°: no jump on press.
		fireEvent.pointerDown(pan, pointer(5, panPoint(90)));
		expect(props.onGestureStart).toHaveBeenCalledWith(expect.objectContaining({ control: "pan", source: "pointer", pointerId: 5, initialPan: 0, initialTilt: 42 }));
		expect(props.onChange).not.toHaveBeenCalled();
		for (let angle = 105; angle <= 90 + 450; angle += 15) fireEvent.pointerMove(pan, pointer(5, panPoint(angle)));
		fireEvent.pointerUp(pan, pointer(5, panPoint(90 + 450)));
		expect(value).toBe(450);
		expect(pan).toHaveAttribute("aria-valuenow", "450");
		expect(screen.getByRole("status", { name: "Pan turns" })).toHaveTextContent("+1.25 turns");
		for (const [change] of props.onChange.mock.calls) expect(Object.keys(change)).toEqual(["pan"]);
		expect(props.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(props.onGestureEnd).toHaveBeenCalledWith(expect.objectContaining({ control: "pan" }), { changed: true });
		expect(captured.size).toBe(0);
		const calls = props.onChange.mock.calls.length;
		fireEvent.pointerMove(pan, pointer(5, panPoint(0)));
		expect(props.onChange).toHaveBeenCalledTimes(calls);
	});

	it("clamps Pan at its limits without wind-up and ignores the hub", () => {
		const { props, pan } = setup({ pan: { ...panDescriptor, value: 700 } });
		fireEvent.pointerDown(pan, pointer(1, { clientX: 112, clientY: 108 }));
		expect(props.onGestureStart).not.toHaveBeenCalled();
		fireEvent.pointerDown(pan, pointer(2, panPoint(0)));
		fireEvent.pointerMove(pan, pointer(2, panPoint(45)));
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 720 }, expect.anything());
		fireEvent.pointerMove(pan, pointer(2, panPoint(30)));
		// Reversing leaves the limit at once: the 25° beyond it were never accumulated.
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 705 }, expect.anything());
		fireEvent.pointerUp(pan, pointer(2, panPoint(30)));
	});

	it("steps Pan with keys and −90°/Reset/+90° as single gestures, and only for a real change", () => {
		const { props, view, pan } = setup();
		const rerender = (value: number) => view.rerender(<PositionDialog {...props} pan={{ ...panDescriptor, value }} />);
		fireEvent.keyDown(pan, { key: "ArrowRight" });
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 25 }, expect.objectContaining({ control: "pan", source: "keyboard" }));
		fireEvent.keyDown(pan, { key: "PageUp" });
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 384 }, expect.anything());
		fireEvent.keyDown(pan, { key: "PageDown" });
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: -336 }, expect.anything());
		fireEvent.keyDown(pan, { key: "Home" });
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 0 }, expect.anything());
		fireEvent.click(screen.getByRole("button", { name: "Increase pan by 90 degrees" }));
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 114 }, expect.objectContaining({ source: "button" }));
		fireEvent.click(screen.getByRole("button", { name: "Decrease pan by 90 degrees" }));
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: -66 }, expect.anything());
		expect(props.onGestureStart).toHaveBeenCalledTimes(6);
		expect(props.onGestureEnd).toHaveBeenCalledTimes(6);
		rerender(0);
		props.onChange.mockClear();
		fireEvent.click(screen.getByRole("button", { name: "Reset pan to zero" }));
		fireEvent.keyDown(pan, { key: "Home" });
		expect(props.onChange).not.toHaveBeenCalled();
		rerender(720);
		expect(screen.getByRole("button", { name: "Increase pan by 90 degrees" })).toBeDisabled();
		fireEvent.keyDown(pan, { key: "PageUp" });
		expect(props.onChange).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Reset pan to zero" }));
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 0 }, expect.anything());
		expect(props.onChange.mock.calls.flatMap(([change]) => Object.keys(change))).not.toContain("tilt");
	});

	it("edits Tilt through the shared touch fader by pointer and keyboard, emitting only Tilt", () => {
		const { props, tilt } = setup();
		fireEvent.pointerDown(tilt, pointer(7, tiltPoint(0)));
		expect(props.onGestureStart).toHaveBeenCalledWith(expect.objectContaining({ control: "tilt", source: "pointer", pointerId: 7 }));
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 0 }, expect.anything());
		fireEvent.pointerMove(tilt, pointer(7, { clientX: tiltPoint(0).clientX + 2, clientY: 30 }));
		expect(props.onChange).toHaveBeenCalledTimes(1);
		fireEvent.pointerMove(tilt, pointer(7, tiltPoint(90)));
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 90 }, expect.anything());
		fireEvent.pointerUp(tilt, pointer(7, tiltPoint(90)));
		expect(props.onGestureEnd).toHaveBeenCalledWith(expect.objectContaining({ control: "tilt" }), { changed: true });
		fireEvent.pointerMove(tilt, pointer(7, tiltPoint(-90)));
		expect(props.onChange).toHaveBeenCalledTimes(2);
		fireEvent.keyDown(tilt, { key: "ArrowRight" });
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 42.1 }, expect.objectContaining({ source: "keyboard" }));
		fireEvent.keyDown(tilt, { key: "ArrowDown" });
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 41.9 }, expect.anything());
		fireEvent.keyDown(tilt, { key: "PageUp" });
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 43 }, expect.anything());
		fireEvent.keyDown(tilt, { key: "Home" });
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: -135 }, expect.anything());
		fireEvent.keyDown(tilt, { key: "End" });
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 135 }, expect.anything());
		expect(props.onChange.mock.calls.flatMap(([change]) => Object.keys(change))).not.toContain("pan");
		expect(props.onGestureEnd).toHaveBeenCalledTimes(6);
	});

	it("moves at a gentle-center rate while held and stops frames and capture on release", () => {
		const { props, joystick } = setup();
		// Inside the dead zone: held, but no movement and no frame loop.
		fireEvent.pointerDown(joystick, pointer(9, joyPoint(.05, 0)));
		expect(props.onGestureStart).toHaveBeenCalledWith(expect.objectContaining({ control: "joystick", source: "pointer" }));
		expect(joystick).toHaveAttribute("data-active", "true");
		expect(pendingFrames()).toBe(0);
		// Half deflection: ((.5 − .08) / .92)² × 120 °/s.
		fireEvent.pointerMove(joystick, pointer(9, joyPoint(.5, 0)));
		expect(pendingFrames()).toBe(1);
		flush(20);
		const halfRate = ((.5 - .08) / .92) ** 2 * 120;
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: Number((24 + halfRate * .02).toFixed(1)) }, expect.anything());
		// Full deflection up-right: both axes at full rate; up raises Tilt.
		fireEvent.pointerMove(joystick, pointer(9, joyPoint(1, -1)));
		flush(20);
		const [change] = props.onChange.mock.lastCall!;
		expect(change.tilt).toBeCloseTo(42 + ((Math.SQRT1_2 - .08) / .92) ** 2 * 90 * .02, 1);
		// A stalled frame integrates at most 50 ms.
		const before = props.onChange.mock.calls.length, panBefore = props.onChange.mock.lastCall![0].pan!;
		flush(5000);
		expect(props.onChange.mock.calls.length).toBe(before + 1);
		const stalled = props.onChange.mock.lastCall![0];
		expect(stalled.pan! - panBefore).toBeCloseTo(((Math.SQRT1_2 - .08) / .92) ** 2 * 120 * .05, 1);
		const capturedBefore = captured.size;
		expect(capturedBefore).toBe(1);
		fireEvent.pointerUp(joystick, pointer(9, joyPoint(1, -1)));
		expect(props.onGestureEnd).toHaveBeenCalledWith(expect.objectContaining({ control: "joystick" }), { changed: true });
		expect(pendingFrames()).toBe(0);
		expect(captured.size).toBe(0);
		expect(joystick).toHaveAttribute("data-active", "false");
		const calls = props.onChange.mock.calls.length;
		flush(16); flush(16);
		fireEvent.pointerMove(joystick, pointer(9, joyPoint(1, 0)));
		flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(calls);
	});

	it("stops frames when returned to center but keeps the held gesture open", () => {
		const { props, joystick } = setup();
		fireEvent.pointerDown(joystick, pointer(3, joyPoint(1, 0)));
		flush(16);
		fireEvent.pointerMove(joystick, pointer(3, joyPoint(0, 0)));
		expect(pendingFrames()).toBe(0);
		const calls = props.onChange.mock.calls.length;
		flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(calls);
		expect(props.onGestureEnd).not.toHaveBeenCalled();
		fireEvent.pointerMove(joystick, pointer(3, joyPoint(-1, 0)));
		expect(pendingFrames()).toBe(1);
		fireEvent.pointerUp(joystick, pointer(3, joyPoint(-1, 0)));
		expect(pendingFrames()).toBe(0);
	});

	it("schedules no further frame when a change callback ends the gesture", () => {
		const { props, joystick } = setup();
		props.onChange.mockImplementation(() => { fireEvent.pointerUp(joystick, pointer(6, joyPoint(1, 0))); });
		fireEvent.pointerDown(joystick, pointer(6, joyPoint(1, 0)));
		flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(1);
		expect(props.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(pendingFrames()).toBe(0);
		flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(1);
	});

	it("does not idle-loop at a hard stop", () => {
		const { props, joystick } = setup({ pan: { ...panDescriptor, value: 720 } });
		fireEvent.pointerDown(joystick, pointer(3, joyPoint(1, 0)));
		flush(16);
		flush(16);
		expect(props.onChange).not.toHaveBeenCalled();
		expect(pendingFrames()).toBe(0);
		fireEvent.pointerUp(joystick, pointer(3, joyPoint(1, 0)));
		expect(props.onGestureEnd).toHaveBeenCalledWith(expect.anything(), { changed: false });
	});

	const terminalPaths: [string, (context: ReturnType<typeof setup>) => void][] = [
		["pointer-cancel", ({ joystick }) => fireEvent.pointerCancel(joystick, pointer(4, joyPoint(1, 0)))],
		["lost-capture", ({ joystick }) => fireEvent.lostPointerCapture(joystick, pointer(4, joyPoint(1, 0)))],
		["blur", ({ joystick }) => fireEvent.blur(joystick)],
		["blur", () => fireEvent.blur(window)],
		["hidden", () => { vi.spyOn(document, "hidden", "get").mockReturnValue(true); fireEvent(document, new Event("visibilitychange")); }],
		["close", () => fireEvent.click(screen.getByRole("button", { name: "Close Position Special Dialog" }))],
		["teardown", ({ view }) => view.unmount()],
	];
	for (const [reason, stop] of terminalPaths) it(`stops joystick frames and capture on ${reason} and emits no later movement`, () => {
		const context = setup();
		const { props, joystick } = context;
		fireEvent.pointerDown(joystick, pointer(4, joyPoint(1, 0)));
		flush(16);
		expect(props.onChange).toHaveBeenCalled();
		expect(pendingFrames()).toBe(1);
		const calls = props.onChange.mock.calls.length;
		stop(context);
		expect(props.onGestureCancel).toHaveBeenCalledTimes(1);
		expect(props.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ control: "joystick", pointerId: 4 }), reason, { changed: true });
		expect(props.onGestureEnd).not.toHaveBeenCalled();
		expect(pendingFrames()).toBe(0);
		expect(captured.size).toBe(0);
		flush(16); flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(calls);
		if (reason === "close") expect(props.onClose).toHaveBeenCalledTimes(1);
	});

	it("drives the joystick by keyboard with the same rate path until the last key is released", () => {
		const { props, joystick } = setup();
		fireEvent.keyDown(joystick, { key: "ArrowRight" });
		expect(props.onGestureStart).toHaveBeenCalledWith(expect.objectContaining({ control: "joystick", source: "keyboard" }));
		flush(20);
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 26.4 }, expect.anything());
		fireEvent.keyDown(joystick, { key: "ArrowUp" });
		fireEvent.keyDown(joystick, { key: "ArrowRight" }); // auto-repeat
		flush(20);
		const [change] = props.onChange.mock.lastCall!;
		expect(change.pan).toBeGreaterThan(26.4);
		expect(change.tilt).toBeGreaterThan(42);
		fireEvent.keyUp(joystick, { key: "ArrowRight" });
		expect(props.onGestureEnd).not.toHaveBeenCalled();
		fireEvent.keyUp(joystick, { key: "ArrowUp" });
		expect(props.onGestureStart).toHaveBeenCalledTimes(1);
		expect(props.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(pendingFrames()).toBe(0);
		const calls = props.onChange.mock.calls.length;
		flush(16);
		expect(props.onChange).toHaveBeenCalledTimes(calls);
	});

	it("keeps callbacks current and adopts external value updates during a held gesture", () => {
		const { props, view, joystick } = setup();
		fireEvent.pointerDown(joystick, pointer(2, joyPoint(1, 0)));
		flush(20);
		expect(props.onChange).toHaveBeenLastCalledWith({ pan: 26.4 }, expect.anything());
		const replacement = vi.fn(), replacementEnd = vi.fn();
		view.rerender(<PositionDialog {...props} pan={{ ...panDescriptor, value: 100 }} onChange={replacement} onGestureEnd={replacementEnd} />);
		flush(20);
		expect(replacement).toHaveBeenLastCalledWith({ pan: 102.4 }, expect.anything());
		expect(props.onChange).toHaveBeenCalledTimes(1);
		fireEvent.pointerUp(joystick, pointer(2, joyPoint(1, 0)));
		expect(replacementEnd).toHaveBeenCalledTimes(1);
		expect(props.onGestureEnd).not.toHaveBeenCalled();
	});

	it("supersedes the joystick when Tilt is touched", () => {
		const { props, joystick, tilt } = setup();
		fireEvent.pointerDown(joystick, pointer(1, joyPoint(1, 0)));
		flush(16);
		fireEvent.pointerDown(tilt, pointer(2, tiltPoint(10)));
		expect(props.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ control: "joystick" }), "superseded", { changed: true });
		expect(pendingFrames()).toBe(0);
		expect(props.onChange).toHaveBeenLastCalledWith({ tilt: 10 }, expect.objectContaining({ control: "tilt" }));
		fireEvent.pointerUp(tilt, pointer(2, tiltPoint(10)));
	});

	it("keeps controls inert for unknown limits or a missing joystick rate", () => {
		const { props, pan, tilt, joystick } = setup({ pan: { ...panDescriptor, minimum: Number.NaN }, joystick: undefined });
		expect(pan).toHaveAttribute("aria-disabled", "true");
		expect(joystick).toHaveAttribute("aria-disabled", "true");
		fireEvent.pointerDown(pan, pointer(1, panPoint(0)));
		fireEvent.pointerMove(pan, pointer(1, panPoint(40)));
		fireEvent.keyDown(pan, { key: "ArrowRight" });
		fireEvent.click(screen.getByRole("button", { name: "Reset pan to zero" }));
		fireEvent.pointerDown(joystick, pointer(2, joyPoint(1, 0)));
		fireEvent.keyDown(joystick, { key: "ArrowRight" });
		flush(16);
		expect(pendingFrames()).toBe(0);
		expect(props.onChange).not.toHaveBeenCalled();
		expect(props.onGestureStart).not.toHaveBeenCalled();
		// Tilt keeps its own finite limits.
		fireEvent.keyDown(tilt, { key: "ArrowUp" });
		expect(props.onChange).toHaveBeenCalledWith({ tilt: 42.1 }, expect.anything());
	});
});

describe("PositionProgrammingEditor mockup wrapper", () => {
	function Harness({ onPaged, show = true }: { onPaged(): void; show?: boolean }) {
		const [angles, setAngles] = useState({ pan: 24, tilt: 42 });
		return <>
			<output aria-label="angles">{`${angles.pan}/${angles.tilt}`}</output>
			{show && <PositionProgrammingEditor fits pan={angles.pan} tilt={angles.tilt} onAngles={(pan, tilt) => setAngles({ pan, tilt })} onGestureEnd={onPaged} onClose={() => {}} />}
		</>;
	}

	it("merges per-axis changes and pages back only after real authoring, not on unmount", () => {
		const onPaged = vi.fn();
		const view = render(<Harness onPaged={onPaged} />);
		flush(0);
		fireEvent.keyDown(screen.getByRole("slider", { name: "Tilt angle" }), { key: "ArrowUp" });
		expect(screen.getByRole("status", { name: "angles", hidden: true })).toHaveTextContent("24/42.1");
		expect(onPaged).toHaveBeenCalledTimes(1);
		const joystick = screen.getByTestId("position-joystick");
		vi.spyOn(joystick, "getBoundingClientRect").mockReturnValue(rect(JOY, JOY));
		fireEvent.pointerDown(joystick, pointer(1, joyPoint(1, 0)));
		flush(20);
		expect(screen.getByRole("status", { name: "angles", hidden: true })).toHaveTextContent("26.4/42.1");
		view.rerender(<Harness onPaged={onPaged} show={false} />);
		expect(onPaged).toHaveBeenCalledTimes(1);
		flush(20);
		expect(screen.getByRole("status", { name: "angles", hidden: true })).toHaveTextContent("26.4/42.1");
	});
});
