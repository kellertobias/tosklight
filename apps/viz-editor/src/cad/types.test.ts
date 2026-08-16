import { describe, expect, it, vi } from "vitest";
import {
	CAD_VIEW_LABELS,
	mapTile,
	newTile,
	planeDelta,
	previewDeltaForEntity,
	projectPoint,
	removeSplitSide,
	setSplitRatio,
	splitTile,
	splitTileAtEdge,
	viewAxes,
} from "./types";

describe("CAD workspace model", () => {
	it("names every required orthographic direction literally", () => {
		expect(Object.values(CAD_VIEW_LABELS)).toEqual([
			"Top down",
			"Left to right",
			"Right to left",
			"Front to back",
			"Back to front",
		]);
	});

	it("maps screen-plane movement back onto the correct world axes", () => {
		expect(planeDelta([120, -40], "top_down")).toEqual([120, 40, 0]);
		expect(planeDelta([120, -40], "left_to_right")).toEqual([0, 120, -40]);
		expect(planeDelta([120, -40], "right_to_left")).toEqual([0, -120, -40]);
		expect(planeDelta([120, -40], "front_to_back")).toEqual([120, 0, -40]);
		expect(planeDelta([120, -40], "back_to_front")).toEqual([-120, 0, -40]);
		expect(projectPoint([10, 20, 30], "top_down")).toEqual([10, -20]);
	});

	it("interpolates positive and negative spread deltas in selection order", () => {
		const preview = {
			entityIds: ["first", "second", "third", "last"],
			deltaMillimetres: [900, -300, 120] as [number, number, number],
			spread: true,
		};
		expect(previewDeltaForEntity(preview, "first")).toEqual([0, 0, 0]);
		expect(previewDeltaForEntity(preview, "second")).toEqual([300, -100, 40]);
		expect(previewDeltaForEntity(preview, "third")).toEqual([600, -200, 80]);
		expect(previewDeltaForEntity(preview, "last")).toEqual([900, -300, 120]);
		expect(
			previewDeltaForEntity({ ...preview, spread: false }, "first"),
		).toEqual([900, -300, 120]);
	});

	it("keeps top-down projection, movement, and axes aligned after rotation", () => {
		expect(projectPoint([10, 20, 30], "top_down", 1)).toEqual([-20, -10]);
		expect(planeDelta([120, -40], "top_down", 1)).toEqual([40, -120, 0]);
		expect(viewAxes("top_down", 1)).toEqual({
			horizontal: { axis: "y", sign: -1 },
			vertical: { axis: "x", sign: -1 },
		});
		expect(projectPoint([10, 20, 30], "top_down", -1)).toEqual([20, 10]);
	});

	it("recursively splits one branch while its sibling remains whole", () => {
		vi.stubGlobal("crypto", {
			randomUUID: vi
				.fn()
				.mockReturnValueOnce("root")
				.mockReturnValueOnce("split-one")
				.mockReturnValueOnce("right")
				.mockReturnValueOnce("split-two")
				.mockReturnValueOnce("lower"),
		});
		const root = newTile();
		const first = splitTile(root, "root", "horizontal");
		expect(first.type).toBe("split");
		if (first.type !== "split") return;
		const siblingId = first.second.type === "tile" ? first.second.id : "";
		const second = splitTile(first, first.first.id, "vertical");
		expect(second.type).toBe("split");
		if (second.type !== "split") return;
		expect(second.second.type).toBe("tile");
		expect(second.first.type).toBe("split");
		const moved = mapTile(second, siblingId, (tile) => ({
			...tile,
			camera: { pan: [400, -200], zoom: 0.4 },
		}));
		if (moved.type !== "split" || moved.second.type !== "tile") return;
		expect(moved.second.camera).toEqual({ pan: [400, -200], zoom: 0.4 });
		vi.unstubAllGlobals();
	});

	it("adds the new viewport on the edge the operator chose and clamps resizing", () => {
		vi.stubGlobal("crypto", {
			randomUUID: vi
				.fn()
				.mockReturnValueOnce("root")
				.mockReturnValueOnce("left")
				.mockReturnValueOnce("split"),
		});
		const root = newTile();
		const split = splitTileAtEdge(root, "root", "left");
		expect(split.type).toBe("split");
		if (split.type !== "split") return;
		expect(split.direction).toBe("horizontal");
		expect(split.first.type === "tile" ? split.first.id : null).toBe("left");
		expect(split.second.type === "tile" ? split.second.id : null).toBe("root");
		expect(setSplitRatio(split, "split", 0.72)).toMatchObject({ ratio: 0.72 });
		expect(setSplitRatio(split, "split", 0.99)).toMatchObject({ ratio: 0.85 });
		vi.unstubAllGlobals();
	});

	it("removes either side of a recursive split without removing the survivor", () => {
		const first = newTile();
		const second = newTile();
		const split = {
			type: "split" as const,
			id: "split",
			direction: "vertical" as const,
			ratio: 0.4,
			first,
			second,
		};
		expect(removeSplitSide(split, "split", "second")).toBe(first);
		expect(removeSplitSide(split, "split", "first")).toBe(second);
		expect(removeSplitSide(first, "missing", "first")).toBe(first);
	});
});
