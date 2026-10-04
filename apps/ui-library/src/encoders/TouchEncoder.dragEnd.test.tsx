import { act, cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS, TouchEncoder } from "./TouchEncoder";

/**
 * A software encoder drag ends once, and its rate motion stops, on pointer release, pointer
 * cancel, lost pointer capture, window blur and the document becoming hidden. A drag that
 * stepped reports the end through `onDragEnd`, so the owner can finish its gesture at once.
 */

const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.restoreAllMocks();
});

function startDrag() {
	vi.useFakeTimers();
	const onStep = vi.fn();
	const onDragEnd = vi.fn();
	render(
		<TouchEncoder
			label="Enc 1 · Pan"
			display="50%"
			value={0.5}
			onStep={onStep}
			onSet={vi.fn()}
			onDragEnd={onDragEnd}
		/>,
	);
	const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
	fireEvent.pointerDown(encoder, { pointerId: 7, button: 0, clientY: 200 });
	fireEvent.pointerMove(encoder, { pointerId: 7, clientY: 150 });
	act(() => {
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS * 2);
	});
	expect(onStep.mock.calls.length).toBeGreaterThan(1);
	expect(encoder).toHaveAttribute("data-motion", "up");
	return { encoder, onStep, onDragEnd };
}

function expectStopped(
	encoder: HTMLElement,
	onStep: ReturnType<typeof vi.fn>,
	onDragEnd: ReturnType<typeof vi.fn>,
) {
	const steps = onStep.mock.calls.length;
	// Later moves of the same pointer are ignored and no repetition keeps running.
	fireEvent.pointerMove(encoder, { pointerId: 7, clientY: 50 });
	act(() => {
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS * 5);
	});
	expect(onStep).toHaveBeenCalledTimes(steps);
	expect(onDragEnd).toHaveBeenCalledOnce();
	expect(encoder).not.toHaveAttribute("data-motion");
}

describe("TouchEncoder drag end", () => {
	it("reports a released drag once", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		fireEvent.pointerUp(encoder, { pointerId: 7, clientY: 150 });
		fireEvent.lostPointerCapture(encoder, { pointerId: 7 });
		expectStopped(encoder, onStep, onDragEnd);
	});

	it("stops the motion on pointer cancel", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		fireEvent.pointerCancel(encoder, { pointerId: 7 });
		expectStopped(encoder, onStep, onDragEnd);
	});

	it("stops the motion on lost pointer capture", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		fireEvent.lostPointerCapture(encoder, { pointerId: 7 });
		expectStopped(encoder, onStep, onDragEnd);
	});

	it("stops the motion on window blur, once", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		act(() => {
			window.dispatchEvent(new Event("blur"));
			window.dispatchEvent(new Event("blur"));
		});
		expectStopped(encoder, onStep, onDragEnd);
		// The release after returning sends no second end.
		fireEvent.pointerUp(encoder, { pointerId: 7 });
		expect(onDragEnd).toHaveBeenCalledOnce();
	});

	it("stops the motion when the document becomes hidden", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		vi.spyOn(document, "hidden", "get").mockReturnValue(true);
		act(() => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		expectStopped(encoder, onStep, onDragEnd);
	});

	it("reports nothing for a press that never stepped, and a new drag works after an end", () => {
		const { encoder, onStep, onDragEnd } = startDrag();
		act(() => {
			window.dispatchEvent(new Event("blur"));
		});
		fireEvent.pointerDown(encoder, { pointerId: 8, button: 0, clientY: 200 });
		fireEvent.pointerUp(encoder, { pointerId: 8, clientY: 200 });
		expect(onDragEnd).toHaveBeenCalledOnce();
		const steps = onStep.mock.calls.length;
		fireEvent.pointerDown(encoder, { pointerId: 9, button: 0, clientY: 200 });
		fireEvent.pointerMove(encoder, { pointerId: 9, clientY: 150 });
		expect(onStep.mock.calls.length).toBeGreaterThan(steps);
		fireEvent.pointerUp(encoder, { pointerId: 9, clientY: 150 });
		expect(onDragEnd).toHaveBeenCalledTimes(2);
	});
});
