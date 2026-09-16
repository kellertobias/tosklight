/**
 * Keeps a viewport's top-left corner on the same plan point when the viewport changes size.
 *
 * The camera is centred — its pan is measured from the middle of the canvas — so opening, closing
 * or resizing a sidebar moved the whole plan by half the change. The pan is corrected by that half
 * as the size changes, before the browser paints, so the corner stays put and the zoom is untouched.
 */
import { type RefObject, useLayoutEffect, useRef } from "react";
import { flushSync } from "react-dom";
import type { TileCamera } from "./types";
import { useLiveCamera } from "./useLiveCamera";

export interface ViewportSize {
	width: number;
	height: number;
}

/** The camera that shows the same plan point at the top-left corner after a resize. */
export function anchorTopLeft(
	camera: TileCamera,
	from: ViewportSize,
	to: ViewportSize,
): TileCamera {
	const dx = to.width - from.width;
	const dy = to.height - from.height;
	if (dx === 0 && dy === 0) return camera;
	return {
		...camera,
		pan: [
			camera.pan[0] - dx / (2 * camera.zoom),
			camera.pan[1] + dy / (2 * camera.zoom),
		],
	};
}

export function useTopLeftAnchor(
	element: RefObject<Element | null>,
	latest: () => TileCamera,
	settle: (camera: TileCamera) => void,
) {
	const handlers = useRef({ latest, settle });
	useLayoutEffect(() => {
		handlers.current = { latest, settle };
	});
	useLayoutEffect(() => {
		const target = element.current;
		if (!target || typeof ResizeObserver === "undefined") return;
		let size: ViewportSize | null = null;
		const observer = new ResizeObserver(() => {
			const next = { width: target.clientWidth, height: target.clientHeight };
			const previous = size;
			size = next;
			// A hidden or not-yet-laid-out viewport has no corner to keep.
			if (!previous || !previous.width || !previous.height || !next.width || !next.height)
				return;
			const camera = handlers.current.latest();
			const anchored = anchorTopLeft(camera, previous, next);
			if (anchored === camera) return;
			flushSync(() => handlers.current.settle(anchored));
		});
		observer.observe(target);
		return () => observer.disconnect();
	}, [element]);
}

/** The viewport's live camera, kept on its top-left corner as the viewport changes size. */
export function useAnchoredCamera(
	element: RefObject<Element | null>,
	camera: TileCamera,
	onCamera: (camera: TileCamera) => void,
) {
	const live = useLiveCamera(camera, onCamera);
	useTopLeftAnchor(element, live.latest, live.settle);
	return live;
}
