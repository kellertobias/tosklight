/**
 * Generated stage equipment as a plan draws it: a flight rack, a PA speaker, a line array and a
 * disco ball on its chain.
 *
 * Each reads its one count off its height with the same measures the Visualizer builds it by
 * (`crates/viz/render/src/instances/scenery/equipment.rs`), so the plan shows the units, the pole
 * or the elements the 3D view does. A rack and a speaker stand on the floor they are placed on and
 * rise from there in an elevation; a line array hangs about its middle. From above, each is the box
 * it covers with its front drawn heavier, authored towards +y as every typed symbol is. A flight
 * rack is drawn as the road case it is, its corners rounded and capped in every view.
 */
import type { PlanPoint } from "./projection";
import type { CadViewDirection } from "./types";

type Colour = [number, number, number];

export interface EquipmentPolygon {
	points: PlanPoint[];
	color: Colour;
}

/** One 19-inch rack unit, and the lid and base a rack case adds, in millimetres. */
export const RACK_UNIT = 44.45;
export const RACK_CASE = 120;
/** A line array's flying frame and one element under it, in millimetres. */
export const LINE_ARRAY_FRAME = 100;
export const LINE_ARRAY_ELEMENT = 250;
/** A PA speaker cabinet's own height; anything the object is placed taller is its pole. */
export const PA_CABINET = 600;

const CASE: Colour = [0.25, 0.28, 0.32];
const BODY: Colour = [0.38, 0.42, 0.47];
const DETAIL: Colour = [0.57, 0.61, 0.66];
/** A road case's ball-corner protector, in millimetres, and the steps each rounded corner is drawn in. */
export const ROAD_CASE_CORNER = 80;
const ARC_STEPS = 4;

/** How many rack units a case of this height holds. */
export function rackUnits(height: number): number {
	return Math.min(48, Math.max(1, Math.round((height - RACK_CASE) / RACK_UNIT)));
}

/** A rack case's height for the units it holds. */
export function rackHeight(units: number): number {
	return RACK_CASE + RACK_UNIT * units;
}

/** How many elements a line array of this height hangs. */
export function lineArrayElements(height: number): number {
	return Math.min(24, Math.max(1, Math.round((height - LINE_ARRAY_FRAME) / LINE_ARRAY_ELEMENT)));
}

/** A line array's height for the elements it hangs. */
export function lineArrayHeight(elements: number): number {
	return LINE_ARRAY_FRAME + LINE_ARRAY_ELEMENT * elements;
}

/** How long a PA speaker's pole stand is, or 0 when it stands on its own cabinet. */
export function paPole(height: number): number {
	const pole = height - PA_CABINET;
	return pole > 50 ? pole : 0;
}

function rect(x: number, y: number, width: number, height: number, color: Colour): EquipmentPolygon {
	return {
		color,
		points: [
			[x, y],
			[x + width, y],
			[x + width, y + height],
			[x, y + height],
		],
	};
}

/** The box it covers from above, with its front edge — towards +y as authored — drawn heavier. */
function fromAbove(w: number, d: number): EquipmentPolygon[] {
	const lip = Math.min(40, d * 0.15);
	return [rect(-w / 2, -d / 2, w, d, BODY), rect(-w / 2, d / 2 - lip, w, lip, DETAIL)];
}

/** Points on a quarter circle, from `from` radians to a quarter turn on, about a centre. */
function arc(cx: number, cy: number, r: number, from: number): PlanPoint[] {
	return Array.from({ length: ARC_STEPS + 1 }, (_, step) => {
		const angle = from + (Math.PI / 2) * (step / ARC_STEPS);
		return [cx + r * Math.cos(angle), cy + r * Math.sin(angle)];
	});
}

/** A rectangle whose four corners are rounded to `r`, as a road case's shell is. */
function roundedRect(x: number, y: number, width: number, height: number, r: number, color: Colour): EquipmentPolygon {
	const radius = Math.min(r, width / 2, height / 2);
	return {
		color,
		points: [
			...arc(x + width - radius, y + radius, radius, -Math.PI / 2),
			...arc(x + width - radius, y + height - radius, radius, 0),
			...arc(x + radius, y + height - radius, radius, Math.PI / 2),
			...arc(x + radius, y + radius, radius, Math.PI),
		],
	};
}

/** How big a road case's corner protectors are for its size. */
function caseCorner(width: number, height: number): number {
	return Math.min(ROAD_CASE_CORNER, width * 0.22, height * 0.22);
}

/**
 * A road case: its shell with rounded corners and a ball-corner protector over each of them, the
 * shape that tells a flight case apart from any other box. Both scale with the case, so a small case
 * still reads as one and a tall one keeps small corners.
 */
function roadCase(x: number, y: number, width: number, height: number): EquipmentPolygon[] {
	const corner = caseCorner(width, height);
	const shell = roundedRect(x, y, width, height, corner * 0.6, CASE);
	const caps = [
		[x, y],
		[x + width - corner, y],
		[x + width - corner, y + height - corner],
		[x, y + height - corner],
	].map(([left, bottom]) => roundedRect(left, bottom, corner, corner, corner * 0.6, DETAIL));
	return [shell, ...caps];
}

function isSide(view: CadViewDirection): boolean {
	return view === "left_to_right" || view === "right_to_left";
}

/**
 * A flight rack: a road case with rounded, capped corners in every view, from the front with a panel
 * line at every unit it holds, and from above with its front edge drawn heavier.
 */
export function flightRackPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): EquipmentPolygon[] {
	const w = Math.max(100, horizontal);
	const h = Math.max(50, vertical);
	if (view === "top_down") {
		// The front edge runs between the corners, under their protectors.
		const [shell, ...caps] = roadCase(-w / 2, -h / 2, w, h);
		const corner = caseCorner(w, h);
		const lip = Math.min(40, h * 0.15);
		return [shell, rect(-w / 2 + corner, h / 2 - lip, w - 2 * corner, lip, DETAIL), ...caps];
	}
	const polygons = roadCase(-w / 2, 0, w, h);
	if (isSide(view)) return polygons;
	const units = rackUnits(h);
	for (let unit = 0; unit < units; unit += 1)
		polygons.push(
			rect(-w * 0.41, RACK_CASE / 2 + RACK_UNIT * unit + RACK_UNIT * 0.07, w * 0.82, RACK_UNIT * 0.86, BODY),
		);
	return polygons;
}

/** A PA speaker: its cabinet at the top of its height, on a pole and splayed feet when it has one. */
export function paSpeakerPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): EquipmentPolygon[] {
	const w = Math.max(100, horizontal);
	const h = Math.max(100, vertical);
	if (view === "top_down") return fromAbove(w, h);
	const pole = paPole(h);
	const cabinet = Math.min(h, PA_CABINET);
	const polygons = [rect(-w / 2, h - cabinet, w, cabinet, CASE)];
	if (!isSide(view)) polygons.push(rect(-w * 0.43, h - cabinet * 0.93, w * 0.86, cabinet * 0.86, BODY));
	if (!pole) return polygons;
	polygons.push(rect(-18, 0, 36, pole, DETAIL));
	// The tripod's feet spread from a collar a quarter of the way up the pole.
	const collar = Math.min(pole * 0.25, 400);
	const spread = Math.min(600, Math.max(250, pole * 0.35));
	for (const side of [-1, 1])
		polygons.push({
			color: DETAIL,
			points: [
				[-12 * side, collar],
				[12 * side, collar],
				[spread * side + 12 * side, 0],
				[spread * side - 12 * side, 0],
			],
		});
	return polygons;
}

/** The points of a circle about a centre, for a ball drawn from any side. */
function circle(x: number, y: number, radius: number, color: Colour): EquipmentPolygon {
	return {
		color,
		points: Array.from({ length: 24 }, (_, index): PlanPoint => {
			const angle = (index / 24) * Math.PI * 2;
			return [x + Math.cos(angle) * radius, y + Math.sin(angle) * radius];
		}),
	};
}

/**
 * A disco ball on its chain: from above the round ball; from the front or side the ball at the
 * bottom of its height with the chain running up from it to the top, about its middle.
 */
export function discoBallPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): EquipmentPolygon[] {
	const w = Math.max(100, horizontal);
	if (view === "top_down") return [circle(0, 0, w / 2, DETAIL)];
	const h = Math.max(w, vertical);
	const top = h / 2;
	const chain = h - w;
	const polygons = [circle(0, -top + w / 2, w / 2, DETAIL)];
	if (chain > 10) polygons.push(rect(-6, top - chain, 12, chain, CASE));
	return polygons;
}

/** A line array: its flying frame over the elements it hangs, about its middle. */
export function lineArrayPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): EquipmentPolygon[] {
	const w = Math.max(100, horizontal);
	const h = Math.max(100, vertical);
	if (view === "top_down") return fromAbove(w, h);
	const top = h / 2;
	const polygons = [rect(-w * 0.52, top - LINE_ARRAY_FRAME * 0.8, w * 1.04, LINE_ARRAY_FRAME * 0.6, DETAIL)];
	const elements = lineArrayElements(h);
	const element = (h - LINE_ARRAY_FRAME) / elements;
	for (let index = 0; index < elements; index += 1) {
		const bottom = top - LINE_ARRAY_FRAME - element * (index + 1);
		polygons.push(rect(-w / 2, bottom + element * 0.03, w, element * 0.94, CASE));
		if (!isSide(view)) polygons.push(rect(-w * 0.45, bottom + element * 0.15, w * 0.9, element * 0.7, BODY));
	}
	return polygons;
}
