/**
 * The CAD screen's single-key shortcuts: the drawing tools, the five view directions, zoom and pan.
 *
 * A key reaches the drawing only when nothing else wants it: never while a field, a select or an
 * editable text has focus, never while a dialog is open, and never with Ctrl, Cmd or Alt held, so
 * the window's own shortcuts keep working. The keys act on the viewport the operator last used.
 */
import { useEffect, useRef } from "react";
import type { CadDrawTool } from "./cadTools";
import { CAD_VIEW_LABELS, type CadViewDirection, type TileCamera } from "./types";

export type CadShortcut =
	| { type: "tool"; tool: CadDrawTool }
	| { type: "view"; view: CadViewDirection }
	| { type: "zoom"; factor: number }
	| { type: "pan"; horizontal: -1 | 0 | 1; vertical: -1 | 0 | 1 };

export const CAD_MIN_ZOOM = 0.004;
export const CAD_MAX_ZOOM = 2.5;
/** One press of + or −. */
export const CAD_ZOOM_STEP = 1.25;
/** How far one press of W, A, S or D moves the view, in screen pixels. */
export const CAD_PAN_STEP_PIXELS = 80;

const TOOL_KEYS: Readonly<Record<string, CadDrawTool>> = {
	v: "select",
	l: "polyline",
	p: "box",
	t: "text",
	m: "measure",
	r: "erase",
};

const PAN_KEYS: Readonly<Record<string, [-1 | 0 | 1, -1 | 0 | 1]>> = {
	w: [0, 1],
	a: [-1, 0],
	s: [0, -1],
	d: [1, 0],
};

/** The views in the order the view menu lists them, so 1 is its first entry and 5 its last. */
const VIEW_ORDER = Object.keys(CAD_VIEW_LABELS) as CadViewDirection[];

export function cadShortcutFor(
	event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey">,
): CadShortcut | null {
	if (event.ctrlKey || event.metaKey || event.altKey) return null;
	const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
	const tool = TOOL_KEYS[key];
	if (tool) return { type: "tool", tool };
	const pan = PAN_KEYS[key];
	if (pan) return { type: "pan", horizontal: pan[0], vertical: pan[1] };
	if (/^[1-9]$/u.test(key)) {
		const view = VIEW_ORDER[Number(key) - 1];
		return view ? { type: "view", view } : null;
	}
	// = is + without Shift on many layouts; _ is − with it.
	if (key === "+" || key === "=") return { type: "zoom", factor: CAD_ZOOM_STEP };
	if (key === "-" || key === "_") return { type: "zoom", factor: 1 / CAD_ZOOM_STEP };
	return null;
}

export function clampZoom(zoom: number) {
	return Math.min(CAD_MAX_ZOOM, Math.max(CAD_MIN_ZOOM, zoom));
}

/** The camera zoomed about the centre of the viewport. */
export function zoomedCamera(camera: TileCamera, factor: number): TileCamera {
	return { ...camera, zoom: clampZoom(camera.zoom * factor) };
}

/**
 * The camera moved one step. The plan's centre is at minus the pan, so looking further right or up
 * lowers the pan; the step is in screen pixels, so it feels the same at every zoom.
 */
export function pannedCamera(
	camera: TileCamera,
	horizontal: -1 | 0 | 1,
	vertical: -1 | 0 | 1,
): TileCamera {
	const step = CAD_PAN_STEP_PIXELS / camera.zoom;
	return {
		...camera,
		pan: [camera.pan[0] - horizontal * step, camera.pan[1] - vertical * step],
	};
}

function wantsTheKey(target: EventTarget | null) {
	const element = target as Element | null;
	return Boolean(
		element?.closest?.("input, textarea, select, [contenteditable=''], [contenteditable='true']"),
	);
}

/** Calls `onShortcut` for every CAD shortcut pressed while this screen is mounted. */
export function useCadShortcuts(onShortcut: (shortcut: CadShortcut) => void) {
	const latest = useRef(onShortcut);
	latest.current = onShortcut;
	useEffect(() => {
		const key = (event: KeyboardEvent) => {
			if (event.defaultPrevented || wantsTheKey(event.target)) return;
			if (document.querySelector("[data-modal-top='true'], [aria-modal='true']")) return;
			const shortcut = cadShortcutFor(event);
			if (!shortcut) return;
			event.preventDefault();
			latest.current(shortcut);
		};
		window.addEventListener("keydown", key);
		return () => window.removeEventListener("keydown", key);
	}, []);
}
