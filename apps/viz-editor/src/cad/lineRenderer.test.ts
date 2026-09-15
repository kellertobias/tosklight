import { expect, it } from "vitest";
import { type CadFrame, LineRenderer } from "./lineRenderer";
import type { CadEntity } from "./types";

/** A WebGL2 context that accepts every call and counts uploads, so a frame's work runs in jsdom. */
function fakeGl(uploads: { count: number }) {
	return new Proxy(
		{},
		{
			get: (_, key) => {
				if (key === "getProgramParameter" || key === "getShaderParameter") return () => true;
				if (key === "bufferData") return () => (uploads.count += 1);
				if (typeof key === "string" && key === key.toUpperCase()) return 1;
				return () => ({});
			},
		},
	) as WebGL2RenderingContext;
}

function rig(count: number): CadEntity[] {
	return Array.from({ length: count }, (_, index) => ({
		id: `e${index}`,
		logicalFixtureId: `e${index}`,
		name: `E ${index}`,
		fixtureNumber: index,
		fixtureDisplayId: String(index),
		dmxAddress: "1.1",
		kind: index % 3 ? "profile" : "venue",
		fixtureType: index % 3 ? "moving_head_profile" : "rigging",
		drawingId: index % 3 ? "profile:1" : "truss",
		layerId: "default",
		selectable: true,
		positionMillimetres: [(index % 30) * 600, Math.floor(index / 30) * 900, 4000],
		rotationDegrees: [0, 0, (index * 15) % 360],
		sizeMillimetres: index % 3 ? [400, 500, 700] : [3000, 290, 290],
		outputDirection: [0, 1, 0],
		...(index % 3 ? {} : { scenery: { kind: "truss", chords: 4, pattern: "standard" } }),
	})) as CadEntity[];
}

function renderer() {
	const canvas = document.createElement("canvas");
	Object.defineProperty(canvas, "clientWidth", { value: 1200 });
	Object.defineProperty(canvas, "clientHeight", { value: 800 });
	const uploads = { count: 0 };
	canvas.getContext = (() => fakeGl(uploads)) as never;
	const created = LineRenderer.create(canvas);
	if (!created) throw new Error("renderer was not created");
	return { renderer: created, uploads };
}

it("pans and zooms a large rig without rebuilding or re-uploading its linework", () => {
	const { renderer: lines, uploads } = renderer();
	const frame: CadFrame = {
		entities: rig(600),
		drawings: new Map(),
		selected: new Set(["e1"]),
		view: "top_down",
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.05 },
		preview: null,
		editEnabled: true,
		guide: null,
		selectionBox: null,
	};
	lines.draw(frame);
	const firstUploads = uploads.count;
	const frames = 120;
	const started = performance.now();
	for (let index = 0; index < frames; index++)
		lines.draw({ ...frame, camera: { pan: [index * 10, 0], zoom: 0.05 + index / 10_000 } });
	const perFrame = (performance.now() - started) / frames;
	console.log(`[bench] pan frame over 600 entities: ${perFrame.toFixed(3)} ms`);
	expect(lines.layers.builds).toEqual({ still: 1, moving: 0 });
	// Only the small camera-sized overlays (gizmo, datum) are uploaded again on a pan frame.
	expect(uploads.count - firstUploads).toBeLessThanOrEqual(frames * 2);
});

it("rebuilds only the dragged elements while a move previews", () => {
	const { renderer: lines } = renderer();
	const entities = rig(600);
	const frame: CadFrame = {
		entities,
		drawings: new Map(),
		selected: new Set(["e1"]),
		view: "top_down",
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.05 },
		preview: null,
		editEnabled: true,
		guide: null,
		selectionBox: null,
	};
	lines.draw(frame);
	const entityIds = ["e1"];
	const started = performance.now();
	for (let index = 1; index <= 60; index++)
		lines.draw({
			...frame,
			preview: { entityIds, deltaMillimetres: [index * 10, 0, 0], spread: false },
		});
	const perFrame = (performance.now() - started) / 60;
	console.log(`[bench] drag frame over 600 entities: ${perFrame.toFixed(3)} ms`);
	// The rig standing still is rebuilt once, without the dragged element, and then kept.
	expect(lines.layers.builds).toEqual({ still: 2, moving: 60 });
});
