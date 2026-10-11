/** Software geometry preserves configured order; Main uses compact rows that fit on entry. */
export function scrollablePlaybackGeometry(
	rowTracks: string,
	columns: number,
	compactMain = false,
) {
	const units = [...rowTracks.matchAll(/minmax\(0, (\d+)fr\)/g)].map((match) =>
		Number(match[1]),
	);
	const rowUnits = units.length ? units : [1];
	return {
		gridTemplateColumns: `repeat(${columns}, minmax(112px, 1fr))`,
		gridTemplateRows: rowUnits
			.map(
				(unit) =>
					`minmax(${compactMain ? unit * 64 + 8 : unit * 92}px, ${unit}fr)`,
			)
			.join(" "),
		minWidth: columns * 112 + (columns - 1) * 5,
		minHeight:
			rowUnits.reduce(
				(sum, unit) => sum + (compactMain ? unit * 64 + 8 : unit * 92),
				0,
			) +
			(rowUnits.length - 1) * 5,
	};
}
