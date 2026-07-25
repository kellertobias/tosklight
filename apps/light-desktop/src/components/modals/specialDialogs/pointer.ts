import type { PointerEvent, RefObject } from "react";

export interface NormalizedPointerPosition {
	x: number;
	y: number;
}

export function normalizedPointerPosition(
	event: PointerEvent<HTMLDivElement>,
	ref: RefObject<HTMLDivElement | null>,
): NormalizedPointerPosition {
	if (!ref.current) throw new Error("Pointer surface is unavailable");
	const box = ref.current.getBoundingClientRect();
	return {
		x: Math.max(0, Math.min(1, (event.clientX - box.left) / box.width)),
		y: Math.max(0, Math.min(1, (event.clientY - box.top) / box.height)),
	};
}
