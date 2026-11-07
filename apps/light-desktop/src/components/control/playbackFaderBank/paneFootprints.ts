import { playbackCellKey } from "./projection";
import type { PlaybackPaneShape, PlaybackSlotProjection } from "./types";

function paneShape(cell: PlaybackSlotProjection) {
	const type = cell.playback?.footprint?.type;
	return type === "single_button" ||
		type === "pane_normal" ||
		type === "pane_taller" ||
		type === "pane_wider"
		? type
		: null;
}

function paneClaim(cell: PlaybackSlotProjection, type: PlaybackPaneShape) {
	const firstRow = cell.rowIndex - (type === "pane_taller" ? 1 : 0);
	const rowSpan = type === "pane_taller" ? 4 : 3;
	const columnSpan = type === "pane_wider" ? 2 : 1;
	const neighbors: string[] = [];
	for (let rowIndex = firstRow; rowIndex < firstRow + rowSpan; rowIndex++) {
		for (
			let columnIndex = cell.columnIndex;
			columnIndex < cell.columnIndex + columnSpan;
			columnIndex++
		) {
			if (rowIndex === cell.rowIndex && columnIndex === cell.columnIndex)
				continue;
			neighbors.push(playbackCellKey({ rowIndex, columnIndex }));
		}
	}
	return {
		neighbors,
		paneFootprint: {
			type,
			rowStart: firstRow + 1,
			columnStart: cell.columnIndex + 1,
			rowSpan,
			columnSpan,
		} as const,
	};
}

/** Claim only empty cells that exist in the pane's final display grid. */
export function projectPlaybackPaneFootprints(
	slots: PlaybackSlotProjection[],
): PlaybackSlotProjection[] {
	if (!slots.some(paneShape)) return slots;
	const empty = new Set(
		slots.filter((cell) => !cell.playback).map(playbackCellKey),
	);
	const claims = new Map<string, ReturnType<typeof paneClaim>>();
	const claimCounts = new Map<string, number>();
	for (const slot of slots) {
		const shape = paneShape(slot);
		if (!shape || shape === "single_button") continue;
		const claim = paneClaim(slot, shape);
		if (!claim.neighbors.every((neighbor) => empty.has(neighbor))) continue;
		claims.set(playbackCellKey(slot), claim);
		for (const neighbor of claim.neighbors)
			claimCounts.set(neighbor, (claimCounts.get(neighbor) ?? 0) + 1);
	}

	const claimed = new Set<string>();
	for (const [anchor, claim] of claims) {
		if (claim.neighbors.some((neighbor) => claimCounts.get(neighbor) !== 1)) {
			claims.delete(anchor);
			continue;
		}
		for (const neighbor of claim.neighbors) claimed.add(neighbor);
	}
	return slots.flatMap((slot) => {
		const key = playbackCellKey(slot);
		if (claimed.has(key)) return [];
		const shape = paneShape(slot);
		if (!shape) return [slot];
		return [
			{
				...slot,
				paneFootprint: claims.get(key)?.paneFootprint ?? { type: "single_button" },
			},
		];
	});
}
