/**
 * Typing into a gizmo move in flight: the readout beside the gizmo, the keys that build a typed
 * coordinate, and the exact move it commits. See `moveEntry.ts` for what a typed value means.
 */
import { type MutableRefObject, useEffect, useState } from "react";
import { rememberMoveAxis } from "./duplicateStep";
import {
	applyMoveEntry,
	entryAxisFor,
	isEntryKey,
	type MoveReadout,
	moveReadout,
	parseMoveEntry,
	worldAxisOf,
} from "./moveEntry";
import { projectPoint } from "./types";
import type { CadViewportContext, Drag } from "./useCadViewportInteraction";

/** The move a valid typed entry asks for, or `null` while nothing valid is typed. */
export function typedDelta(
	context: CadViewportContext,
	active: Drag,
): [number, number, number] | null {
	const entry = parseMoveEntry(active.entry ?? "");
	if (!entry || !active.origin) return null;
	const axis = entryAxisFor(active.axis, active.entryAxis ?? "horizontal");
	return applyMoveEntry(
		entry,
		worldAxisOf(axis, context.view, context.rotationQuarterTurns),
		active.origin,
		active.deltaMillimetres ?? [0, 0, 0],
	);
}

export function useCadMoveEntry({
	drag,
	context,
	cancel,
	endMove,
}: {
	drag: MutableRefObject<Drag | null>;
	context: CadViewportContext;
	/** Abandons the move in flight, leaving the rig where it started. */
	cancel(): void;
	/** Clears what the move drew besides the readout: its axis guide and snap markers. */
	endMove(): void;
}) {
	const [readout, setReadout] = useState<MoveReadout | null>(null);
	/**
	 * Shows the move in flight: a valid typed coordinate wins over where the pointer is on the axis
	 * it names, and the readout beside the gizmo follows whichever the preview shows.
	 */
	function refresh(active: Drag) {
		if (active.type !== "move" || !active.origin) return;
		const typed = typedDelta(context, active);
		const delta = typed ?? active.deltaMillimetres ?? [0, 0, 0];
		if (typed)
			context.onPreview({
				entityIds: active.entityIds ?? context.selectedIds,
				deltaMillimetres: typed,
				spread: false,
			});
		const { view, rotationQuarterTurns } = context;
		const origin = active.origin;
		setReadout(
			moveReadout(
				origin,
				delta,
				projectPoint(
					origin.map((value, index) => value + delta[index]) as [number, number, number],
					view,
					rotationQuarterTurns,
				),
				view,
				rotationQuarterTurns,
				entryAxisFor(active.axis, active.entryAxis ?? "horizontal"),
				active.entry ?? "",
			),
		);
	}
	useEffect(() => {
		// Typing during a move is the move's, not the viewport's: digits and +/- would otherwise
		// switch views and zoom. The capture phase takes them before any shortcut sees them.
		const typing = (event: KeyboardEvent) => {
			const active = drag.current;
			if (active?.type !== "move" || !active.origin) return;
			if (!typeMoveKey(active, event.key)) return;
			event.preventDefault();
			event.stopImmediatePropagation();
		};
		window.addEventListener("keydown", typing, true);
		return () => window.removeEventListener("keydown", typing, true);
	});

	/** Applies one key to the typed entry of a move in flight; false leaves the key alone. */
	function typeMoveKey(active: Drag, key: string): boolean {
		const entry = active.entry ?? "";
		if (isEntryKey(key)) active.entry = entry + key;
		// Backspace during a move is the entry's even when it is empty: it never deletes what moves.
		else if (key === "Backspace") active.entry = entry.slice(0, -1);
		else if (key === "Tab" && active.axis === "plane")
			active.entryAxis = active.entryAxis === "vertical" ? "horizontal" : "vertical";
		else if (key === "Escape") {
			if (!entry) {
				cancel();
				return true;
			}
			active.entry = "";
		} else if (key === "Enter") {
			// Nothing valid typed yet: Enter moves nothing and the drag carries on.
			if (typedDelta(context, active)) void commitTyped(active);
			return true;
		} else return false;
		// A cleared or unfinished entry hands the axis back to the pointer.
		if (!typedDelta(context, active) && active.deltaMillimetres)
			context.onPreview({
				entityIds: active.entityIds ?? context.selectedIds,
				deltaMillimetres: active.deltaMillimetres,
				spread: active.spread ?? false,
			});
		refresh(active);
		return true;
	}

	/** Commits the move a typed coordinate asks for, exactly: it neither snaps nor spreads. */
	async function commitTyped(active: Drag) {
		const delta = typedDelta(context, active);
		if (!delta) return;
		drag.current = null;
		endMove();
		setReadout(null);
		const moved = active.entityIds ?? context.selectedIds;
		context.onPreview({ entityIds: moved, deltaMillimetres: delta, spread: false });
		rememberMoveAxis(moved, delta, context.view, context.rotationQuarterTurns);
		await context.onMove(delta, moved, false, false);
	}

	return { readout, refresh, commitTyped, clearReadout: () => setReadout(null) };
}
