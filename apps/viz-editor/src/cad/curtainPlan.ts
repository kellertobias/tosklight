/**
 * A curtain as a plan draws soft goods: seen from above, a wavy line along its track, four waves
 * for every fold it hangs in; seen from the front, the rectangle it covers with a dotted line down each
 * fold; seen from the side, the thin slab of its gathered depth.
 */
import type { PlanGeometry, PlanLine, PlanPoint, PlanTriangle } from "./projection";
import type { CadViewDirection } from "./types";

const FILL: [number, number, number] = [0.38, 0.42, 0.47];

/** How wide one fold hangs, in millimetres — the same fullness the 3D view gathers a drape in. */
const FOLD_MILLIMETRES = 450;

/** Waves the plan draws in each fold: fine enough to read as gathered fabric at plan scale. */
const WAVES_PER_FOLD = 4;

/** How far each seam in a front view leans off vertical, in turn: alternating sides, 10° to 20°. */
const FOLD_LEAN_DEGREES = [10, -20, 20, -10];

export function curtainPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): PlanGeometry {
	const w = Math.max(100, horizontal);
	const h = Math.max(20, vertical);
	if (view === "top_down") return wave(w, h);
	if (view === "left_to_right" || view === "right_to_left")
		return panel(w, h, []);
	const folds = Math.min(80, Math.max(2, Math.round(w / FOLD_MILLIMETRES)));
	const seams = Array.from(
		{ length: folds - 1 },
		(_, index) => -w / 2 + ((index + 1) / folds) * w,
	);
	return panel(w, h, seams);
}

/** The track from above: a sine wave across the width, half as deep as the curtain is gathered. */
function wave(width: number, depth: number): PlanGeometry {
	const folds = Math.min(80, Math.max(2, Math.round(width / FOLD_MILLIMETRES)));
	const waves = folds * WAVES_PER_FOLD;
	const amplitude = Math.max(7.5, depth / 4);
	const samples = waves * 16;
	const points: PlanPoint[] = Array.from({ length: samples + 1 }, (_, index) => {
		const t = index / samples;
		return [
			-width / 2 + t * width,
			amplitude * Math.sin(t * waves * Math.PI * 2),
		];
	});
	const lines: PlanLine[] = [];
	const triangles: PlanTriangle[] = [];
	// A thin band under the line keeps the curtain pickable and hides what it stands in front of.
	const band = Math.max(8, amplitude * 0.25);
	for (let index = 1; index < points.length; index++) {
		const [ax, ay] = points[index - 1];
		const [bx, by] = points[index];
		lines.push({ points: [points[index - 1], points[index]] });
		triangles.push(
			{ points: [[ax, ay - band], [bx, by - band], [bx, by + band]], color: FILL },
			{ points: [[ax, ay - band], [bx, by + band], [ax, ay + band]], color: FILL },
		);
	}
	return { source: "typed", triangles, outlines: [], lines };
}

/** The face a curtain covers: its rectangle, with a dotted line down every seam between folds. */
function panel(width: number, height: number, seams: number[]): PlanGeometry {
	const corners: PlanPoint[] = [
		[-width / 2, -height / 2],
		[width / 2, -height / 2],
		[width / 2, height / 2],
		[-width / 2, height / 2],
	];
	const dash = Math.min(120, Math.max(30, height / 80));
	const lines: PlanLine[] = [];
	seams.forEach((x, seam) => {
		// The seam runs straight down; its dashes lean 10° to 20° off vertical, each the other way
		// from the one before, so the line reads as fabric folding rather than a ruled line.
		let index = seam;
		for (let y = height / 2 - dash; y - dash >= -height / 2; y -= dash * 2) {
			const degrees = FOLD_LEAN_DEGREES[index++ % FOLD_LEAN_DEGREES.length];
			const radians = (degrees * Math.PI) / 180;
			const half: PlanPoint = [
				(Math.sin(radians) * dash) / 2,
				(Math.cos(radians) * dash) / 2,
			];
			const centre = y - dash / 2;
			lines.push({
				points: [
					[x - half[0], centre - half[1]],
					[x + half[0], centre + half[1]],
				],
			});
		}
	});
	return {
		source: "typed",
		triangles: [
			{ points: [corners[0], corners[1], corners[2]], color: FILL },
			{ points: [corners[0], corners[2], corners[3]], color: FILL },
		],
		outlines: [corners],
		lines,
	};
}
