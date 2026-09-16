/**
 * The menu a right-click on a selected element opens in a CAD viewport: **Duplicate** and **Delete**.
 *
 * Every element — lamp, truss, stage part, imported model — offers the same two actions; nothing
 * about a lamp needs an action of its own here. The menu opens only while Select is in hand; with
 * a drawing tool a right-click finishes the line instead. The keyboard reaches it too: the Menu key
 * or Shift+F10 opens it for the selection, the arrow keys move between its entries, Enter or Space
 * runs one, and Escape closes it. A press anywhere outside it closes it as well.
 */
import { useEffect, useRef } from "react";
import "./cadObjectMenu.css";

/** The menu's room at the window's edge, in pixels. */
const MENU_WIDTH = 180;
const MENU_HEIGHT = 96;

/** Where the menu opens, in window pixels, and the step a copy stands from its original there. */
export interface CadObjectMenuRequest {
	x: number;
	y: number;
	/** Show millimetres: the view's right, `DUPLICATE_OFFSET_MILLIMETRES` long. */
	duplicateOffset: [number, number, number];
}

/** Whether a key press asks for the context menu: the Menu key, or Shift+F10. */
export function isMenuKey(event: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "metaKey" | "altKey">) {
	if (event.ctrlKey || event.metaKey || event.altKey) return false;
	return event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
}

export function CadObjectMenu({
	request,
	count,
	onDuplicate,
	onDelete,
	onClose,
}: {
	request: CadObjectMenuRequest;
	/** How many elements the actions apply to. */
	count: number;
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
	const style = {
		left: `${Math.max(0, Math.min(request.x, window.innerWidth - MENU_WIDTH))}px`,
		top: `${Math.max(0, Math.min(request.y, window.innerHeight - MENU_HEIGHT))}px`,
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
