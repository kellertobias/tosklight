import { afterEach, expect, it, vi } from "vitest";
import { fittedCamera } from "./CadApp";
import { type CadFrame, LineRenderer } from "./lineRenderer";
import {
	CAD_VIEW_LABELS,
	type CadEntity,
	type CadViewDirection,
} from "./types";

const venue: CadEntity = {
	id: "venue",
	logicalFixtureId: "venue",
	name: "Rotated venue",
	fixtureNumber: 1,
	fixtureDisplayId: "1",
	dmxAddress: "",
	kind: "venue",
	fixtureType: "venue",
	drawingId: "venue",
	layerId: "default",
	selectable: true,
	positionMillimetres: [9000, -4000, 6000],
	rotationDegrees: [15, 25, 35],
	sizeMillimetres: [30000, 12000, 18000],
	scenery: { kind: "box", chords: 0, pattern: "standard" },
	outputDirection: [0, 1, 0],
};
const entities: CadEntity[] = [
	venue,
	{
		...venue,
		id: "fixture",
		logicalFixtureId: "fixture",
		kind: "profile",
		fixtureType: "moving_head_profile",
		scenery: undefined,
		positionMillimetres: [24000, 12000, 6000],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [400, 500, 700],
		emitterOffsetMillimetres: [300, 200, 100],
	},
];

/** Observe the real renderer's uploads and draw calls; no geometry or camera is mocked. */
function recordingRenderer(size: { width: number; height: number }) {
	const canvas = document.createElement("canvas");
	Object.defineProperties(canvas, {
		clientWidth: { get: () => size.width },
		clientHeight: { get: () => size.height },
	});
	const buffers = new Map<object, Float32Array>();
	let bound: object;
	let pan = [Number.NaN, Number.NaN];
	let scale = [Number.NaN, Number.NaN];
	let viewport: number[] = [];
	const shaders: string[] = [];
	const draws: {
		data: Float32Array;
		pan: number[];
		scale: number[];
		viewport: number[];
	}[] = [];
	const gl = new Proxy(
		{},
		{
			get: (_, key) => {
				if (key === "getProgramParameter" || key === "getShaderParameter")
					return () => true;
				if (key === "getUniformLocation")
					return (_: unknown, name: string) => name;
				if (key === "shaderSource")
					return (_: unknown, source: string) => shaders.push(source);
				if (key === "bindBuffer")
					return (_: unknown, buffer: object) => {
						bound = buffer;
					};
				if (key === "bufferData")
					return (_: unknown, data: Float32Array) =>
						buffers.set(bound, data.slice());
				if (key === "uniform2f")
					return (name: string, x: number, y: number) => {
						if (name === "pan") pan = [x, y];
						if (name === "scale") scale = [x, y];
					};
				if (key === "viewport")
					return (...values: number[]) => {
						viewport = values;
					};
				if (key === "drawArrays")
					return () => {
						const data = buffers.get(bound);
						if (!data)
							throw new Error("Draw without an uploaded vertex buffer");
						draws.push({
							data,
							pan: [...pan],
							scale: [...scale],
							viewport: [...viewport],
						});
					};
				if (typeof key === "string" && key === key.toUpperCase()) return 1;
				return () => ({});
			},
		},
	) as WebGL2RenderingContext;
	canvas.getContext = (() => gl) as never;
	const renderer = LineRenderer.create(canvas);
	if (!renderer) throw new Error("Renderer creation failed");
	return { canvas, renderer, draws, shaders };
}

afterEach(() => vi.unstubAllGlobals());

for (const ratio of [1, 2]) {
	it(`keeps uploaded rig vertices inside 32 CSS pixel margins at DPR ${ratio}, through unequal panel resizes`, () => {
		vi.stubGlobal("devicePixelRatio", ratio);
		const before = JSON.stringify(entities);
		const size = { width: 410, height: 760 };
		const { canvas, renderer, draws, shaders } = recordingRenderer(size);
		// The projection below evaluates the vertex shader that LineRenderer actually compiled.
		expect(shaders[0]).toContain(
			"gl_Position=vec4((position.xy+pan)*scale,position.z,1.0)",
		);
		const selected = new Set<string>();
		const drawings = new Map();
		for (const view of Object.keys(CAD_VIEW_LABELS) as CadViewDirection[]) {
			for (const turn of view === "top_down" ? [0, 1, 2, 3] : [0]) {
				for (const [width, height] of [
					[410, 760],
					[900, 330],
					[380, 160],
					[1000, 800],
				]) {
					Object.assign(size, { width, height });
					const camera = fittedCamera(
						entities,
						view,
						turn,
						drawings,
						width,
						height,
					);
					const frame: CadFrame = {
						entities,
						drawings,
						selected,
						view,
						rotationQuarterTurns: turn,
						camera,
						preview: null,
						editEnabled: false,
						guide: null,
						selectionBox: null,
					};
					const start = draws.length;
					renderer.draw(frame);
					expect([canvas.width, canvas.height]).toEqual([
						width * ratio,
						height * ratio,
					]);
					let rigVertices = 0;
					let limitingMargin = Infinity;
					for (const draw of draws.slice(start)) {
						expect(draw.viewport).toEqual([
							0,
							0,
							width * ratio,
							height * ratio,
						]);
						for (let offset = 0; offset < draw.data.length; offset += 7) {
							// Datum/guide overlays intentionally extend to the viewport edge. Inspect
							// actual venue, fixture and direction vertices, identified by their ink.
							const red = draw.data[offset + 3];
							if (
								![0.018, 0.56, 0.8, 0.98].some(
									(value) => Math.abs(value - red) < 1e-6,
								)
							)
								continue;
							const clipX = (draw.data[offset] + draw.pan[0]) * draw.scale[0];
							const clipY =
								(draw.data[offset + 1] + draw.pan[1]) * draw.scale[1];
							// WebGL viewport converts NDC to backing pixels, then CSS scaling
							// divides by DPR. Using backing dimensions for uniforms would fail here.
							const x = ((clipX + 1) * draw.viewport[2]) / 2 / ratio;
							const y = ((1 - clipY) * draw.viewport[3]) / 2 / ratio;
							limitingMargin = Math.min(
								limitingMargin,
								x,
								width - x,
								y,
								height - y,
							);
							expect(x).toBeGreaterThanOrEqual(32 - 0.001);
							expect(x).toBeLessThanOrEqual(width - 32 + 0.001);
							expect(y).toBeGreaterThanOrEqual(32 - 0.001);
							expect(y).toBeLessThanOrEqual(height - 32 + 0.001);
							rigVertices++;
						}
					}
					expect(rigVertices).toBeGreaterThan(20);
					// A wrongly doubled backing-pixel scale can shrink the whole rig and still
					// keep it within the viewport. Fit must also fill its limiting dimension.
					expect(limitingMargin).toBeCloseTo(32, 2);
				}
			}
		}
		expect(JSON.stringify(entities)).toBe(before);
	});
}
