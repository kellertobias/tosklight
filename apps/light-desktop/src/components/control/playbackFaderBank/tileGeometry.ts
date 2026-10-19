import type { CSSProperties } from "react";

export interface PlaybackTileViewport {
	width: number;
	height: number;
	columnGap: number;
	rowGap: number;
}

/** Explicit equal tracks keep sparse rows square rather than stretching them to pane height. */
export function squarePlaybackGeometry(
	viewport: PlaybackTileViewport,
	columns: number,
	rows: number,
	scrollable: boolean,
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
	const cell = scrollable
		? Math.max(112, width / columnCount)
		: Math.min(width / columnCount, height / rowCount);
	return {
		gridTemplateColumns: `repeat(${columnCount}, ${cell}px)`,
		gridTemplateRows: `repeat(${rowCount}, ${cell}px)`,
		width: columnCount * cell + viewport.columnGap * (columnCount - 1),
		height: rowCount * cell + viewport.rowGap * (rowCount - 1),
		minWidth: 0,
		minHeight: 0,
		padding: 0,
		alignContent: "start",
		justifyContent: "start",
		alignSelf: "start",
	};
}
