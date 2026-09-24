/**
 * A flight of stairs in the CAD views, drawn the way `push_stairs` builds it in the Visualizer.
 *
 * The flight climbs along its longer side: towards +x when it is wider than deep, else towards −y
 * (downstage). Left and right are as seen climbing it. From above it is a line at every nosing, an
 * arrow up the climb and a solid rail along each railed side. In an elevation the view sees one
 * side of the unturned flight — the side its yaw turns towards the viewer, as a lamp's drawing is
 * read — and shows the steps in profile when it looks across the climb, climbing the way the flight
 * does, or the flight end on when it looks along it.
 */
import type { PlanPoint } from "./projection";
import type { CadStairHandrails, CadViewDirection } from "./types";

type Colour = [number, number, number];

export interface StairPolygon {
	points: PlanPoint[];
	color: Colour;
}

const BODY: Colour = [0.38, 0.42, 0.47];
const DETAIL: Colour = [0.57, 0.61, 0.66];
const DARK: Colour = [0.13, 0.15, 0.18];

/** The line a tread's nosing is drawn with from above, and the arrow up the flight. */
const NOSING = 20;
/** How high a handrail stands above the nosings, in millimetres. */
export const STAIR_RAIL_HEIGHT = 900;
/** The section a rail and its posts are drawn at. */
const RAIL_SECTION = 40;

function rect(x: number, y: number, width: number, height: number, color: Colour): StairPolygon {
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

/** How many steps a flight of this rise has, as `push_stairs` counts them. */
export function stairSteps(rise: number): number {
	return Math.min(24, Math.max(1, Math.round(rise / 200)));
}

/**
 * A flight from above. Points are authored the way every typed symbol is — mirrored top to bottom
 * by the plan afterwards (see `entityPlanGeometry`) — so up the page here is down the plan.
 */
export function stairPlan(
	w: number,
	d: number,
	rise: number,
	handrails: CadStairHandrails,
): StairPolygon[] {
	const alongX = w >= d;
	const run = alongX ? w : d;
	const across = alongX ? d : w;
	const steps = stairSteps(rise);
	const tread = run / steps;
	// A point `a` along the climb (from the bottom) and `b` towards the climber's left.
	const at = (a: number, b: number): PlanPoint =>
		alongX ? [-run / 2 + a, -b] : [b, a - run / 2];
	const band = (from: number, to: number, left: number, right: number, color: Colour): StairPolygon => ({
		color,
		points: [at(from, right), at(to, right), at(to, left), at(from, left)],
	});
	const polygons: StairPolygon[] = [rect(-w / 2, -d / 2, w, d, BODY)];
	for (let index = 1; index < steps; index += 1)
		polygons.push(
			band(tread * index - NOSING / 2, tread * index + NOSING / 2, across / 2, -across / 2, DARK),
		);
	// The arrow runs up the middle of the flight and points at the top step.
	const shaft = Math.max(NOSING, across * 0.06);
	const head = Math.min(across * 0.35, run * 0.3);
	polygons.push(band(run * 0.12, run * 0.88 - head, shaft / 2, -shaft / 2, DETAIL));
	polygons.push({
		color: DETAIL,
		points: [at(run * 0.88 - head, -head / 1.6), at(run * 0.88, 0), at(run * 0.88 - head, head / 1.6)],
	});
	// Each railed side is one solid line the length of the flight.
	const rail = Math.min(RAIL_SECTION * 1.5, across * 0.1);
	if (handrails === "left" || handrails === "both")
		polygons.push(band(0, run, across / 2, across / 2 - rail, DETAIL));
	if (handrails === "right" || handrails === "both")
		polygons.push(band(0, run, -across / 2 + rail, -across / 2, DETAIL));
	return polygons;
}

/**
 * How the unturned flight appears from `seen`: whether the view looks across the climb, and which
 * way along the page the climb or the climber's left runs (+1 to the right).
 *
 * The elevations' page x is desk +x from the front, −x from the back, −y from house left and +y
 * from house right; each looks along its own axis, so the viewer's right is page +x.
 */
function stairAspect(
	alongX: boolean,
	seen: CadViewDirection,
): { profile: boolean; climb: 1 | -1; left: 1 | -1; risers: boolean } {
	const frontOrBack = seen === "front_to_back" || seen === "back_to_front";
	// The climb as a direction on the page when seen across it.
	const climb: 1 | -1 = alongX
		? seen === "back_to_front"
			? -1
			: 1
		: seen === "right_to_left"
			? -1
			: 1;
	if (alongX === frontOrBack) return { profile: true, climb, left: 1, risers: false };
	// End on: the viewer looks up the flight (and sees its risers) or down it (and sees its back).
	// Looking up it, the climber's left is the viewer's left.
	const risers = alongX ? seen === "left_to_right" : seen === "back_to_front";
	return { profile: false, climb: 1, left: risers ? -1 : 1, risers };
}

/**
 * A flight in an elevation, as seen from `seen` (the view already turned by the flight's yaw): `w`
 * across and `d` deep as it was placed, `h` high, standing on its origin.
 */
export function stairElevation(
	w: number,
	d: number,
	h: number,
	handrails: CadStairHandrails,
	seen: CadViewDirection,
): StairPolygon[] {
	const alongX = w >= d;
	const aspect = stairAspect(alongX, seen);
	const steps = stairSteps(h);
	const rise = h / steps;
	const railed = [
		...(handrails === "left" || handrails === "both" ? [1] : []),
		...(handrails === "right" || handrails === "both" ? [-1] : []),
	];
	const polygons: StairPolygon[] = [];
	if (aspect.profile) {
		const run = alongX ? w : d;
		const tread = run / steps;
		// `x` along the climb from the bottom step, turned onto the page.
		const page = (x: number) => aspect.climb * (-run / 2 + x);
		const box = (from: number, to: number, bottom: number, top: number, color: Colour) =>
			rect(Math.min(page(from), page(to)), bottom, Math.abs(page(to) - page(from)), top - bottom, color);
		for (let index = 0; index < steps; index += 1)
			polygons.push(box(tread * index, tread * (index + 1), 0, rise * (index + 1), BODY));
		if (!railed.length) return polygons;
		// A post on every nosing up to the rail, and the rail itself along their tops, following the
		// climb from the first nosing to the last, where it stands one rail-height over the top step.
		const rail = Math.min(STAIR_RAIL_HEIGHT, Math.max(200, h));
		for (let index = 0; index <= steps; index += 1) {
			const x = tread * index;
			polygons.push(box(x - RAIL_SECTION / 2, x + RAIL_SECTION / 2, rise * index, rise * index + rail, DETAIL));
		}
		const top = (x: number) => (h * x) / run + rail;
		polygons.push({
			color: DETAIL,
			points: [
				[page(0), top(0) - RAIL_SECTION],
				[page(run), top(run) - RAIL_SECTION],
				[page(run), top(run)],
				[page(0), top(0)],
			],
		});
		return polygons;
	}
	// End on: the flight's full height and width, its risers when it is seen from the bottom, and
	// each rail as the post standing over its side of the top step.
	const across = alongX ? d : w;
	polygons.push(rect(-across / 2, 0, across, h, BODY));
	if (aspect.risers)
		for (let index = 1; index < steps; index += 1)
			polygons.push(rect(-across / 2, rise * index - NOSING / 2, across, NOSING, DARK));
	const rail = Math.min(STAIR_RAIL_HEIGHT, Math.max(200, h));
	for (const side of railed) {
		const centre = aspect.left * side * (across / 2 - RAIL_SECTION / 2);
		polygons.push(rect(centre - RAIL_SECTION / 2, 0, RAIL_SECTION, h + rail, DETAIL));
	}
	return polygons;
}
