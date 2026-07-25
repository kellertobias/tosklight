import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TouchEncoder } from "./TouchEncoder";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

function renderEncoder(overrides: Partial<Parameters<typeof TouchEncoder>[0]> = {}) {
	const onStep = vi.fn();
	const onSet = vi.fn();
	render(
		<TouchEncoder
			label="Enc 1 · Pan"
			display="50%"
			value={0.5}
			onStep={onStep}
			onSet={onSet}
			{...overrides}
		/>,
	);
	return { onStep, onSet };
}

describe("TouchEncoder", () => {
	it("exposes coarse, fine, and absolute-entry zones without a fader", () => {
		const { onStep } = renderEncoder();
		fireEvent.click(screen.getByRole("button", { name: "+10" }));
		fireEvent.click(screen.getByRole("button", { name: "−1" }));
		expect(onStep).toHaveBeenNthCalledWith(1, 0.1);
		expect(onStep).toHaveBeenNthCalledWith(2, -0.01);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "Set Value" }));
		expect(
			screen.getByRole("dialog", { name: "Enc 1 · Pan value" }),
		).toBeInTheDocument();
		expect(onStep).toHaveBeenCalledTimes(2);
	});

	it("uses one undo-group identity for continuous displacement-rate samples", () => {
		vi.useFakeTimers();
		const { onStep } = renderEncoder();
		const encoder = screen.getByRole("region", { name: "Enc 1 · Pan" });
		fireEvent.pointerDown(encoder, {
			pointerId: 7,
			button: 0,
			clientY: 120,
		});
		fireEvent.pointerMove(encoder, { pointerId: 7, clientY: 40 });
		vi.advanceTimersByTime(240);
		fireEvent.pointerUp(encoder, { pointerId: 7, clientY: 40 });

		expect(onStep.mock.calls.length).toBeGreaterThanOrEqual(2);
		const groups = new Set(onStep.mock.calls.map((call) => call[1]));
		expect(groups.size).toBe(1);
		expect([...groups][0]).toEqual(expect.any(String));
		expect(onStep.mock.calls.every(([delta]) => delta > 0)).toBe(true);
	});

	it("maps wheel direction to fine steps and Shift to coarse steps", () => {
		const { onStep } = renderEncoder();
		const encoder = screen.getByRole("region", { name: "Enc 1 · Pan" });
		fireEvent.wheel(encoder, { deltaY: -10 });
		fireEvent.wheel(encoder, { deltaY: 10, shiftKey: true });
		expect(onStep).toHaveBeenNthCalledWith(1, 0.01);
		expect(onStep).toHaveBeenNthCalledWith(2, -0.1);
	});

	it("shows indexed values as constrained instead of applying a normalized step", () => {
		const { onStep } = renderEncoder({ indexed: true, display: "Gobo 3" });
		expect(screen.getByText("Indexed value")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "+1" })).toBeDisabled();
		fireEvent.wheel(screen.getByRole("region", { name: "Enc 1 · Pan" }), {
			deltaY: -10,
		});
		expect(onStep).not.toHaveBeenCalled();
	});
});
