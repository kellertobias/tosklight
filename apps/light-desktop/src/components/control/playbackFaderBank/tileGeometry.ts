import type { CSSProperties } from "react";

export interface PlaybackTileViewport {
	width: number;
	height: number;
	columnGap: number;
	rowGap: number;
}

/** Fit the authored columns and rows independently to use the complete pane viewport. */
export function fittedPlaybackGeometry(
	viewport: PlaybackTileViewport,
	columns: number,
	rows: number,
): CSSProperties {
	const columnCount = Math.max(1, Math.floor(columns));
	const rowCount = Math.max(1, Math.floor(rows));
	const width = Math.max(
		0,
		viewport.width - viewport.columnGap * (columnCount - 1),
	);
	const height = Math.max(
		0,
		viewport.height - viewport.rowGap * (rowCount - 1),
	);
	const columnWidth = width / columnCount;
	const rowHeight = height / rowCount;
	return {
		gridTemplateColumns: `repeat(${columnCount}, ${columnWidth}px)`,
		gridTemplateRows: `repeat(${rowCount}, ${rowHeight}px)`,
		width: viewport.width,
		height: viewport.height,
		minWidth: 0,
		minHeight: 0,
		padding: 0,
		alignContent: "start",
		justifyContent: "start",
		alignSelf: "start",
	};
}
