import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	createShiftAllPressGesture,
	SHIFT_ALL_HOLD_DURATION_MS,
	type ShiftAllPressGestureCallbacks,
} from "./ShiftAllPressGesture";

function setup(holdDurationMs = SHIFT_ALL_HOLD_DURATION_MS) {
	const callbacks: ShiftAllPressGestureCallbacks = {
		onCycleGridMethod: vi.fn(),
		onOpenGridSettings: vi.fn(),
	};
	return {
		callbacks,
		gesture: createShiftAllPressGesture(callbacks, holdDurationMs),
	};
}

describe("ShiftAllPressGesture", () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("cycles exactly once when released before 650 ms", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS - 1);
		gesture.release();
		vi.runAllTimers();

		expect(callbacks.onCycleGridMethod).toHaveBeenCalledOnce();
		expect(callbacks.onOpenGridSettings).not.toHaveBeenCalled();
		expect(gesture.isPressed()).toBe(false);
	});

	it("opens settings exactly once upon reaching 650 ms", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS);
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS);

		expect(callbacks.onOpenGridSettings).toHaveBeenCalledOnce();
		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
		expect(gesture.isPressed()).toBe(true);
	});

	it("does not cycle when released after a completed hold", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS);
		gesture.release();
		gesture.release();

		expect(callbacks.onOpenGridSettings).toHaveBeenCalledOnce();
		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
		expect(gesture.isPressed()).toBe(false);
	});

	it("treats the hold boundary as reached even when timer delivery is late", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.setSystemTime(new Date(Date.now() + SHIFT_ALL_HOLD_DURATION_MS));
		gesture.release();
		vi.runAllTimers();

		expect(callbacks.onOpenGridSettings).toHaveBeenCalledOnce();
		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
	});

	it.each([
		"pointer cancel",
		"blur",
	])("invokes neither action when interrupted by %s", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS - 1);
		gesture.cancel();
		vi.runAllTimers();
		gesture.release();

		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
		expect(callbacks.onOpenGridSettings).not.toHaveBeenCalled();
		expect(gesture.isPressed()).toBe(false);
	});

	it("invokes neither action when disposed during an active press", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		gesture.dispose();
		vi.runAllTimers();
		gesture.release();
		gesture.press();

		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
		expect(callbacks.onOpenGridSettings).not.toHaveBeenCalled();
		expect(gesture.isPressed()).toBe(false);
	});

	it("ignores repeated press events from keys, pointers, or hardware", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		vi.advanceTimersByTime(400);
		gesture.press();
		vi.advanceTimersByTime(249);
		gesture.release();

		expect(callbacks.onCycleGridMethod).toHaveBeenCalledOnce();
		expect(callbacks.onOpenGridSettings).not.toHaveBeenCalled();
	});

	it("can own consecutive gestures after release and cancellation", () => {
		const { callbacks, gesture } = setup();

		gesture.press();
		gesture.release();
		gesture.press();
		gesture.cancel();
		gesture.press();
		vi.advanceTimersByTime(SHIFT_ALL_HOLD_DURATION_MS);
		gesture.release();

		expect(callbacks.onCycleGridMethod).toHaveBeenCalledOnce();
		expect(callbacks.onOpenGridSettings).toHaveBeenCalledOnce();
	});

	it("ignores release and cancellation when no gesture is active", () => {
		const { callbacks, gesture } = setup();

		gesture.release();
		gesture.cancel();
		vi.runAllTimers();

		expect(callbacks.onCycleGridMethod).not.toHaveBeenCalled();
		expect(callbacks.onOpenGridSettings).not.toHaveBeenCalled();
	});

	it("exposes only shifted grid actions, never an ordinary ALL action", () => {
		const { gesture } = setup();

		expect(Object.keys(gesture).sort()).toEqual([
			"cancel",
			"dispose",
			"isPressed",
			"press",
			"release",
		]);
	});

	it.each([
		-1,
		Number.NaN,
		Number.POSITIVE_INFINITY,
	])("rejects invalid hold duration %s", (holdDurationMs) => {
		expect(() => setup(holdDurationMs)).toThrow(RangeError);
	});
});
