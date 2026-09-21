/**
 * Whether the CAD object menu is open, and where.
 *
 * A right-click in a viewport opens it through `open`. From the keyboard, the Menu key or Shift+F10
 * opens it over the middle of the viewport last used, for whatever is selected, as long as Select
 * is in hand and nothing else has the keys.
 */
import { useCallback, useEffect, useState } from "react";
import { duplicateStep } from "./duplicateStep";
import { type CadObjectMenuRequest, isMenuKey } from "./CadObjectMenu";
import type { CadEntity, CadViewDirection } from "./types";

/** Keys typed into a field, a dialog or a menu belong to it, as with the CAD shortcuts. */
function keysAreTaken(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return false;
	return Boolean(
		target.closest("input, textarea, select, [contenteditable=true], [role=dialog], [role=menu]"),
	);
}

export function useCadObjectMenu({
	enabled,
	selectedIds,
	entities,
	activeView,
}: {
	/** Select is in hand and the print pages are closed. */
	enabled: boolean;
	/** What the keyboard's menu acts on: the current selection. */
	selectedIds: readonly string[];
	/** The drawing, so a copy's step can be measured from what it copies. */
	entities: readonly CadEntity[];
	/** The view the keyboard's menu duplicates along: the viewport last used. */
	activeView: { view: CadViewDirection; rotationQuarterTurns: number };
}) {
	const [request, setRequest] = useState<CadObjectMenuRequest | null>(null);
	const close = useCallback(() => setRequest(null), []);
	const { view, rotationQuarterTurns } = activeView;

	useEffect(() => {
		if (!enabled) setRequest(null);
	}, [enabled]);

	useEffect(() => {
		const keyDown = (event: KeyboardEvent) => {
			if (!enabled || !selectedIds.length || !isMenuKey(event) || keysAreTaken(event.target))
				return;
			event.preventDefault();
			const tile =
				document.querySelector(".cad-tile.is-active") ?? document.querySelector(".cad-tile");
			const box = tile?.getBoundingClientRect();
			setRequest({
				x: box ? Math.round(box.left + box.width / 2) : Math.round(window.innerWidth / 2),
				y: box ? Math.round(box.top + box.height / 2) : Math.round(window.innerHeight / 2),
				duplicateOffset: duplicateStep(entities, selectedIds, view, rotationQuarterTurns),
				entityIds: selectedIds,
			});
		};
		window.addEventListener("keydown", keyDown);
		return () => window.removeEventListener("keydown", keyDown);
	}, [enabled, selectedIds, entities, view, rotationQuarterTurns]);

	return { request: enabled ? request : null, open: setRequest, close };
}
