/**
 * A truss in plan and elevation, drawn the way it is built.
 *
 * The proportions follow the square truss a rig is usually hung from (Global Truss F34, Prolyte
 * H30V): 290 mm outside, 50 mm chords and 20 mm braces; a node about every chord spacing, so the
 * diagonals run near 45°; an end frame just inside each end and no uprights between them; a coupler
 * receiver on every chord end with the conical coupler ("egg") centred on the joint. Every face
 * zig-zags from the chord before it going round the section, so opposite faces run the other way
 * and a side view shows an X in every bay. The 3D view
 * (`crates/viz/render/src/instances/scenery/truss.rs`) builds from the same rules.
 *
 * Every member is laid out in the truss's own width/depth/height millimetres and then projected,
 * so each view — along the run or looking down it — falls out of one description.
 */
import type { CadViewDirection } from "./types";

type Colour = [number, number, number];
type Point = [number, number];
type Vector = [number, number, number];

export interface TrussPolygon {
	points: Point[];
	color: Colour;
}

const RECEIVER: Colour = [0.25, 0.28, 0.32];
const BRACE: Colour = [0.38, 0.42, 0.47];
const CHORD: Colour = [0.57, 0.61, 0.66];
const COUPLER: Colour = [0.72, 0.75, 0.79];

/** The members of one truss section, in millimetres, from its outside size alone. */
export function trussParts(section: number, chordCount: number) {
	const chords = Math.min(4, Math.max(1, Math.round(chordCount)));
	const chord =
		chords === 1 ? clamp(section, 20, 100) : clamp(section * 0.17, 25, 60);
	const brace = chord * 0.4;
	return {
		chords,
		chord,
		brace,
		spacing: Math.max(chord, section - chord),
		receiver: chord,
		endFrame: chord + brace / 2,
	};
}

type Parts = ReturnType<typeof trussParts>;

/** Bays between the end frames of a piece `length` long: as many as keep the diagonals nearest 45°. */
export function trussBays(parts: Parts, length: number): number {
	const inner = length - parts.endFrame * 2;
	if (inner <= parts.brace) return 0;
	return clamp(Math.round(inner / parts.spacing), 1, 400);
}

type Member =
	| {
			kind: "tube";
			from: Vector;
			to: Vector;
			diameter: number;
			color: Colour;
			/** A brace stops at the chord surface, half a chord diameter short of its centre. */
			brace: boolean;
	  }
	| { kind: "coupler"; at: Vector; axis: Vector; diameter: number };

export function trussPlan(
	sizeMillimetres: readonly [number, number, number],
	view: CadViewDirection,
	chordCount: number,
	deco: boolean,
): TrussPolygon[] {
	const size = sizeMillimetres.map((value) => Math.max(20, value)) as Vector;
	const run = size.indexOf(Math.max(...size));
	const [acrossAxis, upAxis] = run === 0 ? [1, 2] : run === 1 ? [0, 2] : [0, 1];
	const length = size[run];
	const parts = trussParts(
		Math.max(size[acrossAxis], size[upAxis]),
		chordCount,
	);
	const point = (along: number, [across, up]: Point): Vector => {
		const result: Vector = [0, 0, 0];
		result[run] = along - length / 2;
		result[acrossAxis] = across;
		result[upAxis] = up;
		return result;
	};
	const unit: Vector = [0, 0, 0];
	unit[run] = 1;
	const chords = chordOffsets(parts);
	const faces = trussFaces(parts);
	const members: Member[] = [];
	const tube = (
		from: Vector,
		to: Vector,
		diameter: number,
		color: Colour,
		brace = false,
	) => members.push({ kind: "tube", from, to, diameter, color, brace });
	const brace = (from: Vector, to: Vector) =>
		tube(from, to, parts.brace, BRACE, true);

	const bays = trussBays(parts, length);
	const bay = bays ? (length - parts.endFrame * 2) / bays : 0;
	for (const [first, second] of faces) {
		for (let index = 0; index < bays; index++) {
			const near = parts.endFrame + bay * index;
			// Deco truss crosses its diagonals in every bay.
			for (const forward of deco ? [true, false] : [index % 2 === 0]) {
				const [from, to] = forward ? [first, second] : [second, first];
				brace(point(near, chords[from]), point(near + bay, chords[to]));
			}
		}
	}
	for (const along of chords.length > 1
		? [parts.endFrame, length - parts.endFrame]
		: []) {
		for (const [first, second] of faces)
			brace(point(along, chords[first]), point(along, chords[second]));
		// A box's end frame carries one diagonal across it, as the end elevation shows.
		if (chords.length === 4)
			brace(point(along, chords[0]), point(along, chords[2]));
	}
	for (const offset of chords)
		tube(point(0, offset), point(length, offset), parts.chord, CHORD);
	if (chords.length > 1) {
		for (const offset of chords) {
			for (const [from, to] of [
				[0, parts.receiver],
				[length - parts.receiver, length],
			])
				tube(point(from, offset), point(to, offset), parts.chord * 1.2, RECEIVER);
			for (const along of [0, length])
				members.push({
					kind: "coupler",
					at: point(along, offset),
					axis: unit,
					diameter: parts.chord,
				});
		}
	}
	return drawMembers(members, view, unit, parts.chord);
}

/** Where each chord sits in the section, as (across, up), in order round it so neighbours share a face. */
export function chordOffsets(parts: Parts): Point[] {
	const half = parts.spacing / 2;
	switch (parts.chords) {
		case 1:
			return [[0, 0]];
		case 2:
			return [
				[0, half],
				[0, -half],
			];
		case 3:
			return [
				[0, half],
				[half, -half],
				[-half, -half],
			];
		default:
			return [
				[half, half],
				[-half, half],
				[-half, -half],
				[half, -half],
			];
	}
}

/** Faces as neighbouring chord pairs, taken in order round the section. */
function trussFaces(parts: Parts): [number, number][] {
	if (parts.chords < 2) return [];
	if (parts.chords === 2) return [[0, 1]];
	return Array.from({ length: parts.chords }, (_, face) => [
		face,
		(face + 1) % parts.chords,
	]);
}

function drawMembers(
	members: Member[],
	view: CadViewDirection,
	runUnit: Vector,
	chord: number,
): TrussPolygon[] {
	const run = project(runUnit, view);
	const endOn = Math.hypot(...run) < 0.5;
	const polygons: TrussPolygon[] = [];
	const seen = new Set<string>();
	const add = (key: string, polygon: TrussPolygon) => {
		if (seen.has(key)) return;
		seen.add(key);
		polygons.push(polygon);
	};
	for (const member of members) {
		if (member.kind === "coupler") {
			const polygon = coupler(member, view);
			add(`coupler|${pointsKey(polygon.points)}`, polygon);
			continue;
		}
		let from = project(member.from, view);
		let to = project(member.to, view);
		const dx = to[0] - from[0];
		const dy = to[1] - from[1];
		const length = Math.hypot(dx, dy);
		if (member.brace) {
			const runs = Math.abs(member.to[0] - member.from[0]) +
				Math.abs(member.to[1] - member.from[1]) +
				Math.abs(member.to[2] - member.from[2]);
			const alongRun = Math.abs(
				(member.to[0] - member.from[0]) * runUnit[0] +
					(member.to[1] - member.from[1]) * runUnit[1] +
					(member.to[2] - member.from[2]) * runUnit[2],
			);
			// Hidden inside a chord: seen end-on as a dot, lying along the run in a face seen
			// edge-on, or — looking down the run — a diagonal falling on its own end frame.
			if (length < 1 || (endOn && alongRun > 0.5)) continue;
			const sine = endOn ? 1 : Math.abs(dx * run[1] - dy * run[0]) / length;
			if (runs > 0 && sine < 0.01) continue;
			// Stop at the chord's edge as this view shows it.
			const trim = Math.min(length / 2.5, chord / 2 / Math.max(sine, 0.3));
			from = [from[0] + (dx / length) * trim, from[1] + (dy / length) * trim];
			to = [to[0] - (dx / length) * trim, to[1] - (dy / length) * trim];
		}
		const ends = [from, to].map((end) => end.map(Math.round).join(",")).sort();
		const key = `${member.color.join()}|${member.diameter}|${ends.join(";")}`;
		if (Math.hypot(to[0] - from[0], to[1] - from[1]) < 1) {
			add(key, circle(from, member.diameter / 2, member.color));
		} else {
			add(key, band(from, to, member.diameter, member.color));
		}
	}
	return polygons;
}

/** A conical coupler: two cones back to back, widest in the joint, seen along or across its axis. */
function coupler(
	member: Extract<Member, { kind: "coupler" }>,
	view: CadViewDirection,
): TrussPolygon {
	const centre = project(member.at, view);
	const [ax, ay] = project(member.axis, view);
	const diameter = member.diameter * 0.7;
	if (Math.hypot(ax, ay) < 0.5)
		return circle(centre, diameter / 2, COUPLER, 10);
	const half = member.diameter * 0.8;
	const profile: Point[] = [
		[-half, 0.4],
		[-half * 0.25, 0.5],
		[half * 0.25, 0.5],
		[half, 0.4],
		[half, -0.4],
		[half * 0.25, -0.5],
		[-half * 0.25, -0.5],
		[-half, -0.4],
	];
	return {
		color: COUPLER,
		points: profile.map(([along, side]) => [
			centre[0] + ax * along - ay * side * diameter,
			centre[1] + ay * along + ax * side * diameter,
		]),
	};
}

function project(point: Vector, view: CadViewDirection): Point {
	// Width, depth, height millimetres, laid out the way the typed plan symbols are.
	switch (view) {
		case "top_down":
			return [point[0], point[1]];
		case "front_to_back":
		case "back_to_front":
			return [point[0], point[2]];
		case "left_to_right":
		case "right_to_left":
			return [point[1], point[2]];
	}
}

function pointsKey(points: Point[]): string {
	return points.map((point) => point.map(Math.round).join(",")).join(";");
}

function band(
	from: Point,
	to: Point,
	thickness: number,
	color: Colour,
): TrussPolygon {
	const dx = to[0] - from[0];
	const dy = to[1] - from[1];
	const length = Math.max(1e-6, Math.hypot(dx, dy));
	const x = (-dy / length) * (thickness / 2);
	const y = (dx / length) * (thickness / 2);
	return {
		color,
		points: [
			[from[0] + x, from[1] + y],
			[to[0] + x, to[1] + y],
			[to[0] - x, to[1] - y],
			[from[0] - x, from[1] - y],
		],
	};
}

function circle(
	centre: Point,
	radius: number,
	color: Colour,
	segments = 14,
): TrussPolygon {
	return {
		color,
		points: Array.from({ length: segments }, (_, index) => {
			const angle = (index / segments) * Math.PI * 2;
			return [
				centre[0] + Math.cos(angle) * radius,
				centre[1] + Math.sin(angle) * radius,
			];
		}),
	};
}

function clamp(value: number, low: number, high: number): number {
	return Math.min(high, Math.max(low, value));
}
