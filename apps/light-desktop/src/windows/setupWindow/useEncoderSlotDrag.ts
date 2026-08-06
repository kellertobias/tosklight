import {
	type PointerEvent as ReactPointerEvent,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import type { EncoderSlotTarget } from "./encoderLayoutModel";

/** Pointer travel that separates a tap on a slot's own controls from a drag. */
const DRAG_THRESHOLD = 4;

export interface EncoderDragState {
	attribute: string;
	label: string;
	/** Null until the pointer has travelled far enough to be a drag. */
	target: EncoderSlotTarget | null;
	active: boolean;
	x: number;
	y: number;
}

/**
 * Drags an encoder with pointer events rather than HTML5 drag-and-drop: the desk surface is
 * touch-first, and a WebView drag never starts without a dataTransfer payload. The caller
 * renders the layout from the previewed target so the tiles visibly move under the pointer.
 */
export function useEncoderSlotDrag({
	onCommit,
}: {
	onCommit(attribute: string, target: EncoderSlotTarget): void;
}) {
	const [drag, setDrag] = useState<EncoderDragState | null>(null);
	const origin = useRef<{ x: number; y: number } | null>(null);
	/**
	 * The authoritative drag: pointer moves are continuous events, so React may not have
	 * re-rendered the hovered slot by the time the pointer is released.
	 */
	const latest = useRef<EncoderDragState | null>(null);
	const publish = useCallback((next: EncoderDragState | null) => {
		latest.current = next;
		setDrag(next);
	}, []);

	const cancel = useCallback(() => {
		origin.current = null;
		publish(null);
	}, [publish]);

	const begin = useCallback(
		(event: ReactPointerEvent, attribute: string, label: string) => {
			if (event.button !== 0 && event.pointerType === "mouse") return;
			if (isInteractive(event.target)) return;
			origin.current = { x: event.clientX, y: event.clientY };
			publish({
				attribute,
				label,
				target: null,
				active: false,
				x: event.clientX,
				y: event.clientY,
			});
		},
		[publish],
	);

	/**
	 * A slot reports itself while the pointer is over it. This is recorded even before the
	 * drag turns active, because the slot sees the activating move before the window does.
	 */
	const hover = useCallback(
		(target: EncoderSlotTarget) => {
			const current = latest.current;
			if (!current || sameSlot(current.target, target)) return;
			publish({ ...current, target });
		},
		[publish],
	);

	useEffect(() => {
		if (!drag) return;
		const move = (event: PointerEvent) => {
			const start = origin.current;
			const current = latest.current;
			if (!start || !current) return;
			const travelled =
				Math.abs(event.clientX - start.x) + Math.abs(event.clientY - start.y);
			publish({
				...current,
				x: event.clientX,
				y: event.clientY,
				active: current.active || travelled > DRAG_THRESHOLD,
			});
			if (travelled > DRAG_THRESHOLD) event.preventDefault();
		};
		const finish = () => {
			const current = latest.current;
			if (current?.active && current.target)
				onCommit(current.attribute, current.target);
			cancel();
		};
		window.addEventListener("pointermove", move, { passive: false });
		window.addEventListener("pointerup", finish);
		window.addEventListener("pointercancel", cancel);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", finish);
			window.removeEventListener("pointercancel", cancel);
		};
	}, [drag, onCommit, cancel, publish]);

	return { drag, begin, hover, cancel };
}

function sameSlot(left: EncoderSlotTarget | null, right: EncoderSlotTarget) {
	return (
		left?.group === right.group &&
		left?.page === right.page &&
		left?.slot === right.slot
	);
}

/** The slot's own buttons, selects, and disclosure controls keep their normal behavior. */
function isInteractive(target: EventTarget | null) {
	return (
		target instanceof Element &&
		Boolean(target.closest("button, select, input, textarea, summary, [role='option'], .ui-select"))
	);
}
