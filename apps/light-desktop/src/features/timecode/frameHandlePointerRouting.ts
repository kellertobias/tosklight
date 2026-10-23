import type { PointerEvent as ReactPointerEvent } from "react";
const starters = new WeakMap<HTMLElement, (event: ReactPointerEvent) => void>();
export function registerFramePointer(
	node: HTMLElement | null,
	begin: (event: ReactPointerEvent) => void,
) {
	if (node) starters.set(node, begin);
}
/** Touch rectangles may overlap; the nearest visible boundary owns the entire gesture. */
export function routeFramePointer(
	event: ReactPointerEvent,
	actual: HTMLElement,
) {
	let target = actual;
	if (actual.classList.contains("cue-divider-grip")) {
		const scope =
			actual.closest(".timecode-timeline-item.item-clip") ??
			actual.closest(".timecode-cue-clip-contents") ??
			actual.closest(".timecode-cue-divider-handles");
		const candidates = [
			...(scope?.querySelectorAll<HTMLElement>(".cue-divider-grip") ?? []),
		]
			.map((node) => {
				const box = node.getBoundingClientRect();
				return {
					node,
					box,
					x:
						box.left +
						22 +
						Number(node.dataset.timeAnchorX) -
						Number.parseFloat(node.style.left),
				};
			})
			.filter(
				({ box }) =>
					box.width > 0 &&
					event.clientX >= box.left &&
					event.clientX <= box.right &&
					event.clientY >= box.top &&
					event.clientY <= box.bottom,
			)
			.sort(
				(a, b) => Math.abs(a.x - event.clientX) - Math.abs(b.x - event.clientX),
			);
		target = candidates[0]?.node ?? actual;
		event.preventDefault();
		event.stopPropagation();
	}
	starters.get(target)?.(event);
}
