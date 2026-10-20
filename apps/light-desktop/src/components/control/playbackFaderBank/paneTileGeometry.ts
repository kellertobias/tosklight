import {
	type PoolCardSizing,
	resolvePoolGridColumns,
} from "@tosklight/ui/pools";
import type { CSSProperties } from "react";
import type { PlaybackFaderBankProps } from "./controller";
import type { PlaybackTileViewport } from "./tileGeometry";

/** Reflow presentation only: never manufacture or discard authored addresses. */
export function paneTileGeometry(
	viewport: PlaybackTileViewport,
	count: number,
	layout: NonNullable<PlaybackFaderBankProps["tileLayout"]>,
	sizing: PoolCardSizing = { defaultWidth: 144, minimumWidth: 100 },
) {
	const columns =
		layout.mode === "fixed"
			? Math.max(1, Math.floor(layout.columns))
			: resolvePoolGridColumns(viewport.width, viewport.columnGap, sizing);
	const width = Math.max(
		0,
		(viewport.width - viewport.columnGap * (columns - 1)) / columns,
	);
	const height =
		layout.mode === "automatic"
			? width
			: Math.max(
					0,
					(viewport.height - viewport.rowGap * (layout.rows - 1)) / layout.rows,
				);
	const rows = Math.max(
		layout.mode === "fixed" ? layout.rows : 1,
		Math.ceil(count / columns),
	);
	return {
		columns,
		rows,
		style: {
			gridTemplateColumns: `repeat(${columns}, ${width}px)`,
			gridTemplateRows: `repeat(${rows}, ${height}px)`,
			width: viewport.width,
			height: rows * height + (rows - 1) * viewport.rowGap,
			minHeight: 0,
			minWidth: 0,
			padding: 0,
			alignContent: "start",
			overflow: "visible",
		} as CSSProperties,
	};
}

export function reflowPlaybackCells<
	T extends { rowIndex: number; columnIndex: number },
>(cells: readonly T[], columns: number): T[] {
	return cells.map((cell, index) => ({
		...cell,
		rowIndex: Math.floor(index / columns),
		columnIndex: index % columns,
	}));
}
