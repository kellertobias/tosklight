import {
	cleanup,
	render,
	waitFor,
	act,
	fireEvent,
} from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackSoloRegion } from "../../../features/playbackSolo/contracts";
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
function harness(savedRegions: readonly PlaybackSoloRegion[] = regions) {
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
	function View({ version = 0, currentRegions = savedRegions }: { version?: number; currentRegions?: readonly PlaybackSoloRegion[] }) {
		const [bank, setBank] = useState<HTMLDivElement | null>(null);
		return (
			<div data-testid="viewport" style={{ overflow: "auto" }}>
				<div ref={setBank} data-grid="true" style={{ columnGap: 5, rowGap: 5 }}>
					<article data-playback-row="0" data-playback-slot="1" />
					<article data-playback-row="1" data-playback-slot="3" />
					<SoloFenceOverlay
						bank={bank}
						page={1}
						regions={currentRegions}
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
		recolor: (next: readonly PlaybackSoloRegion[]) => view.rerender(<View currentRegions={next} />),
	};
}
const coordinates = (container: HTMLElement) =>
	[...container.querySelectorAll("path")].map(
		(path) => path.getAttribute("d")!,
	);
describe("measured Solo perimeter overlay", () => {
	it("draws only the exposed partial edge using actual unequal card heights, and never receives input", async () => {
		const view = harness();
		await waitFor(() =>
			expect(view.container.querySelectorAll("path").length).toBeGreaterThan(0),
		);
		expect(coordinates(view.container)).toHaveLength(1);
		expect(coordinates(view.container)[0]).toContain("Q 102.5 102.5");
		expect(coordinates(view.container)[0]).toContain("Q 207.5 102.5");
		const svg = view.container.querySelector("svg")!;
		expect(svg.style.pointerEvents).toBe("none");
		expect(svg).toHaveAttribute("aria-hidden", "true");
		expect(view.observe).toHaveBeenCalledTimes(3);
		view.widen();
		expect(coordinates(view.container)[0]).not.toContain("102.5");
	});
	it("renders distinct saved stroke colors for adjacent regions and amber for legacy regions", () => {
		const view = harness([
			{ id: "upper", name: "Upper", color: "#06b6d4", members: [{ page: 1, position: 1 }] },
			{ id: "lower", name: "Lower", color: "#22c55e", members: [{ page: 1, position: 3 }] },
			{ id: "legacy", name: "Legacy", members: [{ page: 1, position: 1 }] },
		]);
		expect(view.container.querySelector('[data-solo-region="upper"]')).toHaveAttribute("stroke", "#06b6d4");
		expect(view.container.querySelector('[data-solo-region="lower"]')).toHaveAttribute("stroke", "#22c55e");
		expect(view.container.querySelector('[data-solo-region="legacy"]')).toHaveAttribute("stroke", "var(--amber)");
		expect(coordinates(view.container)).toHaveLength(3);
	});
	it("updates an authoritative region color without changing its measured outline", () => {
		const view = harness();
		const before = coordinates(view.container);
		view.recolor([{ ...regions[0], color: "#06b6d4" }]);
		expect(coordinates(view.container)).toEqual(before);
		expect(view.container.querySelector('[data-solo-region="pair"]')).toHaveAttribute("stroke", "#06b6d4");
	});
	it("keeps overlapping regions as closed perimeters in each region's own color", () => {
		const view = harness([
			{ ...regions[0], color: "#06b6d4" },
			{ id: "overlap", name: "Overlap", color: "#f97316", members: [{ page: 1, position: 1 }] },
		]);
		const pair = view.container.querySelector('[data-solo-region="pair"]')!;
		const overlap = view.container.querySelector('[data-solo-region="overlap"]')!;
		expect(pair).toHaveAttribute("stroke", "#06b6d4");
		expect(overlap).toHaveAttribute("stroke", "#f97316");
		expect(pair.getAttribute("d")).toMatch(/ Z$/);
		expect(overlap.getAttribute("d")).toMatch(/ Z$/);
		expect(pair.getAttribute("d")).not.toEqual(overlap.getAttribute("d"));
		expect(coordinates(view.container)).toHaveLength(2);
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
