import type { PointerEvent as ReactPointerEvent } from "react";
import type {
	CueClipTimingRow,
	CueFadeKind,
	CueFadeEdge,
} from "./cueClipTiming";

/** Resolve overlapping touch regions by the nearest true envelope vertex. */
export function redirectCueVertexPointer(
	event: ReactPointerEvent,
	selected: CueClipTimingRow,
	begin: (
		event: ReactPointerEvent,
		row: CueClipTimingRow,
		kind: CueFadeKind,
		edge: CueFadeEdge,
	) => void,
) {
	const hit = (event.target as Element).closest<HTMLElement>(
		".anchored-vertex",
	);
	if (
		!hit ||
		hit.classList.contains("displaced") ||
		hit.getAttribute("aria-disabled") === "true"
	)
		return;
	// Overlapping ordinary 44px targets dispatch to the nearest actual corner,
	// rather than letting DOM paint order steal a neighbouring vertex.
	const points = [
		...event.currentTarget.querySelectorAll<HTMLElement>(".anchored-vertex"),
	]
		.filter((node) => node.getAttribute("aria-disabled") !== "true")
		.map((node) => {
			const box = node
				.querySelector(".timecode-cue-point-anchor")!
				.getBoundingClientRect();
			return {
				node,
				box,
				distance: Math.hypot(
					event.clientX - box.left - box.width / 2,
					event.clientY - box.top - box.height / 2,
				),
			};
		})
		.filter((point) => point.box.width > 0)
		.sort((a, b) => a.distance - b.distance);
	const nearest = points[0]?.node;
	if (!nearest || nearest === hit) return;
	nearest.focus({ preventScroll: true });
	begin(
		event,
		selected,
		nearest.classList.contains("in") ? "in" : "out",
		nearest.classList.contains("start") ? "start" : "end",
	);
}
