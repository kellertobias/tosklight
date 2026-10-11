import { useLayoutEffect, useState } from "react";
import type { PlaybackSoloRegion } from "../../../features/playbackSolo/contracts";
import {
	soloFenceSegments,
	type SoloFenceCell,
	type SoloFenceSegment,
} from "./soloFence";

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
		segments: SoloFenceSegment[];
	}>({ width: 0, height: 0, segments: [] });
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
				segments: soloFenceSegments(cells, page, regions),
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
			{drawing.segments.map((line, index) => (
				<line
					key={`${line.cellKey}:${line.edge}:${index}`}
					x1={line.x1}
					y1={line.y1}
					x2={line.x2}
					y2={line.y2}
				/>
			))}
		</svg>
	);
}
