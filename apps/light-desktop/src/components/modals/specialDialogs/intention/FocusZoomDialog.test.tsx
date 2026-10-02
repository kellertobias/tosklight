import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FocusZoomDialog, type FocusZoomDialogProps } from "./FocusZoomDialog";

// Default diagram geometry (640 × 180) mapped 1:1 to client pixels.
const WIDTH = 640, HEIGHT = 180, LEFT = 32, RIGHT = WIDTH - 32, LENGTH = RIGHT - LEFT, CENTER = HEIGHT / 2;
const MAXIMUM_OPENING = CENTER - 28;
const rad = (degrees: number) => degrees * Math.PI / 180;
const halfOpening = (zoom: number, maximum = 48) => MAXIMUM_OPENING * Math.tan(rad(zoom / 2)) / Math.tan(rad(maximum / 2));
const zoomFromOpening = (opening: number, maximum = 48) => Math.round(2 * Math.atan(opening / MAXIMUM_OPENING * Math.tan(rad(maximum / 2))) * 180 / Math.PI * 10) / 10;

const zoomDescriptor = { value: 24, minimum: 8, maximum: 48, step: .1, keyStep: 1, largeKeyStep: 5, convention: "beam" as const };
const focusDescriptor = { value: .5, minimum: 0, maximum: 1, step: .01 };

type Resize = (width: number, height: number) => void;
let resizeObservers: Resize[] = [];

beforeEach(() => {
	resizeObservers = [];
	vi.stubGlobal("ResizeObserver", class {
		constructor(private readonly callback: ResizeObserverCallback) {}
		observe() {
			resizeObservers.push((width, height) => this.callback([{ contentRect: { width, height } } as ResizeObserverEntry], this as unknown as ResizeObserver));
		}
		disconnect() {}
		unobserve() {}
	});
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

function setup(overrides: Partial<Pick<FocusZoomDialogProps, "zoom" | "focus">> = {}) {
	const props = {
		zoom: zoomDescriptor, focus: focusDescriptor,
		onZoomChange: vi.fn(), onFocusChange: vi.fn(), onGestureStart: vi.fn(), onGestureEnd: vi.fn(), onGestureCancel: vi.fn(), onClose: vi.fn(),
		...overrides,
	} satisfies FocusZoomDialogProps;
	const view = render(<FocusZoomDialog {...props} />);
	const diagram = screen.getByRole("group", { name: "Beam angle and focus diagram" });
	vi.spyOn(diagram, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, right: WIDTH, bottom: HEIGHT, width: WIDTH, height: HEIGHT, toJSON: () => ({}) });
	return { props, view, diagram };
}
const angleSlider = () => screen.getByRole("slider", { name: /opening angle$/ });
const focusSlider = () => screen.getByRole("slider", { name: "Focus position" });
const upperHandle = () => screen.getByTestId("beam-angle-handle-upper").querySelector(".focus-zoom-angle-hit")!;
const lowerHandle = () => screen.getByTestId("beam-angle-handle-lower").querySelector(".focus-zoom-angle-hit")!;
const focusHandle = () => focusSlider().querySelector(".focus-zoom-focus-handle")!;
const edgeHits = () => [...screen.getByTestId("beam-edge-drag").querySelectorAll(".focus-zoom-edge-hit")];
const pointer = (pointerId: number, clientX: number, clientY: number) => ({ pointerId, clientX, clientY, button: 0, pointerType: "touch" });
const numberAttribute = (element: Element, name: string) => Number(element.getAttribute(name));

describe("FocusZoomDialog", () => {
	it("keeps the standard modal chrome and displays only the supplied limits, steps and convention", () => {
		setup({
			zoom: { value: 12.5, minimum: 4, maximum: 60, step: .5, convention: "field" },
			focus: { value: .255, minimum: .1, maximum: .9, step: .001 },
		});
		const dialog = screen.getByRole("dialog", { name: "Focus Special Dialog" });
		expect(within(dialog).getByText("Focus", { selector: ".ui-modal-titlebar *" })).toBeInTheDocument();
		expect(within(dialog).getByRole("button", { name: "Close Focus Special Dialog" })).toBeInTheDocument();
		expect(angleSlider()).toHaveAccessibleName("Field opening angle");
		expect(angleSlider()).toHaveAttribute("aria-valuemin", "4");
		expect(angleSlider()).toHaveAttribute("aria-valuemax", "60");
		expect(angleSlider()).toHaveAttribute("aria-valuenow", "12.5");
		expect(angleSlider()).toHaveAttribute("aria-valuetext", "12.5 degrees");
		expect(focusSlider()).toHaveAttribute("aria-valuemin", "10");
		expect(focusSlider()).toHaveAttribute("aria-valuemax", "90");
		expect(focusSlider()).toHaveAttribute("aria-valuetext", "25.5%");
		expect(within(dialog).getByText("Near · 10.0%")).toBeInTheDocument();
		expect(within(dialog).getByText("Far · 90.0%")).toBeInTheDocument();
		expect(dialog.querySelector(".focus-zoom-readouts")).toHaveTextContent("Field 12.5°Focus 25.5%");
		expect(dialog.querySelector("canvas")).toBeNull();
		expect(within(dialog).queryByRole("button", { name: "Expand" })).toBeNull();
	});

	it("emits no edit or gesture when opening, resizing, navigating focus or closing", () => {
		const { props } = setup();
		act(() => resizeObservers.forEach(resize => resize(720, 240)));
		act(() => resizeObservers.forEach(resize => resize(320, 140)));
		angleSlider().focus(); focusSlider().focus();
		fireEvent.keyDown(focusSlider(), { key: "Tab" });
		fireEvent.click(screen.getByRole("button", { name: "Close Focus Special Dialog" }));
		fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
		expect(props.onClose).toHaveBeenCalled();
		for (const callback of [props.onZoomChange, props.onFocusChange, props.onGestureStart, props.onGestureEnd, props.onGestureCancel]) expect(callback).not.toHaveBeenCalled();
	});

	it("drags a beam handle from its initial grab offset without jumping", () => {
		const { props } = setup();
		const handleY = numberAttribute(upperHandle(), "cy");
		expect(handleY).toBeCloseTo(CENTER - Math.max(44, halfOpening(24)), 5);
		// Grab 9 px below the visible handle centre.
		fireEvent.pointerDown(upperHandle(), pointer(3, RIGHT + 4, handleY + 9));
		expect(props.onGestureStart).toHaveBeenCalledWith(expect.objectContaining({ control: "zoom", source: "pointer", pointerId: 3, initialValue: 24 }));
		fireEvent.pointerMove(upperHandle(), pointer(3, RIGHT + 4, handleY + 9));
		expect(props.onZoomChange).not.toHaveBeenCalled();
		fireEvent.pointerMove(upperHandle(), pointer(3, RIGHT + 4, handleY + 9 - 10));
		expect(props.onZoomChange).toHaveBeenLastCalledWith(zoomFromOpening(halfOpening(24) + 10), expect.objectContaining({ control: "zoom" }));
		// Moves stay relative to the gesture seed, not to the last emitted value.
		fireEvent.pointerMove(upperHandle(), pointer(3, RIGHT + 4, handleY + 9 - 20));
		expect(props.onZoomChange).toHaveBeenLastCalledWith(zoomFromOpening(halfOpening(24) + 20), expect.anything());
		fireEvent.pointerUp(upperHandle(), pointer(3, RIGHT + 4, handleY - 11));
		expect(props.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(props.onGestureCancel).not.toHaveBeenCalled();
		fireEvent.pointerMove(upperHandle(), pointer(3, RIGHT + 4, handleY - 60));
		expect(props.onZoomChange).toHaveBeenCalledTimes(2);
	});

	it("drags a beam edge with the touched point's local beam fraction", () => {
		const { props } = setup();
		const x = LEFT + LENGTH * .5, y = CENTER + halfOpening(24) * .5 + 6;
		fireEvent.pointerDown(edgeHits()[1], pointer(4, x, y));
		fireEvent.pointerMove(edgeHits()[1], pointer(4, x + 30, y + 5));
		// Five pixels at half the beam length widens the far opening by ten pixels.
		expect(props.onZoomChange).toHaveBeenLastCalledWith(zoomFromOpening(halfOpening(24) + 10), expect.anything());
		fireEvent.pointerUp(edgeHits()[1], pointer(4, x + 30, y + 5));
		expect(props.onFocusChange).not.toHaveBeenCalled();
	});

	it("drags the Focus plane from its grab offset in normalized steps", () => {
		const { props } = setup();
		const planeX = LEFT + .5 * LENGTH;
		fireEvent.pointerDown(focusHandle(), pointer(5, planeX + 12, CENTER + 8));
		fireEvent.pointerMove(focusHandle(), pointer(5, planeX + 12 + LENGTH * .1, CENTER + 30));
		expect(props.onFocusChange).toHaveBeenLastCalledWith(.6, expect.objectContaining({ control: "focus", initialValue: .5 }));
		fireEvent.pointerMove(focusHandle(), pointer(5, planeX + 12 + LENGTH, CENTER));
		expect(props.onFocusChange).toHaveBeenLastCalledWith(1, expect.anything());
		fireEvent.pointerMove(focusHandle(), pointer(5, planeX + 12 + LENGTH * 2, CENTER));
		expect(props.onFocusChange).toHaveBeenCalledTimes(2);
		fireEvent.pointerUp(focusHandle(), pointer(5, 0, 0));
		expect(props.onZoomChange).not.toHaveBeenCalled();
		expect(props.onGestureEnd).toHaveBeenCalledWith(expect.objectContaining({ control: "focus" }));
	});

	it("keeps both handles and the Focus plane independently touchable for a narrow beam at either endpoint", () => {
		for (const focus of [0, 1]) {
			const { props, view } = setup({ zoom: { ...zoomDescriptor, value: 8 }, focus: { ...focusDescriptor, value: focus } });
			const upper = upperHandle(), lower = lowerHandle(), hit = focusSlider().querySelector(".focus-zoom-focus-hit")!;
			const r = numberAttribute(upper, "r");
			expect(r).toBeGreaterThanOrEqual(22);
			expect(numberAttribute(lower, "cy") - numberAttribute(upper, "cy")).toBeGreaterThanOrEqual(4 * r);
			const hitTop = numberAttribute(hit, "y"), hitBottom = hitTop + numberAttribute(hit, "height");
			expect(numberAttribute(hit, "width")).toBeGreaterThanOrEqual(44);
			expect(hitBottom - hitTop).toBeGreaterThanOrEqual(44);
			// Directly at the handle column the Focus target stays clear of both handle circles.
			const clear = Math.min(CENTER + 22, hitBottom) - Math.max(CENTER - 22, hitTop);
			expect(clear).toBeGreaterThanOrEqual(44);
			expect(numberAttribute(upper, "cy") + r).toBeLessThanOrEqual(CENTER - 22 + .001);
			expect(numberAttribute(lower, "cy") - r).toBeGreaterThanOrEqual(CENTER + 22 - .001);

			fireEvent.pointerDown(upper, pointer(1, RIGHT, numberAttribute(upper, "cy")));
			fireEvent.pointerMove(upper, pointer(1, RIGHT, numberAttribute(upper, "cy") - 15));
			fireEvent.pointerUp(upper, pointer(1, RIGHT, numberAttribute(upper, "cy") - 15));
			fireEvent.pointerDown(lower, pointer(2, RIGHT, numberAttribute(lower, "cy")));
			fireEvent.pointerMove(lower, pointer(2, RIGHT, numberAttribute(lower, "cy") + 15));
			fireEvent.pointerUp(lower, pointer(2, RIGHT, numberAttribute(lower, "cy") + 15));
			expect(props.onZoomChange).toHaveBeenCalledTimes(2);
			for (const [value] of props.onZoomChange.mock.calls) expect(value).toBeGreaterThan(8);
			const planeX = LEFT + focus * LENGTH;
			fireEvent.pointerDown(focusHandle(), pointer(3, planeX, CENTER));
			fireEvent.pointerMove(focusHandle(), pointer(3, planeX + (focus ? -LENGTH * .25 : LENGTH * .25), CENTER));
			fireEvent.pointerUp(focusHandle(), pointer(3, 0, 0));
			expect(props.onFocusChange).toHaveBeenCalledWith(focus ? .75 : .25, expect.anything());
			view.unmount();
		}
	});

	it("uses the pointer's side of the axis when narrow edge targets overlap", () => {
		const { props } = setup({ zoom: { ...zoomDescriptor, value: 8 } });
		const x = LEFT + LENGTH * .9;
		// The lower edge path is on top, but the touch is above the axis: this is the upper edge.
		fireEvent.pointerDown(edgeHits()[1], pointer(6, x, CENTER - 3));
		fireEvent.pointerMove(edgeHits()[1], pointer(6, x, CENTER - 13));
		expect(props.onZoomChange.mock.calls.at(-1)?.[0]).toBeGreaterThan(8);
		fireEvent.pointerUp(edgeHits()[1], pointer(6, x, CENTER - 13));
	});

	it.each([
		["pointer cancel", "pointer-cancel", (target: Element) => fireEvent.pointerCancel(target, pointer(7, 0, 0))],
		["lost capture", "lost-capture", (target: Element) => fireEvent.lostPointerCapture(target, pointer(7, 0, 0))],
	] as const)("stops the gesture on %s", (_name, reason, stop) => {
		const { props } = setup();
		const handleY = numberAttribute(upperHandle(), "cy");
		fireEvent.pointerDown(upperHandle(), pointer(7, RIGHT, handleY));
		fireEvent.pointerMove(upperHandle(), pointer(7, RIGHT, handleY - 8));
		stop(upperHandle());
		expect(props.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ control: "zoom", pointerId: 7 }), reason);
		expect(props.onGestureEnd).not.toHaveBeenCalled();
		const calls = props.onZoomChange.mock.calls.length;
		fireEvent.pointerMove(upperHandle(), pointer(7, RIGHT, handleY - 40));
		fireEvent.pointerUp(upperHandle(), pointer(7, RIGHT, handleY - 40));
		expect(props.onZoomChange).toHaveBeenCalledTimes(calls);
		expect(props.onGestureCancel).toHaveBeenCalledTimes(1);
	});

	it("ignores other pointers while a gesture is active", () => {
		const { props } = setup();
		const handleY = numberAttribute(upperHandle(), "cy");
		fireEvent.pointerDown(upperHandle(), pointer(1, RIGHT, handleY));
		fireEvent.pointerDown(focusHandle(), pointer(2, LEFT + LENGTH / 2, CENTER));
		fireEvent.pointerMove(focusHandle(), pointer(2, LEFT + LENGTH, CENTER));
		fireEvent.pointerUp(focusHandle(), pointer(2, LEFT + LENGTH, CENTER));
		expect(props.onGestureStart).toHaveBeenCalledTimes(1);
		expect(props.onFocusChange).not.toHaveBeenCalled();
		expect(props.onGestureEnd).not.toHaveBeenCalled();
		fireEvent.pointerUp(upperHandle(), pointer(1, RIGHT, handleY));
		expect(props.onGestureEnd).toHaveBeenCalledTimes(1);
	});

	it("routes an active gesture to the current callbacks and renders only supplied values", () => {
		const { props, view } = setup();
		const handleY = numberAttribute(upperHandle(), "cy");
		fireEvent.pointerDown(upperHandle(), pointer(8, RIGHT, handleY));
		fireEvent.pointerMove(upperHandle(), pointer(8, RIGHT, handleY - 10));
		// No local value ownership: without a new prop the readout still shows the supplied value.
		expect(angleSlider()).toHaveAttribute("aria-valuenow", "24");
		const next = { onZoomChange: vi.fn(), onGestureEnd: vi.fn(), onGestureCancel: vi.fn() };
		view.rerender(<FocusZoomDialog {...props} {...next} zoom={{ ...zoomDescriptor, value: 30 }} />);
		expect(angleSlider()).toHaveAttribute("aria-valuenow", "30");
		fireEvent.pointerMove(upperHandle(), pointer(8, RIGHT, handleY - 20));
		expect(props.onZoomChange).toHaveBeenCalledTimes(1);
		// The gesture keeps its start seed and geometry, so the new prop does not move the grab point.
		expect(next.onZoomChange).toHaveBeenCalledWith(zoomFromOpening(halfOpening(24) + 20), expect.objectContaining({ initialValue: 24 }));
		fireEvent.pointerUp(upperHandle(), pointer(8, RIGHT, handleY - 20));
		expect(next.onGestureEnd).toHaveBeenCalledTimes(1);
		expect(props.onGestureEnd).not.toHaveBeenCalled();
	});

	it("cancels an active gesture on close and on teardown with the latest callbacks", () => {
		const first = setup();
		fireEvent.pointerDown(focusHandle(), pointer(9, LEFT + LENGTH / 2, CENTER));
		fireEvent.click(screen.getByRole("button", { name: "Close Focus Special Dialog" }));
		expect(first.props.onGestureCancel).toHaveBeenCalledWith(expect.objectContaining({ control: "focus" }), "close");
		expect(first.props.onClose).toHaveBeenCalledTimes(1);
		first.view.unmount();
		expect(first.props.onGestureCancel).toHaveBeenCalledTimes(1);

		const second = setup();
		fireEvent.pointerDown(focusHandle(), pointer(10, LEFT + LENGTH / 2, CENTER));
		const latest = vi.fn();
		second.view.rerender(<FocusZoomDialog {...second.props} onGestureCancel={latest} />);
		second.view.unmount();
		expect(latest).toHaveBeenCalledWith(expect.objectContaining({ control: "focus", pointerId: 10 }), "teardown");
		expect(second.props.onGestureCancel).not.toHaveBeenCalled();
		expect(second.props.onGestureEnd).not.toHaveBeenCalled();
	});

	it("steps with the keyboard as one-shot gestures inside the supplied limits", () => {
		const { props, view } = setup();
		fireEvent.keyDown(angleSlider(), { key: "ArrowUp" });
		expect(props.onZoomChange).toHaveBeenLastCalledWith(25, expect.objectContaining({ source: "keyboard", control: "zoom" }));
		fireEvent.keyDown(angleSlider(), { key: "ArrowDown", shiftKey: true });
		expect(props.onZoomChange).toHaveBeenLastCalledWith(19, expect.anything());
		fireEvent.keyDown(angleSlider(), { key: "End" });
		expect(props.onZoomChange).toHaveBeenLastCalledWith(48, expect.anything());
		fireEvent.keyDown(focusSlider(), { key: "ArrowRight" });
		expect(props.onFocusChange).toHaveBeenLastCalledWith(.51, expect.objectContaining({ source: "keyboard", control: "focus" }));
		fireEvent.keyDown(focusSlider(), { key: "PageDown" });
		expect(props.onFocusChange).toHaveBeenLastCalledWith(.4, expect.anything());
		expect(props.onGestureStart).toHaveBeenCalledTimes(5);
		expect(props.onGestureEnd).toHaveBeenCalledTimes(5);
		view.rerender(<FocusZoomDialog {...props} zoom={{ ...zoomDescriptor, value: 48 }} focus={{ ...focusDescriptor, value: 0 }} />);
		fireEvent.keyDown(angleSlider(), { key: "ArrowUp" });
		fireEvent.keyDown(focusSlider(), { key: "Home" });
		expect(props.onGestureStart).toHaveBeenCalledTimes(5);
	});
});
