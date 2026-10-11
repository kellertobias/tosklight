import {
	cleanup,
	render,
	waitFor,
	act,
	fireEvent,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SoloFenceOverlay } from "./SoloFenceOverlay";
const regions = [
	{
		id: "pair",
		name: "Pair",
		members: [
			{ page: 1, position: 1 },
			{ page: 1, position: 3 },
		],
	},
];
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});
function harness() {
	let offset = 20;
	let peerWidth = 100;
	let resize: ResizeObserverCallback = () => {};
	const disconnect = vi.fn();
	const observe = vi.fn();
	vi.stubGlobal(
		"ResizeObserver",
		class {
			constructor(callback: ResizeObserverCallback) {
				resize = callback;
			}
			observe = observe;
			disconnect = disconnect;
		},
	);
	vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
		function (this: HTMLElement) {
			const peer = this.dataset.playbackSlot === "3";
			const width = this.dataset.grid ? 310 : peer ? peerWidth : 205;
			const height = this.dataset.grid ? 310 : peer ? 200 : 100;
			const top = offset + (peer ? 105 : 0);
			return {
				left: offset,
				top,
				right: offset + width,
				bottom: top + height,
				width,
				height,
				x: offset,
				y: top,
				toJSON: () => ({}),
			};
		},
	);
	function View({ version = 0 }: { version?: number }) {
		const [bank, setBank] = useState<HTMLDivElement | null>(null);
		return (
			<div data-testid="viewport" style={{ overflow: "auto" }}>
				<div ref={setBank} data-grid="true" style={{ columnGap: 5, rowGap: 5 }}>
					<article data-playback-row="0" data-playback-slot="1" />
					<article data-playback-row="1" data-playback-slot="3" />
					<SoloFenceOverlay
						bank={bank}
						page={1}
						regions={regions}
						layoutKey={version}
					/>
				</div>
			</div>
		);
	}
	const view = render(<View />);
	return {
		...view,
		disconnect,
		observe,
		move: () => {
			offset = -80;
		},
		widen: () => {
			peerWidth = 205;
			act(() => resize([], {} as ResizeObserver));
		},
		relayout: () => view.rerender(<View version={1} />),
	};
}
const coordinates = (container: HTMLElement) =>
	[...container.querySelectorAll("line")].map((line) =>
		["x1", "y1", "x2", "y2"].map((name) => Number(line.getAttribute(name))),
	);
describe("measured Solo perimeter overlay", () => {
	it("draws only the exposed partial edge using actual unequal card heights, and never receives input", async () => {
		const view = harness();
		await waitFor(() =>
			expect(view.container.querySelectorAll("line").length).toBeGreaterThan(0),
		);
		expect(coordinates(view.container)).toContainEqual([
			102.5, 102.5, 207.5, 102.5,
		]);
		expect(coordinates(view.container)).not.toContainEqual([
			1.5, 102.5, 207.5, 102.5,
		]);
		const svg = view.container.querySelector("svg")!;
		expect(svg.style.pointerEvents).toBe("none");
		expect(svg).toHaveAttribute("aria-hidden", "true");
		expect(view.observe).toHaveBeenCalledTimes(3);
		view.widen();
		expect(
			coordinates(view.container).filter(
				(line) => line[1] === 102.5 && line[3] === 102.5,
			),
		).toEqual([]);
	});
	it("keeps content-relative fences through scrolling and recomputes/cleans observers on layout changes", () => {
		const view = harness();
		const before = coordinates(view.container);
		view.move();
		fireEvent.scroll(view.getByTestId("viewport"));
		fireEvent(window, new Event("resize"));
		expect(coordinates(view.container)).toEqual(before);
		view.relayout();
		expect(view.disconnect).toHaveBeenCalledOnce();
		expect(view.observe).toHaveBeenCalledTimes(6);
		view.unmount();
		expect(view.disconnect).toHaveBeenCalledTimes(2);
	});
});
