import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CadViewport } from "./CadViewport";
import type { TileCamera } from "./types";
import { anchorTopLeft, type ViewportSize } from "./useTopLeftAnchor";

/** The plan point drawn at a viewport's top-left corner, by the viewport's own screen mapping. */
function topLeftPlanPoint(camera: TileCamera, size: ViewportSize) {
	return [
		-size.width / 2 / camera.zoom - camera.pan[0],
		size.height / 2 / camera.zoom - camera.pan[1],
	];
}

describe("anchorTopLeft", () => {
	const camera: TileCamera = { pan: [1250, -430], zoom: 0.137 };
	const cases: [string, ViewportSize, ViewportSize][] = [
		["a sidebar opens", { width: 1400, height: 800 }, { width: 1080, height: 800 }],
		["a sidebar closes", { width: 1080, height: 800 }, { width: 1400, height: 800 }],
		["a sidebar is dragged wider", { width: 1100, height: 800 }, { width: 1033, height: 800 }],
		["the viewport grows taller", { width: 900, height: 500 }, { width: 900, height: 740 }],
	];

	it.each(cases)("keeps the top-left plan point and zoom when %s", (_, from, to) => {
		const anchored = anchorTopLeft(camera, from, to);
		expect(anchored.zoom).toBe(camera.zoom);
		const before = topLeftPlanPoint(camera, from);
		const after = topLeftPlanPoint(anchored, to);
		expect(after[0]).toBeCloseTo(before[0], 9);
		expect(after[1]).toBeCloseTo(before[1], 9);
	});

	it("returns the same camera when the size did not change", () => {
		const size = { width: 800, height: 600 };
		expect(anchorTopLeft(camera, size, size)).toBe(camera);
	});
});

describe("CadViewport resize", () => {
	let observers: ResizeObserverCallback[] = [];
	let width = 1400;
	let height = 800;

	beforeEach(() => {
		observers = [];
		vi.stubGlobal(
			"ResizeObserver",
			class {
				constructor(callback: ResizeObserverCallback) {
					observers.push(callback);
				}
				observe() {}
				disconnect() {}
			},
		);
		vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
		vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => width);
		vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(() => height);
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.unstubAllGlobals();
	});

	function resizeTo(nextWidth: number, nextHeight: number) {
		width = nextWidth;
		height = nextHeight;
		for (const notify of observers) notify([], {} as ResizeObserver);
	}

	it("hands the layout a camera that keeps the top-left corner when a sidebar changes the width", async () => {
		const onCamera = vi.fn();
		const camera: TileCamera = { pan: [200, 100], zoom: 0.1 };
		render(
			<CadViewport
				entities={[]}
				drawings={[]}
				selectedIds={[]}
				view="top_down"
				rotationQuarterTurns={0}
				camera={camera}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				onCamera={onCamera}
				onSelection={vi.fn()}
				onPreview={vi.fn()}
				onMove={vi.fn().mockResolvedValue(undefined)}
			/>,
		);
		resizeTo(1400, 800);
		resizeTo(1080, 800);
		resizeTo(1000, 800);

		await waitFor(() => expect(onCamera).toHaveBeenCalledTimes(1));
		const committed = onCamera.mock.calls[0][0] as TileCamera;
		expect(committed.zoom).toBe(0.1);
		const before = topLeftPlanPoint(camera, { width: 1400, height: 800 });
		const after = topLeftPlanPoint(committed, { width: 1000, height: 800 });
		expect(after[0]).toBeCloseTo(before[0], 9);
		expect(after[1]).toBeCloseTo(before[1], 9);
	});
});
