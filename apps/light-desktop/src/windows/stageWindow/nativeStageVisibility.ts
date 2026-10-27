import type { StagePaneGeometry } from "../../platform/desktop/types";

/** Zero-sized/collapsed panes retain their last positive drawable instead of allocating 1px. */
export function measureNativeStage(element: HTMLElement): StagePaneGeometry | null {
	const rect = element.getBoundingClientRect();
	const scale = window.devicePixelRatio || 1;
	if (![rect.left, rect.top, rect.width, rect.height, scale, window.innerWidth, window.innerHeight].every(Number.isFinite) ||
		rect.width <= 0 || rect.height <= 0 || scale <= 0 || window.innerWidth <= 0 || window.innerHeight <= 0) return null;
	return {
		x: rect.left, y: rect.top, width: rect.width, height: rect.height, scale,
		surfaceWidth: Math.max(1, Math.round(window.innerWidth * scale)),
		surfaceHeight: Math.max(1, Math.round(window.innerHeight * scale)),
	};
}

/** Visibility is about a drawable surface, never which window owns keyboard focus. */
export function observeNativeStageVisibility(
	element: HTMLElement,
	enabled: () => boolean,
	changed: (geometry: StagePaneGeometry | null, visible: boolean) => void,
) {
	let intersecting = true;
	let disposed = false;
	const report = () => {
		if (disposed) return;
		const geometry = measureNativeStage(element);
		const style = window.getComputedStyle(element);
		const visible = enabled() && document.visibilityState === "visible" && intersecting &&
			style.visibility !== "hidden" && style.visibility !== "collapse" && Boolean(geometry &&
				geometry.x < window.innerWidth && geometry.y < window.innerHeight &&
				geometry.x + geometry.width > 0 && geometry.y + geometry.height > 0);
		changed(geometry, visible);
	};
	const resize = new ResizeObserver(report);
	resize.observe(element);
	const intersection = typeof IntersectionObserver === "undefined" ? null : new IntersectionObserver((entries) => {
		for (const entry of entries) if (entry.target === element) intersecting = entry.isIntersecting && entry.intersectionRatio > 0;
		report();
	});
	intersection?.observe(element);
	document.addEventListener("visibilitychange", report);
	window.addEventListener("resize", report);
	window.addEventListener("scroll", report, true);
	report();
	return {
		report,
		dispose: () => {
			disposed = true;
			resize.disconnect(); intersection?.disconnect();
			document.removeEventListener("visibilitychange", report);
			window.removeEventListener("resize", report);
			window.removeEventListener("scroll", report, true);
		},
	};
}
