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
/** Logical points of travel past which a press is aiming the camera rather than selecting. */
const SELECT_SLOP = 4;

/** Metres the camera walks per key press. Shift takes longer strides. */
const FLY_STEP = 0.35;

export function NativeStageSurface({
	pane,
	interactive = true,
	plan = false,
}: {
	pane: NativeStagePane;
	interactive?: boolean;
	/** A plan view, which slides rather than orbits. */
	plan?: boolean;
}) {
	const dragging = useRef<{ x: number; y: number; button: number } | null>(null);
	/** How far the pointer travelled, so a drag to aim the camera is not read as a click to select. */
	const travelled = useRef(0);
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
				travelled.current = 0;
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				const drag = dragging.current;
				if (!capturing || !drag) return;
				const dx = event.clientX - drag.x;
				const dy = event.clientY - drag.y;
				if (dx === 0 && dy === 0) return;
				travelled.current += Math.abs(dx) + Math.abs(dy);
				dragging.current = { ...drag, x: event.clientX, y: event.clientY };
				/*
				 * In a 3D view the primary button orbits, the middle walks the camera across its own
				 * axes and leaves what it looks at alone, and the secondary slides both together so
				 * the picture translates without turning.
				 *
				 * A plan has nothing to orbit, so the primary button slides it instead. Leaving
				 * orbit on the primary there meant the button an operator reaches for first did
				 * nothing at all, on the one view where sliding is the only thing to do.
				 */
				const gesture = plan
					? "pan"
					: drag.button === 1
						? "truck"
						: drag.button === 2
							? "pan"
							: "orbit";
				pane.send(gesture, dx, dy);
			}}
			onPointerUp={(event) => {
				const drag = dragging.current;
				dragging.current = null;
				/*
				 * A primary press that barely moved is a click on a fixture, not an attempt to aim
				 * the camera. The threshold is what separates them: an operator picking a fixture
				 * on a touch surface never holds perfectly still, and one orbiting never means to
				 * select what they started on.
				 */
				if (capturing && drag?.button === 0 && travelled.current < SELECT_SLOP) {
					const rect = event.currentTarget.getBoundingClientRect();
					if (rect.width > 0 && rect.height > 0) {
						pane.send(
							event.shiftKey || event.metaKey || event.ctrlKey
								? "pick-add"
								: "pick",
							(event.clientX - rect.left) / rect.width,
							(event.clientY - rect.top) / rect.height,
						);
					}
				}
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
			/*
			 * WASD walks the camera, and only while the pane itself has focus.
			 *
			 * A lighting desk's keyboard belongs to the command line — a console where W silently
			 * moved the camera while an operator was typing would be unusable — so these keys are
			 * the pane's only once an operator has clicked into it, and they are not allowed to
			 * travel any further.
			 */
			tabIndex={0}
			onKeyDown={(event) => {
				if (!capturing || event.metaKey || event.ctrlKey || event.altKey) return;
				const step = event.shiftKey ? FLY_STEP * 4 : FLY_STEP;
				const walk: Record<string, [number, number]> = {
					w: [step, 0],
					s: [-step, 0],
					a: [0, -step],
					d: [0, step],
				};
				const move = walk[event.key.toLowerCase()];
				if (!move) return;
				event.preventDefault();
				event.stopPropagation();
				pane.send("fly", move[1], move[0]);
			}}
			onContextMenu={(event) => event.preventDefault()}
		/>
	);
}
