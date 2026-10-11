/** Touch pane geometry: fixed column/row order with scroll instead of font shrinking. */
export function scrollablePlaybackGeometry(rowTracks: string, columns: number) {
	const units = [...rowTracks.matchAll(/minmax\(0, (\d+)fr\)/g)].map((match) =>
		Number(match[1]),
	);
	const rowUnits = units.length ? units : [1];
	return {
		gridTemplateColumns: `repeat(${columns}, minmax(112px, 1fr))`,
		gridTemplateRows: rowUnits
			.map((unit) => `minmax(${unit * 92}px, ${unit}fr)`)
			.join(" "),
		minWidth: columns * 112 + (columns - 1) * 5,
		minHeight:
			rowUnits.reduce((sum, unit) => sum + unit * 92, 0) +
			(rowUnits.length - 1) * 5,
	};
}
