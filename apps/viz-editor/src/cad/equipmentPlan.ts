/**
 * Generated stage equipment as a plan draws it: a flight rack, a PA speaker and a line array.
 *
 * Each reads its one count off its height with the same measures the Visualizer builds it by
 * (`crates/viz/render/src/instances/scenery/equipment.rs`), so the plan shows the units, the pole
 * or the elements the 3D view does. A rack and a speaker stand on the floor they are placed on and
 * rise from there in an elevation; a line array hangs about its middle. From above, each is the box
 * it covers with its front drawn heavier, authored towards +y as every typed symbol is.
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

function isSide(view: CadViewDirection): boolean {
	return view === "left_to_right" || view === "right_to_left";
}

/** A flight rack: from the front, a panel line at every unit it holds; from the side, its case. */
export function flightRackPlan(
	horizontal: number,
	vertical: number,
	view: CadViewDirection,
): EquipmentPolygon[] {
	const w = Math.max(100, horizontal);
	const h = Math.max(50, vertical);
	if (view === "top_down") return fromAbove(w, h);
	const polygons = [rect(-w / 2, 0, w, h, CASE)];
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
