/**
 * Typed movement while dragging the gizmo: the live coordinates beside it, and a number typed into
 * the drag that sets or shifts the position on the active axis.
 *
 * Coordinates are world coordinates in metres on the two axes the view shows, whatever way the
 * view turns or mirrors them, so a typed value means the same as the Info panel's position field.
 * `2.5` puts the selection's origin at 2.5 m on the active axis; `+0.5` and `-0.5` move it by that
 * much along the axis's positive or negative world direction.
 */
import type { MoveAxis } from "./planGeometry";
import type { CadEntity, CadViewDirection, WorldAxis } from "./types";
import { viewAxes } from "./types";

export interface MoveEntry {
	mode: "absolute" | "relative";
	metres: number;
}

/** The screen axis a typed value applies to: the locked arrow, or the one chosen with Tab. */
export type EntryAxis = "horizontal" | "vertical";

const WORLD_INDEX: Record<WorldAxis, 0 | 1 | 2> = { x: 0, y: 1, z: 2 };

/**
 * What a typed entry asks for, or `null` while it is not yet a number: empty, a bare sign, or
 * malformed text never moves anything. A comma reads as a decimal point.
 */
export function parseMoveEntry(text: string): MoveEntry | null {
	const trimmed = text.trim().replace(",", ".");
	const match = /^([+-])?(\d+(?:\.\d*)?|\.\d+)$/.exec(trimmed);
	if (!match) return null;
	const value = Number(match[2]);
	if (!Number.isFinite(value)) return null;
	if (!match[1]) return { mode: "absolute", metres: value };
	return { mode: "relative", metres: match[1] === "-" ? -value : value };
}

/** Whether a key extends a typed entry. */
export function isEntryKey(key: string): boolean {
	return /^[0-9.,+-]$/.test(key);
}

/** The screen axis a typed value applies to for a drag of `axis`. */
export function entryAxisFor(axis: MoveAxis, chosen: EntryAxis): EntryAxis {
	return axis === "plane" ? chosen : axis;
}

/** The world axis a screen axis shows in this view. */
export function worldAxisOf(
	axis: EntryAxis,
	view: CadViewDirection,
	rotationQuarterTurns: number,
): WorldAxis {
	return viewAxes(view, rotationQuarterTurns)[axis].axis;
}

/**
 * The world origin the gizmo stands on, in millimetres: one element's position, or the centre of
 * the box around a group's positions — the point the typed coordinate places.
 */
export function moveOrigin(
	entities: readonly CadEntity[],
	ids: readonly string[],
): [number, number, number] | null {
	const wanted = new Set(ids);
	const positions = entities
		.filter((entity) => entity.selectable && wanted.has(entity.logicalFixtureId))
		.map((entity) => entity.positionMillimetres);
	if (!positions.length) return null;
	const middle = (index: 0 | 1 | 2) =>
		(Math.min(...positions.map((position) => position[index])) +
			Math.max(...positions.map((position) => position[index]))) /
		2;
	return [middle(0), middle(1), middle(2)];
}

/**
 * The drag delta once a typed entry has set the active world axis. The other axes keep what the
 * pointer moved them by, so a free drag can be dragged on one axis and typed on the other.
 */
export function applyMoveEntry(
	entry: MoveEntry,
	axis: WorldAxis,
	origin: readonly [number, number, number],
	delta: readonly [number, number, number],
): [number, number, number] {
	const index = WORLD_INDEX[axis];
	const next: [number, number, number] = [delta[0], delta[1], delta[2]];
	next[index] =
		entry.mode === "absolute"
			? entry.metres * 1000 - origin[index]
			: entry.metres * 1000;
	return next;
}

/** A world coordinate in millimetres, as the readout shows it. */
export function formatCoordinate(millimetres: number): string {
	const metres = Math.round(millimetres) / 1000;
	// A value that rounds to nothing reads 0.000, never -0.000.
	return `${(metres === 0 ? 0 : metres).toFixed(3)} m`;
}

/** What the readout beside the gizmo shows while a move is in flight. */
export interface MoveReadout {
	/** Where the readout sits, in plan millimetres: the gizmo's own origin. */
	anchor: [number, number];
	coordinates: ReadonlyArray<{
		axis: EntryAxis;
		label: string;
		value: string;
		active: boolean;
	}>;
	/** The text typed so far; empty until the operator types. */
	entry: string;
	/** The typed text is not a number yet, so Enter would not move anything. */
	invalid: boolean;
}

export function moveReadout(
	origin: readonly [number, number, number],
	delta: readonly [number, number, number],
	anchor: [number, number],
	view: CadViewDirection,
	rotationQuarterTurns: number,
	activeAxis: EntryAxis,
	entry: string,
): MoveReadout {
	const axes = viewAxes(view, rotationQuarterTurns);
	return {
		anchor,
		coordinates: (["horizontal", "vertical"] as const).map((axis) => {
			const index = WORLD_INDEX[axes[axis].axis];
			return {
				axis,
				label: axes[axis].axis.toUpperCase(),
				value: formatCoordinate(origin[index] + delta[index]),
				active: axis === activeAxis,
			};
		}),
		entry,
		invalid: entry.length > 0 && parseMoveEntry(entry) === null,
	};
}
