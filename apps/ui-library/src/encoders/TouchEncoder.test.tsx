import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
} from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
	TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS,
	TOUCH_ENCODER_DRAG_DEAD_ZONE_PX,
	TouchEncoder,
} from "./TouchEncoder";

const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

function renderEncoder(
	overrides: Partial<Parameters<typeof TouchEncoder>[0]> = {},
) {
	const onStep = vi.fn();
	const onSet = vi.fn();
	render(
		<TouchEncoder
			label="Enc 1 · Pan"
			slot={1}
			attributeLabel="Pan"
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
	it("uses one continuous ridged surface with a centered absolute value", () => {
		const { onStep } = renderEncoder();
		const surface = document.querySelector(".touch-encoder-surface");
		expect(surface).not.toBeNull();
		expect(surface?.querySelector(".touch-encoder-ridges")).toBeInTheDocument();
		expect(surface?.querySelector(".touch-encoder-legend")).toHaveTextContent(
			"Increase•••Set•••Decrease",
		);
		expect(surface?.querySelector(".touch-encoder-value")).toHaveTextContent(
			"50%",
		);
		expect(
			document.querySelector(".touch-encoder-drag-feedback"),
		).not.toBeInTheDocument();
		expect(surface?.querySelector(".touch-encoder-labels")).toHaveTextContent(
			"PanEnc 1",
		);
		expect(
			document.querySelector(".touch-encoder-set"),
		).not.toBeInTheDocument();
		fireEvent.click(
			surface?.querySelector(".touch-encoder-tap-positive") as Element,
		);
		fireEvent.click(
			surface?.querySelector(".touch-encoder-tap-negative") as Element,
		);
		expect(onStep).toHaveBeenNthCalledWith(1, 0.001);
		expect(onStep).toHaveBeenNthCalledWith(2, -0.001);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
		for (const removedName of ["+10", "+1", "−1", "−10"])
			expect(
				screen.queryByRole("button", { name: removedName }),
			).not.toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Enc 1 · Pan value" }),
		).toBeInTheDocument();
		expect(onStep).toHaveBeenCalledTimes(2);
	});

	it("uses one undo group while scaling linearly with drag distance and reversing", () => {
		vi.useFakeTimers();
		const { onStep } = renderEncoder();
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		fireEvent.pointerDown(encoder, {
			pointerId: 7,
			button: 0,
			clientY: 120,
		});
		fireEvent.pointerMove(encoder, {
			pointerId: 7,
			clientY: 120 - TOUCH_ENCODER_DRAG_DEAD_ZONE_PX - 1,
		});
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
		fireEvent.pointerMove(encoder, {
			pointerId: 7,
			clientY: 72,
		});
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
		fireEvent.pointerMove(encoder, {
			pointerId: 7,
			clientY: 120 - TOUCH_ENCODER_DRAG_DEAD_ZONE_PX - 1,
		});
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
		fireEvent.pointerMove(encoder, {
			pointerId: 7,
			clientY: 120 + TOUCH_ENCODER_DRAG_DEAD_ZONE_PX + 1,
		});
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
		fireEvent.pointerUp(encoder, { pointerId: 7, clientY: 140 });

		const deltas = onStep.mock.calls.map(([delta]) => delta as number);
		expect(deltas).toHaveLength(4);
		expect(deltas[0]).toBeGreaterThanOrEqual(0.001);
		expect(deltas[1]).toBeGreaterThan(deltas[0] ?? Number.POSITIVE_INFINITY);
		expect(deltas[2]).toBeCloseTo(deltas[0] ?? 0, 8);
		expect(deltas[3]).toBeCloseTo(-(deltas[0] ?? 0), 8);
		const groups = new Set(onStep.mock.calls.map((call) => call[1]));
		expect(groups.size).toBe(1);
		expect([...groups][0]).toEqual(expect.any(String));
	});

	it("keeps ridges moving up while downward return travel slows them", () => {
		renderEncoder();
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		fireEvent.pointerDown(encoder, {
			pointerId: 4,
			button: 0,
			clientY: 120,
		});
		fireEvent.pointerMove(encoder, { pointerId: 4, clientY: 110 });
		expect(encoder).toHaveAttribute("data-motion", "up");
		const slowSpeed = Number.parseFloat(
			encoder.style.getPropertyValue("--encoder-motion-speed"),
		);
		fireEvent.pointerMove(encoder, { pointerId: 4, clientY: 70 });
		const fastSpeed = Number.parseFloat(
			encoder.style.getPropertyValue("--encoder-motion-speed"),
		);
		expect(fastSpeed).toBeGreaterThan(slowSpeed);
		fireEvent.pointerMove(encoder, { pointerId: 4, clientY: 90 });
		expect(encoder).toHaveAttribute("data-motion", "up");
		const returningSpeed = Number.parseFloat(
			encoder.style.getPropertyValue("--encoder-motion-speed"),
		);
		expect(returningSpeed).toBeLessThan(fastSpeed);
		expect(returningSpeed).toBeGreaterThan(slowSpeed);
		fireEvent.pointerUp(encoder, { pointerId: 4, clientY: 90 });
		expect(encoder).not.toHaveAttribute("data-motion");
	});

	it("can drag from every semantic zone and cancel stops repetition without a trailing action", () => {
		vi.useFakeTimers();
		const { onStep } = renderEncoder();
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		const zones = [
			document.querySelector(".touch-encoder-tap-positive"),
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
			document.querySelector(".touch-encoder-tap-negative"),
		];
		zones.forEach((zone, index) => {
			fireEvent.pointerDown(zone as Element, {
				pointerId: index + 1,
				button: 0,
				clientY: 100,
			});
			fireEvent.pointerMove(encoder, {
				pointerId: index + 1,
				clientY: 70,
			});
			vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS);
			fireEvent.pointerCancel(encoder, {
				pointerId: index + 1,
				clientY: 70,
			});
		});
		const callsAfterCancel = onStep.mock.calls.length;
		vi.advanceTimersByTime(TOUCH_ENCODER_CONTINUOUS_INTERVAL_MILLIS * 3);
		expect(callsAfterCancel).toBe(3);
		expect(onStep).toHaveBeenCalledTimes(callsAfterCancel);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("maps wheel direction to fine steps and Shift to coarse steps", () => {
		const { onStep } = renderEncoder({ slowStep: 0.0001, fastStep: 0.025 });
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		fireEvent.wheel(encoder, { deltaY: -10 });
		fireEvent.wheel(encoder, { deltaY: 10, shiftKey: true });
		expect(onStep).toHaveBeenNthCalledWith(1, 0.0001);
		expect(onStep).toHaveBeenNthCalledWith(2, -0.025);
	});

	it("supports fine keyboard steps and keyboard absolute entry", () => {
		const { onStep } = renderEncoder();
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		fireEvent.keyDown(encoder, { key: "ArrowUp" });
		fireEvent.keyDown(encoder, { key: "ArrowLeft" });
		expect(onStep).toHaveBeenNthCalledWith(1, 0.001);
		expect(onStep).toHaveBeenNthCalledWith(2, -0.001);
		fireEvent.keyDown(encoder, { key: "Enter" });
		expect(screen.getByRole("dialog")).toBeInTheDocument();
		expect(encoder).toHaveAccessibleDescription(
			/The upper third increases the value and the lower third decreases it/u,
		);
	});

	it("keeps internal values separate from rendered and entered values", () => {
		const { onSet } = renderEncoder({
			value: 520,
			display: undefined,
			formatValue: (value) => `${value / 10}%`,
			minimum: 0,
			maximum: 1000,
			inputScale: 0.1,
		});
		expect(screen.getByText("52%")).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.getByRole("textbox", { name: "Enc 1 · Pan value" }),
		).toHaveTextContent("52");
		fireEvent.click(screen.getByRole("button", { name: "7" }));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(onSet).toHaveBeenCalledWith(750);
	});

	it("stacks range endpoints around a centered ellipsis", () => {
		renderEncoder({ display: "0% ... 100%" });
		const value = screen.getByRole("button", {
			name: "Set Enc 1 · Pan value",
		});
		expect(value).toHaveClass("range-value");
		expect(value.querySelectorAll("span")).toHaveLength(2);
		expect(value.querySelectorAll("span")[0]).toHaveTextContent("0%");
		expect(value.querySelector("i")).toHaveTextContent("...");
		expect(value.querySelectorAll("span")[1]).toHaveTextContent("100%");
	});

	it("uses the shared preset mode for absolute encoder values", () => {
		const { onSet } = renderEncoder({
			presets: {
				groups: [
					{
						label: "Position",
						options: [
							{ value: "25", label: "Quarter" },
							{ value: "75", label: "Three quarters" },
						],
					},
				],
			},
		});
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Show presets" }));
		fireEvent.click(screen.getByRole("button", { name: /Three quarters/u }));

		expect(onSet).toHaveBeenCalledWith(0.75);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("offers a background-free chevron surface and direct preset chooser for discrete values", () => {
		const { onSet, onStep } = renderEncoder({
			display: "Loop",
			value: 0,
			inputScale: 1,
			slowStep: 1,
			fastStep: 1,
			touchInteraction: "choices",
			presets: {
				selectedValue: "0",
				groups: [
					{
						label: "Run mode",
						options: [
							{ value: "0", label: "Loop" },
							{ value: "1", label: "One-shot" },
						],
					},
				],
			},
		});
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		expect(encoder).toHaveClass("choice-encoder");
		expect(
			encoder.querySelector(".touch-encoder-ridges"),
		).not.toBeInTheDocument();
		expect(
			encoder.querySelector(".touch-encoder-legend"),
		).not.toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", { name: "Next Enc 1 · Pan value" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Previous Enc 1 · Pan value" }),
		);
		expect(onStep).toHaveBeenNthCalledWith(1, 1);
		expect(onStep).toHaveBeenNthCalledWith(2, -1);

		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.getByRole("button", { name: "One-shot" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Show value input" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "One-shot" }));
		expect(onSet).toHaveBeenCalledWith(1);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("shows indexed values as constrained instead of applying a normalized step", () => {
		const { onStep } = renderEncoder({ indexed: true, display: "Gobo 3" });
		expect(screen.getByText("Gobo 3")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		).toBeDisabled();
		const encoder = screen.getByRole("group", { name: "Enc 1 · Pan" });
		expect(encoder).toHaveAttribute("aria-disabled", "true");
		fireEvent.wheel(encoder, {
			deltaY: -10,
		});
		fireEvent.click(
			document.querySelector(".touch-encoder-tap-positive") as Element,
		);
		expect(onStep).not.toHaveBeenCalled();
	});

	it("keeps Release off the encoder face and offers it only in absolute entry", () => {
		const onRelease = vi.fn();
		const { onSet } = renderEncoder({ canRelease: true, onRelease });

		expect(
			screen.queryByRole("button", { name: "Release Pan" }),
		).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Release" }));

		expect(onRelease).toHaveBeenCalledOnce();
		expect(onSet).not.toHaveBeenCalled();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("does not offer modal Release without both ownership and a callback", () => {
		const rendered = renderEncoder({ canRelease: true });
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.queryByRole("button", { name: "Release" }),
		).not.toBeInTheDocument();

		cleanup();
		renderEncoder({ canRelease: false, onRelease: vi.fn() });
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.queryByRole("button", { name: "Release" }),
		).not.toBeInTheDocument();
		expect(rendered.onSet).not.toHaveBeenCalled();
	});
});
