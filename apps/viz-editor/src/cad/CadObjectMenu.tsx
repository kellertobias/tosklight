/**
 * The menu a right-click on a selected element opens in a CAD viewport: **Group**, **Ungroup**,
 * **Duplicate** and **Delete**.
 *
 * Every element — lamp, truss, stage part, imported model — offers Duplicate and Delete; nothing
 * about a lamp needs an action of its own here. Group and Ungroup stand above them and show only
 * when they would do something: Group with two Venue elements that are not already exactly one
 * group, Ungroup with a grouped element among the selection. They are the same actions as ⌘G and
 * ⇧⌘G and as the buttons in Elements › Objects. The menu opens only while Select is in hand; with
 * a drawing tool a right-click finishes the line instead. The keyboard reaches it too: the Menu key
 * or Shift+F10 opens it for the selection, the arrow keys move between its entries, Enter or Space
 * runs one, and Escape closes it. A press anywhere outside it closes it as well.
 */
import { useEffect, useRef } from "react";
import "./cadObjectMenu.css";

/** The menu's room at the window's edge, in pixels: its width, and one entry's height. */
const MENU_WIDTH = 180;
const MENU_ITEM_HEIGHT = 40;
const MENU_PADDING = 8;

/** Where the menu opens, in window pixels, and the step a copy stands from its original there. */
export interface CadObjectMenuRequest {
	x: number;
	y: number;
	/** Show millimetres: the step a copy stands from its original, from `duplicateStep`. */
	duplicateOffset: [number, number, number];
	/**
	 * The logical fixtures the menu's actions apply to, known when it opens.
	 *
	 * A right-click also makes them the selection, but that round-trips through the desk. The menu
	 * carries them so it can paint in the same frame as the click rather than waiting for the
	 * selection to come back.
	 */
	entityIds: readonly string[];
}

/** Whether a key press asks for the context menu: the Menu key, or Shift+F10. */
export function isMenuKey(event: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "metaKey" | "altKey">) {
	if (event.ctrlKey || event.metaKey || event.altKey) return false;
	return event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
}

export function CadObjectMenu({
	request,
	count,
	onGroup,
	onUngroup,
	onDuplicate,
	onDelete,
	onClose,
}: {
	request: CadObjectMenuRequest;
	/** How many elements the actions apply to. */
	count: number;
	/** Group the selection, or null when grouping it would do nothing; then the entry stays away. */
	onGroup?: (() => void) | null;
	/** Ungroup the selection, or null when none of it is grouped. */
	onUngroup?: (() => void) | null;
	onDuplicate(): void;
	onDelete(): void;
	onClose(): void;
}) {
	const menu = useRef<HTMLDivElement>(null);

	useEffect(() => {
		menu.current?.querySelector<HTMLButtonElement>("[role=menuitem]")?.focus();
		const closeOutside = (event: PointerEvent) => {
			if (!menu.current?.contains(event.target as Node)) onClose();
		};
		const closeOnBlur = () => onClose();
		window.addEventListener("pointerdown", closeOutside, true);
		window.addEventListener("blur", closeOnBlur);
		return () => {
			window.removeEventListener("pointerdown", closeOutside, true);
			window.removeEventListener("blur", closeOnBlur);
		};
	}, [onClose]);

	function keyDown(event: React.KeyboardEvent<HTMLDivElement>) {
		// The CAD shortcuts listen on the window; a key used here is not also a tool or a view.
		event.stopPropagation();
		const items = [...(menu.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]") ?? [])];
		const at = items.indexOf(document.activeElement as HTMLButtonElement);
		const step = (by: number) => {
			event.preventDefault();
			items[(at + by + items.length) % items.length]?.focus();
		};
		if (event.key === "ArrowDown") step(1);
		else if (event.key === "ArrowUp") step(-1);
		else if (event.key === "Home") step(-at);
		else if (event.key === "End") step(items.length - 1 - at);
		else if (event.key === "Escape" || event.key === "Tab") {
			event.preventDefault();
			onClose();
		}
	}

	const run = (action: () => void) => () => {
		onClose();
		action();
	};
	const noun = count > 1 ? `${count} elements` : "element";
	// Kept inside the window, so a click near the right or bottom edge still shows every entry.
	const height = MENU_PADDING + (2 + (onGroup ? 1 : 0) + (onUngroup ? 1 : 0)) * MENU_ITEM_HEIGHT;
	const style = {
		left: `${Math.max(0, Math.min(request.x, window.innerWidth - MENU_WIDTH))}px`,
		top: `${Math.max(0, Math.min(request.y, window.innerHeight - height))}px`,
	};

	return (
		<div
			ref={menu}
			className="cad-object-menu"
			role="menu"
			aria-label={`Actions for the selected ${noun}`}
			style={style}
			onKeyDown={keyDown}
			onContextMenu={(event) => event.preventDefault()}
		>
			{onGroup ? (
				<button type="button" role="menuitem" className="cad-object-menu-item" onClick={run(onGroup)}>
					Group
				</button>
			) : null}
			{onUngroup ? (
				<button type="button" role="menuitem" className="cad-object-menu-item" onClick={run(onUngroup)}>
					Ungroup
				</button>
			) : null}
			<button type="button" role="menuitem" className="cad-object-menu-item" onClick={run(onDuplicate)}>
				Duplicate
			</button>
			<button
				type="button"
				role="menuitem"
				className="cad-object-menu-item is-danger"
				onClick={run(onDelete)}
			>
				Delete
			</button>
		</div>
	);
}
