export const SHIFT_ALL_HOLD_DURATION_MS = 650;

export interface ShiftAllPressGestureCallbacks {
	onCycleGridMethod: () => void;
	onOpenGridSettings: () => void;
}

export interface ShiftAllPressGesture {
	/**
	 * Starts a Shift+ALL gesture. Repeated key, pointer, or hardware press events
	 * are ignored until the active gesture is released or cancelled.
	 */
	press(): void;
	/**
	 * Completes the active gesture. A short press cycles the grid method; a press
	 * that reached the hold threshold opens grid settings.
	 */
	release(): void;
	/**
	 * Abandons the active gesture without invoking either action. Use for
	 * pointer cancellation, keyboard blur, or loss of hardware ownership.
	 */
	cancel(): void;
	/**
	 * Cancels the active gesture and permanently disables this controller.
	 * Intended for component unmount cleanup.
	 */
	dispose(): void;
	isPressed(): boolean;
}

/**
 * Owns only the shifted ALL gesture. It deliberately has no ordinary ALL
 * callback, so wiring it cannot alter the existing unshifted ALL behavior.
 */
export function createShiftAllPressGesture(
	callbacks: ShiftAllPressGestureCallbacks,
	holdDurationMs = SHIFT_ALL_HOLD_DURATION_MS,
): ShiftAllPressGesture {
	if (!Number.isFinite(holdDurationMs) || holdDurationMs < 0) {
		throw new RangeError(
			"holdDurationMs must be a finite, non-negative number",
		);
	}

	let pressedAt: number | null = null;
	let holdTimer: ReturnType<typeof setTimeout> | null = null;
	let settingsOpened = false;
	let disposed = false;

	const clearHoldTimer = () => {
		if (holdTimer !== null) {
			clearTimeout(holdTimer);
			holdTimer = null;
		}
	};

	const reset = () => {
		clearHoldTimer();
		pressedAt = null;
		settingsOpened = false;
	};

	const openSettings = () => {
		if (pressedAt === null || settingsOpened || disposed) {
			return;
		}
		settingsOpened = true;
		clearHoldTimer();
		callbacks.onOpenGridSettings();
	};

	return {
		press() {
			if (disposed || pressedAt !== null) {
				return;
			}
			pressedAt = Date.now();
			settingsOpened = false;
			holdTimer = setTimeout(openSettings, holdDurationMs);
		},
		release() {
			if (disposed || pressedAt === null) {
				return;
			}

			const reachedHoldThreshold = Date.now() - pressedAt >= holdDurationMs;
			if (reachedHoldThreshold) {
				openSettings();
			}
			const shouldCycle = !settingsOpened;
			reset();
			if (shouldCycle) {
				callbacks.onCycleGridMethod();
			}
		},
		cancel() {
			if (disposed) {
				return;
			}
			reset();
		},
		dispose() {
			reset();
			disposed = true;
		},
		isPressed() {
			return !disposed && pressedAt !== null;
		},
	};
}
