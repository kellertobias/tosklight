import { useLayoutEffect, useState } from "react";
import { soloFencePaths } from "./soloFencePaths";
import type { PlaybackSoloRegion } from "../../../features/playbackSolo/contracts";
import { soloFenceSegments, type SoloFenceCell } from "./soloFence";

export function SoloFenceOverlay({
	bank,
	page,
	regions,
	layoutKey,
}: {
	bank: HTMLDivElement | null;
	page: number | null;
	regions: readonly PlaybackSoloRegion[];
	layoutKey: unknown;
}) {
	const [drawing, setDrawing] = useState<{
		width: number;
		height: number;
		fences: { regionId: string; color: string; paths: string[] }[];
	}>({ width: 0, height: 0, fences: [] });
	useLayoutEffect(() => {
		const grid = bank;
		if (!grid || page == null) return;
		const measure = () => {
			const bounds = grid.getBoundingClientRect();
			const style = window.getComputedStyle(grid);
			const halfX = (Number.parseFloat(style.columnGap) || 0) / 2;
			const halfY = (Number.parseFloat(style.rowGap) || 0) / 2;
			const cards = grid.querySelectorAll<HTMLElement>(
				"article[data-playback-row][data-playback-slot]",
			);
			const cells: SoloFenceCell[] = [...cards].map((card, index) => {
				const rect = card.getBoundingClientRect();
				return {
					key: String(index),
					position: Number(card.dataset.playbackSlot),
					rect: {
						left: Math.max(1.5, rect.left - bounds.left - halfX),
						top: Math.max(1.5, rect.top - bounds.top - halfY),
						right: Math.min(
							bounds.width - 1.5,
							rect.right - bounds.left + halfX,
						),
						bottom: Math.min(
							bounds.height - 1.5,
							rect.bottom - bounds.top + halfY,
						),
					},
				};
			});
			setDrawing({
				width: bounds.width,
				height: bounds.height,
				fences: regions.map((region) => ({
					regionId: region.id,
					color: region.color ?? "var(--amber)",
					paths: soloFencePaths(soloFenceSegments(cells, page, [region])),
				})),
			});
		};
		measure();
		const observer =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(measure);
		observer?.observe(grid);
		for (const card of grid.querySelectorAll("article[data-playback-slot]"))
			observer?.observe(card);
		window.addEventListener("resize", measure);
		return () => {
			observer?.disconnect();
			window.removeEventListener("resize", measure);
		};
	}, [bank, page, regions, layoutKey]);
	if (page == null || !drawing.width || !drawing.height) return null;
	return (
		<svg
			className="playback-solo-fence-overlay"
			aria-hidden="true"
			focusable="false"
			viewBox={`0 0 ${drawing.width} ${drawing.height}`}
			preserveAspectRatio="none"
			style={{ pointerEvents: "none" }}
		>
			{drawing.fences.flatMap((fence) =>
				fence.paths.map((path, index) => (
					<path
						key={`${fence.regionId}:${index}`}
						d={path}
						data-solo-region={fence.regionId}
						stroke={fence.color}
					/>
				)),
			)}
		</svg>
	);
}
