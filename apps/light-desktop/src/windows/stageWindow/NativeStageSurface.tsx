import { useRef } from "react";
import type { NativeStagePane } from "./useNativeStagePane";

/**
 * The hole in the interface where the native renderer draws.
 *
 * It paints nothing. The desk's window is transparent wherever the interface paints nothing, and
 * the picture underneath shows through — which is what puts the menus, dialogs and sheets above the
 * Stage rather than under it.
 *
 * Input is the part that cannot work by itself. A `WKWebView` on top of a native surface wins
 * AppKit hit-testing whatever CSS says, so pointer events never reach the surface below and
 * `pointer-events: none` does not change that: it is a web-layer concept and the native view is
 * still the hit-test winner. So the gestures are captured here, in the web layer where they do
 * arrive, and forwarded as intent — one message per gesture step rather than one per pointermove,
 * because every one of them is an IPC round trip.
 */
export function NativeStageSurface({
	pane,
	interactive = true,
}: {
	pane: NativeStagePane;
	interactive?: boolean;
}) {
	const dragging = useRef<{ x: number; y: number; button: number } | null>(null);
	// While the renderer is still starting, the desk's own canvas is drawing underneath and this
	// is only here to be measured. Taking its pointer events would make the Stage briefly dead.
	const capturing = interactive && pane.active;

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: the pane is a camera surface, and its
		// selectable contents are reached through the Stage's own controls rather than through this
		// element, which has nothing in the accessibility tree to focus.
		<div
			className="stage-native-pane"
			ref={pane.ref}
			data-native-stage={pane.active ? "drawing" : "waiting"}
			style={{ pointerEvents: capturing ? "auto" : "none" }}
			onPointerDown={(event) => {
				if (!capturing) return;
				dragging.current = {
					x: event.clientX,
					y: event.clientY,
					button: event.button,
				};
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				const drag = dragging.current;
				if (!capturing || !drag) return;
				const dx = event.clientX - drag.x;
				const dy = event.clientY - drag.y;
				if (dx === 0 && dy === 0) return;
				dragging.current = { ...drag, x: event.clientX, y: event.clientY };
				/*
				 * The primary button orbits, which is what the desk's own 3D Stage does, so an
				 * operator moving between the two renderers does not have to learn the pane twice.
				 * The middle and secondary buttons are what the pane adds: the middle walks the
				 * camera across its own axes and leaves what it looks at alone, the secondary
				 * slides both together so the picture translates without turning.
				 */
				const gesture =
					drag.button === 1 ? "truck" : drag.button === 2 ? "pan" : "orbit";
				pane.send(gesture, dx, dy);
			}}
			onPointerUp={(event) => {
				dragging.current = null;
				if (event.currentTarget.hasPointerCapture(event.pointerId))
					event.currentTarget.releasePointerCapture(event.pointerId);
			}}
			onPointerCancel={() => {
				dragging.current = null;
			}}
			onWheel={(event) => {
				if (!capturing) return;
				// Normalized to notches: a trackpad reports pixels and a mouse reports lines, and
				// the renderer should not have to know which one the operator has.
				const notches = event.deltaMode === 0 ? event.deltaY / 100 : event.deltaY;
				pane.send("zoom", 0, -notches);
			}}
			onContextMenu={(event) => event.preventDefault()}
		/>
	);
}
