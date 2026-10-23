// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Ruler, rulerLabelFrames } from "./TimecodeRuler";

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("Timecode ruler label spacing", () => {
	for (const [name, duration, scale, width] of [
		["Fit with a fractional-second end", 44 * 61 + 13, 640 / (44 * 61 + 13), 74],
		["zoomed", 44 * 30 + 7, 4, 74],
		["large operator text", 44 * 60, 2, 140],
	] as const) it(`keeps labels separate and the exact endpoint at ${name}`, () => {
		const frames = rulerLabelFrames(duration, 44, scale, width);
		expect(frames.at(-1)).toBe(duration);
		expect(new Set(frames).size).toBe(frames.length);
		for (let index = 1; index < frames.length; index += 1) {
			const previousRight = frames[index - 1] * scale + width;
			const nextLeft = frames[index] * scale - (index === frames.length - 1 ? width : 0);
			expect(nextLeft - previousRight).toBeGreaterThanOrEqual(8);
		}
	});

	it("keeps the stripe grid and exact non-multiple endpoint positions", () => {
		const { container } = render(<Ruler duration={277} fps={44} pixelsPerFrame={2} />);
		expect([...container.querySelectorAll<HTMLElement>(".timecode-ruler-stripes i")].map((node) => node.style.left)).toEqual(["0px", "88px", "176px", "264px", "352px", "440px", "528px"]);
		const endpoint = container.querySelector<HTMLElement>(".timecode-ruler-final-tick")!;
		expect(endpoint.dataset.frame).toBe("277");
		expect(endpoint.style.left).toBe("554px");
		expect(endpoint.textContent).toBe("00:00:06.13");
	});

	it("remeasures labels when the operator font size changes without moving ticks", () => {
		let width = 64;
		let resized = () => {};
		vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
			return { width: this.classList.contains("timecode-ruler-label-probe") ? width : 0 } as DOMRect;
		});
		vi.stubGlobal("ResizeObserver", class { constructor(callback: () => void) { resized = callback; } observe() {} disconnect() {} });
		const { container } = render(<Ruler duration={880} fps={44} pixelsPerFrame={1} />);
		const grid = container.querySelector(".timecode-ruler-stripes")!.innerHTML;
		const count = container.querySelectorAll(".timecode-ruler > span").length;
		width = 250;
		// ResizeObserver is driven explicitly so the measurement remains deterministic.
		act(() => resized());
		expect(container.querySelectorAll(".timecode-ruler > span").length).toBeLessThan(count);
		expect(container.querySelector(".timecode-ruler-stripes")!.innerHTML).toBe(grid);
	});
});
