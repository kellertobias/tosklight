import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ColorPlanePicker, type ColorPlanePickerProps } from "./ColorPlanePicker";
import { HorizontalRangeFader, type HorizontalRangeFaderProps, type RangeGesture, type ValueRange } from "./HorizontalRangeFader";
import { HueRingPicker, type HueRingPickerProps } from "./HueRingPicker";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

const rect = (width: number, height: number) => ({ x: 0, y: 0, left: 0, top: 0, right: width, bottom: height, width, height, toJSON: () => ({}) }) as DOMRect;
const sized = <T extends Element>(element: T, width: number, height: number) => { vi.spyOn(element as Element, "getBoundingClientRect").mockReturnValue(rect(width, height)); return element; };
const at = (pointerId: number, clientX: number, clientY: number, extra: Record<string, unknown> = {}) => ({ pointerId, clientX, clientY, button: 0, pointerType: "touch", ...extra });
const tap = (element: Element, x: number, y: number, extra: Record<string, unknown> = {}, pointerId = 1) => {
	fireEvent.pointerDown(element, at(pointerId, x, y, extra));
	fireEvent.pointerUp(element, at(pointerId, x, y, extra));
};
const callbacks = () => {
	let next = 100;
	return {
		onGestureStart: vi.fn((_gesture: RangeGesture) => `host-${next++}`),
		onGestureEnd: vi.fn(),
		onGestureCancel: vi.fn(),
	};
};

type FaderHost = Partial<HorizontalRangeFaderProps> & { initialValue?: number; initialRange?: ValueRange };
/** A controlled host: it owns the value and range and re-renders the fader with whatever it stored. */
function Fader({ initialValue = 50, initialRange, onChange, ...props }: FaderHost) {
	const [state, setState] = useState<{ value: number; range?: ValueRange }>({ value: initialValue, range: initialRange });
	return <>
		<HorizontalRangeFader label="White Blend" value={state.value} range={state.range} {...props}
			onChange={(value, range, gesture) => { onChange?.(value, range, gesture); setState({ value, range }); }} />
		<button type="button" onClick={() => setState({ value: 70 })}>External 70</button>
		<button type="button" onClick={() => setState(current => ({ ...current, value: current.value + 1e-7 }))}>Echo noise</button>
		<button type="button" onClick={() => setState({ value: 10, range: [10, 90] })}>External range</button>
	</>;
}
const faderInput = (name = "White Blend") => sized(screen.getByRole("slider", { name }), 200, 72);

describe("HorizontalRangeFader", () => {
	it("maps touches through the supplied limits and steps and reports one finite pointer gesture", () => {
		const onChange = vi.fn(), gestures = callbacks();
		render(<Fader min={1000} max={20000} step={100} format={v => `${Math.round(v)} K`} label="Temperature" onChange={onChange} {...gestures} />);
		const input = faderInput("Temperature");
		fireEvent.pointerDown(input, at(7, 50, 30));
		expect(onChange).toHaveBeenLastCalledWith(5800, undefined, expect.objectContaining({ id: "host-100", source: "pointer", pointerId: 7, control: "Temperature", shifted: false }));
		fireEvent.pointerMove(input, at(7, 51, 30));
		expect(onChange).toHaveBeenCalledTimes(1);
		fireEvent.pointerMove(input, at(7, 100, 30));
		fireEvent.pointerMove(input, at(7, 100.2, 31));
		expect(onChange).toHaveBeenCalledTimes(2);
		expect(onChange).toHaveBeenLastCalledWith(10500, undefined, expect.objectContaining({ id: "host-100" }));
		fireEvent.pointerMove(input, at(8, 180, 30));
		expect(onChange).toHaveBeenCalledTimes(2);
		fireEvent.pointerUp(input, at(7, 100, 30));
		expect(gestures.onGestureStart).toHaveBeenCalledTimes(1);
		expect(gestures.onGestureEnd).toHaveBeenCalledWith(expect.objectContaining({ id: "host-100" }));
		expect(gestures.onGestureCancel).not.toHaveBeenCalled();
		fireEvent.lostPointerCapture(input, at(7, 100, 30));
		fireEvent.pointerMove(input, at(7, 150, 30));
		expect(onChange).toHaveBeenCalledTimes(2);
		expect(gestures.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(input).toHaveAttribute("aria-valuetext", "10500 K");
		expect(screen.getByLabelText("Temperature value")).toHaveTextContent("10500 K");
	});

	it("keeps keyboard Shift endpoints in operator order, including descending ranges, and collapses on an ordinary edit", () => {
		const onChange = vi.fn();
		render(<Fader onChange={onChange} />);
		const input = faderInput();
		tap(input, 160, 30);
		tap(input, 40, 30, { shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(80, [80, 20], expect.objectContaining({ shifted: true }));
		expect(input).toHaveAttribute("aria-valuetext", "80% through 20%");
		expect(input).toHaveAttribute("aria-valuenow", "20");
		expect(screen.getByLabelText("White Blend value")).toHaveTextContent("80% → 20%");
		expect([...document.querySelectorAll(".horizontal-range-handle")].map(handle => handle.textContent)).toEqual(["1", "2"]);
		tap(input, 100, 30);
		expect(onChange).toHaveBeenLastCalledWith(50, undefined, expect.objectContaining({ shifted: false }));
		expect(input).toHaveAttribute("aria-valuetext", "50%");
	});

	it("uses software SHIFT and hardware shiftArmed for a pending first and a completing last endpoint", () => {
		for (const mode of ["software", "hardware"]) {
			const onChange = vi.fn();
			const view = render(<Fader shiftArmed onChange={onChange} />);
			const input = faderInput();
			tap(input, 40, 30);
			expect(onChange, mode).toHaveBeenLastCalledWith(20, undefined, expect.objectContaining({ shifted: true }));
			expect(screen.getByText("Shift-click the last value")).toBeInTheDocument();
			expect(document.querySelector(".horizontal-range-handle")).toHaveTextContent("1");
			tap(input, 160, 30);
			expect(onChange, mode).toHaveBeenLastCalledWith(20, [20, 80], expect.objectContaining({ shifted: true }));
			expect(screen.queryByText("Shift-click the last value")).toBeNull();
			// A drag under SHIFT re-spreads from the same first endpoint.
			fireEvent.pointerDown(input, at(2, 100, 30));
			fireEvent.pointerMove(input, at(2, 190, 30));
			expect(onChange, mode).toHaveBeenLastCalledWith(20, [20, 95], expect.anything());
			fireEvent.pointerUp(input, at(2, 190, 30));
			view.unmount();
		}
	});

	it("never ranges when allowRange is false and supports Arrow, Home, End and Shift+Arrow keys", () => {
		const onChange = vi.fn(), gestures = callbacks();
		const view = render(<Fader allowRange={false} shiftArmed onChange={onChange} />);
		tap(faderInput(), 40, 30, { shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(20, undefined, expect.objectContaining({ shifted: false }));
		view.unmount();
		onChange.mockClear();
		render(<Fader onChange={onChange} {...gestures} />);
		const input = faderInput();
		fireEvent.keyDown(input, { key: "ArrowRight" });
		expect(onChange).toHaveBeenLastCalledWith(51, undefined, expect.objectContaining({ source: "keyboard", id: "host-100" }));
		expect(gestures.onGestureEnd).toHaveBeenLastCalledWith(expect.objectContaining({ id: "host-100", source: "keyboard" }));
		fireEvent.keyDown(input, { key: "End" });
		expect(input).toHaveAttribute("aria-valuenow", "100");
		const calls = onChange.mock.calls.length;
		fireEvent.keyDown(input, { key: "End" });
		expect(onChange, "a key that changes nothing authors nothing").toHaveBeenCalledTimes(calls);
		fireEvent.keyDown(input, { key: "Home" });
		fireEvent.keyDown(input, { key: "ArrowRight", shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(0, [0, 1], expect.objectContaining({ shifted: true }));
		fireEvent.keyDown(input, { key: "ArrowRight", shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(0, [0, 2], expect.anything());
		fireEvent.keyDown(input, { key: "a" });
		expect(gestures.onGestureStart.mock.calls.length).toBe(gestures.onGestureEnd.mock.calls.length);
	});

	it("discards a stale anchor after an external update but keeps it through its own echo", () => {
		const onChange = vi.fn();
		render(<Fader onChange={onChange} />);
		const input = faderInput();
		tap(input, 60, 30);
		fireEvent.click(screen.getByText("Echo noise"));
		tap(input, 140, 30, { shiftKey: true });
		expect(onChange, "a conversion echo keeps the anchor").toHaveBeenLastCalledWith(30, [30, 70], expect.anything());
		tap(input, 60, 30);
		fireEvent.click(screen.getByText("External 70"));
		tap(input, 160, 30, { shiftKey: true });
		expect(onChange, "no stale 30 endpoint after replacement").toHaveBeenLastCalledWith(80, undefined, expect.objectContaining({ shifted: true }));
		expect(screen.getByText("Shift-click the last value")).toBeInTheDocument();
		fireEvent.click(screen.getByText("External range"));
		expect(screen.queryByText("Shift-click the last value")).toBeNull();
		tap(input, 40, 30, { shiftKey: true });
		expect(onChange, "a supplied range's first endpoint is the anchor").toHaveBeenLastCalledWith(10, [10, 20], expect.anything());
	});

	it("cancels an active gesture on external replacement and emits no stale movement afterwards", () => {
		const onChange = vi.fn(), gestures = callbacks();
		render(<Fader onChange={onChange} {...gestures} />);
		const input = faderInput();
		fireEvent.pointerDown(input, at(3, 40, 30, { shiftKey: true }));
		fireEvent.pointerMove(input, at(3, 120, 30));
		expect(onChange).toHaveBeenLastCalledWith(20, [20, 60], expect.anything());
		fireEvent.click(screen.getByText("External 70"));
		expect(gestures.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ id: "host-100" }), "external");
		const calls = onChange.mock.calls.length;
		fireEvent.pointerMove(input, at(3, 180, 30));
		fireEvent.pointerUp(input, at(3, 180, 30));
		expect(onChange).toHaveBeenCalledTimes(calls);
		expect(gestures.onGestureEnd).not.toHaveBeenCalled();
		expect(input).toHaveAttribute("aria-valuetext", "70%");
	});

	it.each([
		["pointer-cancel", (input: Element) => fireEvent.pointerCancel(input, at(4, 0, 0))],
		["lost-capture", (input: Element) => fireEvent.lostPointerCapture(input, at(4, 0, 0))],
		["blur", (input: Element) => fireEvent.blur(input)],
		["hidden", () => {
			vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
			fireEvent(document, new Event("visibilitychange"));
		}],
	] as const)("ends a gesture once on %s and ignores later movement", (reason, stop) => {
		const onChange = vi.fn(), gestures = callbacks();
		render(<Fader onChange={onChange} {...gestures} />);
		const input = faderInput();
		fireEvent.pointerDown(input, at(4, 40, 30));
		stop(input);
		fireEvent.pointerMove(input, at(4, 180, 30));
		fireEvent.pointerUp(input, at(4, 180, 30));
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(gestures.onGestureCancel).toHaveBeenCalledTimes(1);
		expect(gestures.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ id: "host-100" }), reason);
		expect(gestures.onGestureEnd).not.toHaveBeenCalled();
	});

	it("cancels on close by unmount with the latest host callbacks and authors nothing on remount or resize", () => {
		const first = callbacks(), second = callbacks(), onChange = vi.fn();
		const view = render(<HorizontalRangeFader label="Duv" value={0} min={-.03} max={.03} step={.0005} onChange={onChange} {...first} />);
		fireEvent.pointerDown(faderInput("Duv"), at(5, 150, 30));
		expect(onChange).toHaveBeenLastCalledWith(.015, undefined, expect.anything());
		view.rerender(<HorizontalRangeFader label="Duv" value={.015} min={-.03} max={.03} step={.0005} onChange={onChange} {...second} />);
		view.unmount();
		expect(first.onGestureCancel).not.toHaveBeenCalled();
		expect(second.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ id: "host-100" }), "teardown");
		onChange.mockClear();
		const again = render(<HorizontalRangeFader label="Duv" value={-.015} range={[-.015, .015]} min={-.03} max={.03} step={.0005} format={v => v.toFixed(3)} onChange={onChange} />);
		sized(screen.getByRole("slider", { name: "Duv" }), 500, 90);
		again.rerender(<HorizontalRangeFader label="Duv" value={-.015} range={[-.015, .015]} min={-.03} max={.03} step={.0005} format={v => v.toFixed(3)} onChange={onChange} />);
		fireEvent(window, new Event("resize"));
		expect(onChange).not.toHaveBeenCalled();
		expect(screen.getByRole("slider", { name: "Duv" })).toHaveAttribute("aria-valuetext", "-0.015 through 0.015");
	});

	it("suppresses native touch handling and duplicate native scalar edits", () => {
		const onChange = vi.fn(), gestures = callbacks();
		render(<Fader onChange={onChange} {...gestures} />);
		const input = faderInput();
		const touch = new Event("touchstart", { bubbles: true, cancelable: true });
		input.dispatchEvent(touch);
		expect(touch.defaultPrevented).toBe(true);
		const move = new Event("touchmove", { bubbles: true, cancelable: true });
		input.dispatchEvent(move);
		expect(move.defaultPrevented).toBe(true);
		const pointerDown = fireEvent.pointerDown(input, at(6, 40, 30));
		expect(pointerDown, "pointerdown is default-prevented").toBe(false);
		fireEvent.change(input, { target: { value: "90" } });
		expect(onChange).toHaveBeenCalledTimes(1);
		fireEvent.pointerUp(input, at(6, 40, 30));
		fireEvent.change(input, { target: { value: "20" } });
		expect(onChange).toHaveBeenCalledTimes(1);
		fireEvent.change(input, { target: { value: "35" } });
		expect(onChange).toHaveBeenLastCalledWith(35, undefined, expect.objectContaining({ source: "native" }));
		expect(gestures.onGestureEnd).toHaveBeenLastCalledWith(expect.objectContaining({ source: "native" }));
	});

	it("keeps limits finite and scopes its own class names while accepting extra hooks", () => {
		render(<HorizontalRangeFader label="Odd" value={5} min={Number.NaN} max={10} step={-1} onChange={vi.fn()} classNames={{ field: "x-field", fader: "x-fader", handle: "x-handle" }} />);
		const input = screen.getByRole("slider", { name: "Odd" });
		expect(input).toHaveAttribute("min", "0");
		expect(input).toHaveAttribute("max", "10");
		expect(input).toHaveAttribute("step", "0.1");
		expect(input.closest(".horizontal-range-field")).toHaveClass("x-field");
		expect(input.closest(".vertical-touch-fader")).toHaveClass("horizontal-range-fader", "x-fader");
		expect(document.querySelector(".horizontal-range-handle")).toHaveClass("x-handle");
	});
});

function Ring(props: Partial<HueRingPickerProps> & { initialRange?: ValueRange }) {
	const [hue, setHue] = useState<{ value: number; range?: ValueRange }>({ value: 120, range: props.initialRange });
	const [saturation, setSaturation] = useState<{ value: number; range?: ValueRange }>({ value: 75, range: [75, 25] });
	return <HueRingPicker preview="#0f0" hue={hue.value} range={hue.range} saturation={saturation.value} saturationRange={saturation.range} {...props}
		onHue={(value, range, gesture) => { props.onHue?.(value, range, gesture); setHue({ value, range }); }}
		onSaturation={(value, range, gesture) => { props.onSaturation?.(value, range, gesture); setSaturation({ value, range }); }} />;
}
const ringAt = (ring: Element, degrees: number, extra: Record<string, unknown> = {}) => {
	const radians = degrees * Math.PI / 180;
	tap(ring, 100 + Math.sin(radians) * 84, 100 - Math.cos(radians) * 84, extra);
};

describe("HueRingPicker", () => {
	it("maps the accurate ring clockwise from the top and edits only hue", () => {
		const onHue = vi.fn(), onSaturation = vi.fn();
		render(<Ring onHue={onHue} onSaturation={onSaturation} />);
		const ring = sized(screen.getByRole("slider", { name: "Hue" }), 200, 200);
		for (const degrees of [0, 90, 180, 270]) {
			ringAt(ring, degrees);
			expect(onHue).toHaveBeenLastCalledWith(degrees, undefined, expect.objectContaining({ control: "hue" }));
		}
		ringAt(ring, 300);
		ringAt(ring, 60, { shiftKey: true });
		expect(onHue).toHaveBeenLastCalledWith(300, [300, 60], expect.objectContaining({ shifted: true }));
		expect(ring).toHaveAttribute("aria-valuetext", "300 through 60 degrees");
		expect(screen.getByLabelText("Hue value")).toHaveTextContent("300° → 60°");
		expect(onSaturation).not.toHaveBeenCalled();
		expect(screen.getByRole("slider", { name: "Saturation" })).toHaveAttribute("aria-valuetext", "75% through 25%");
	});

	it("collapses only the edited component and keeps keyboard and hardware SHIFT paths", () => {
		const onHue = vi.fn(), onSaturation = vi.fn();
		const view = render(<Ring initialRange={[300, 60]} onHue={onHue} onSaturation={onSaturation} />);
		tap(faderInput("Saturation"), 100, 30);
		expect(onSaturation).toHaveBeenLastCalledWith(50, undefined, expect.objectContaining({ control: "saturation" }));
		expect(onHue).not.toHaveBeenCalled();
		const ring = screen.getByRole("slider", { name: "Hue" });
		expect(ring).toHaveAttribute("aria-valuetext", "300 through 60 degrees");
		fireEvent.keyDown(ring, { key: "ArrowRight", shiftKey: true });
		expect(onHue).toHaveBeenLastCalledWith(300, [300, 61], expect.objectContaining({ source: "keyboard" }));
		fireEvent.keyDown(ring, { key: "ArrowLeft" });
		expect(onHue).toHaveBeenLastCalledWith(299, undefined, expect.anything());
		view.unmount();
		render(<Ring shiftArmed onHue={onHue} />);
		const armed = sized(screen.getByRole("slider", { name: "Hue" }), 200, 200);
		ringAt(armed, 240);
		expect(screen.getByText("Shift-click the last hue")).toBeInTheDocument();
		expect(document.querySelector(".hue-ring-handle")).toHaveTextContent("1");
		ringAt(armed, 0);
		expect(onHue).toHaveBeenLastCalledWith(240, [240, 0], expect.anything());
		expect([...document.querySelectorAll(".hue-ring-handle")].map(handle => handle.textContent)).toEqual(["1", "2"]);
	});
});

function Plane(props: Partial<ColorPlanePickerProps>) {
	const [state, setState] = useState<{ hue: number; saturation: number; hueRange?: ValueRange; saturationRange?: ValueRange }>({ hue: 120, saturation: 60 });
	return <>
		<ColorPlanePicker preview="#0f0" {...state} {...props}
			onChange={(hue, saturation, hueRange, saturationRange, gesture) => { props.onChange?.(hue, saturation, hueRange, saturationRange, gesture); setState({ hue, saturation, hueRange, saturationRange }); }} />
		<button type="button" onClick={() => setState({ hue: 200, saturation: 10 })}>Encoder edit</button>
	</>;
}
const planeAt = (plane: Element, hue: number, saturation: number, extra: Record<string, unknown> = {}) => tap(plane, hue, 100 - saturation, extra);

describe("ColorPlanePicker", () => {
	it("maps hue to X and saturation to Y and completes ordered ranges for both with hardware shiftArmed", () => {
		const onChange = vi.fn(), gestures = callbacks();
		render(<Plane shiftArmed onChange={onChange} {...gestures} />);
		const plane = sized(screen.getByTestId("color-picker"), 359, 100);
		planeAt(plane, 300, 80);
		expect(onChange).toHaveBeenLastCalledWith(300, 80, undefined, undefined, expect.objectContaining({ control: "plane", shifted: true, id: "host-100" }));
		expect(screen.getByText("Shift-click the last color")).toBeInTheDocument();
		expect([...plane.querySelectorAll(".color-plane-marker")].map(marker => marker.textContent)).toEqual(["1"]);
		planeAt(plane, 60, 40);
		expect(onChange).toHaveBeenLastCalledWith(300, 80, [300, 60], [80, 40], expect.objectContaining({ id: "host-101" }));
		expect([...plane.querySelectorAll(".color-plane-marker")].map(marker => marker.textContent)).toEqual(["1", "2"]);
		expect(screen.queryByText("Shift-click the last color")).toBeNull();
		expect(gestures.onGestureEnd).toHaveBeenCalledTimes(2);
	});

	it("collapses on an ordinary contact, steps with arrows and drops a pending endpoint on an external edit", () => {
		const onChange = vi.fn();
		render(<Plane onChange={onChange} />);
		const plane = sized(screen.getByTestId("color-picker"), 359, 100);
		planeAt(plane, 10, 90);
		planeAt(plane, 350, 20, { shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(10, 90, [10, 350], [90, 20], expect.anything());
		planeAt(plane, 120, 60);
		expect(onChange).toHaveBeenLastCalledWith(120, 60, undefined, undefined, expect.anything());
		expect([...plane.querySelectorAll(".color-plane-marker")].map(marker => marker.textContent)).toEqual([""]);
		fireEvent.keyDown(plane, { key: "ArrowUp" });
		expect(onChange).toHaveBeenLastCalledWith(120, 61, undefined, undefined, expect.objectContaining({ source: "keyboard" }));
		fireEvent.keyDown(plane, { key: "ArrowRight", shiftKey: true });
		expect(onChange).toHaveBeenLastCalledWith(120, 61, [120, 121], [61, 61], expect.anything());
		const calls = onChange.mock.calls.length;
		fireEvent.keyDown(plane, { key: "Home" });
		expect(onChange).toHaveBeenCalledTimes(calls);
		planeAt(plane, 30, 30);
		planeAt(plane, 200, 50, { shiftKey: true });
		planeAt(plane, 100, 100);
		act(() => { fireEvent.click(screen.getByText("Encoder edit")); });
		planeAt(plane, 300, 30, { shiftKey: true });
		expect(onChange, "the anchor from before the encoder edit is gone").toHaveBeenLastCalledWith(300, 30, undefined, undefined, expect.objectContaining({ shifted: true }));
		expect(plane).toHaveAttribute("data-hue", "300");
	});

	it("drags without authoring under the 3 px threshold and ignores a second contact", () => {
		const onChange = vi.fn();
		render(<Plane onChange={onChange} />);
		const plane = sized(screen.getByTestId("color-picker"), 359, 100);
		fireEvent.pointerDown(plane, at(1, 100, 50));
		fireEvent.pointerMove(plane, at(1, 101, 51));
		fireEvent.pointerDown(plane, at(2, 300, 10));
		fireEvent.pointerMove(plane, at(2, 300, 10));
		expect(onChange).toHaveBeenCalledTimes(1);
		fireEvent.pointerMove(plane, at(1, 200, 20));
		expect(onChange).toHaveBeenLastCalledWith(200, 80, undefined, undefined, expect.anything());
		fireEvent.pointerUp(plane, at(1, 200, 20));
	});
});
