import type { AttributeValue } from "../../api/types/playback";
import { sampleSpread, spreadSamplePositions } from "./colorDisplay";

/** At most this many dots fit a pool tile's small preview square legibly. */
export const MAX_POSITION_DOTS = 10;

export type PositionSpace = "angles" | "target";

/** One requested aim: Pan/Tilt degrees, or a Target offset in metres (X right, Y upstage). */
export interface PositionPoint {
	space: PositionSpace;
	x: number;
	y: number;
}

/** A dot inside the unit preview square: x grows right, y grows up. */
export interface PreviewDot {
	x: number;
	y: number;
}

type PositionIntent = Extract<AttributeValue, { kind: "position" }>["value"];
type ScalarIntent = Extract<PositionIntent, { kind: "angles" }>["pan_degrees"];

function scalarAt(intent: ScalarIntent, t: number) {
	return intent.kind === "value" ? intent.value : sampleSpread(intent.value, t);
}

function scalarPoints(intent: ScalarIntent) {
	return intent.kind === "value" ? 1 : intent.value.length;
}

/**
 * The aims one stored Position intent asks for. `positions` places the samples of a spread, for
 * example one per Group member; without it a spread is sampled at its control points and between
 * them.
 */
export function positionValuePoints(
	value: AttributeValue,
	positions?: readonly number[],
): PositionPoint[] {
	if (value.kind !== "position") return [];
	const intent = value.value;
	const axes =
		intent.kind === "angles"
			? [intent.pan_degrees, intent.tilt_degrees]
			: [intent.offset_metres[0], intent.offset_metres[1]];
	const spread = axes.some((axis) => axis.kind === "spread");
	const samples = spread
		? (positions ?? spreadSamplePositions(Math.max(...axes.map(scalarPoints))))
		: [0];
	return samples.map((t) => ({
		space: intent.kind,
		x: scalarAt(axes[0], t),
		y: scalarAt(axes[1], t),
	}));
}

const EXTREME_DIRECTIONS: ReadonlyArray<readonly [number, number]> = [
	[-1, 0],
	[1, 0],
	[0, -1],
	[0, 1],
	[-1, -1],
	[1, 1],
	[-1, 1],
	[1, -1],
];

function rounded(value: number) {
	return Math.round(value * 1000) / 1000;
}

function byCoordinates(left: PositionPoint, right: PositionPoint) {
	return left.x - right.x || left.y - right.y;
}

/**
 * Chooses at most `limit` representative aims, deterministically and independent of input order.
 * The extremes on both axes and the outermost corners are always kept; the remaining dots are spaced evenly through the rest
 * of the distribution in coordinate order, so a dense cluster keeps proportionally more dots than
 * a lone outlier.
 */
export function representativePoints(
	points: readonly PositionPoint[],
	limit = MAX_POSITION_DOTS,
): PositionPoint[] {
	const unique = new Map<string, PositionPoint>();
	for (const point of points) {
		const key = `${rounded(point.x)}:${rounded(point.y)}`;
		if (!unique.has(key)) unique.set(key, { ...point, x: rounded(point.x), y: rounded(point.y) });
	}
	const sorted = [...unique.values()].sort(byCoordinates);
	if (sorted.length <= limit) return sorted;
	const pick = (score: (point: PositionPoint) => number) =>
		sorted.reduce((best, point, index) => (score(point) > score(sorted[best]) ? index : best), 0);
	// Both axes first, then the four diagonal corners, so a grid keeps all of its corners.
	const extremes = new Set(
		EXTREME_DIRECTIONS.map(([dx, dy]) => pick((p) => dx * p.x + dy * p.y)).slice(0, limit),
	);
	const rest = sorted.filter((_, index) => !extremes.has(index));
	const slots = limit - extremes.size;
	const chosen = new Set(extremes);
	for (let slot = 0; slot < slots; slot += 1) {
		const point = rest[Math.round(((slot + 0.5) * rest.length) / slots - 0.5)];
		chosen.add(sorted.indexOf(point));
	}
	return [...chosen].sort((left, right) => left - right).map((index) => sorted[index]);
}

/** The smallest span the square shows, so near-identical aims stay one central cluster. */
const MINIMUM_SPAN: Record<PositionSpace, number> = { angles: 20, target: 2 };
/** Share of the square the dots may occupy; the rest keeps them off its border. */
const USABLE = 0.8;

/** Places points in the unit square with one uniform scale, keeping the spread's true shape. */
export function previewDots(points: readonly PositionPoint[]): PreviewDot[] {
	if (points.length === 0) return [];
	const xs = points.map((point) => point.x);
	const ys = points.map((point) => point.y);
	const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
	const span = Math.max(maxX - minX, maxY - minY, MINIMUM_SPAN[points[0].space]);
	const [centerX, centerY] = [(minX + maxX) / 2, (minY + maxY) / 2];
	return points.map((point) => ({
		x: 0.5 + ((point.x - centerX) / span) * USABLE,
		y: 0.5 + ((point.y - centerY) / span) * USABLE,
	}));
}

/** The space most of the aims share; a Target wins a tie because it names what to look at. */
export function dominantSpace(points: readonly PositionPoint[]): PositionSpace | null {
	if (points.length === 0) return null;
	const target = points.filter((point) => point.space === "target").length;
	return target * 2 >= points.length ? "target" : "angles";
}
