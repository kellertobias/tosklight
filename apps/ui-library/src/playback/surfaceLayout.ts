export type PlaybackPresentation = "fader" | "buttons" | "push_encoder";
export interface PlaybackSurfaceRow {
	first_position: number;
	count: number;
	presentation: PlaybackPresentation;
	button_count: number;
}
export interface PlaybackSurfaceLayout {
	rows: PlaybackSurfaceRow[];
}
export type PlaybackPageSource =
	| { type: "pinned"; page: number }
	| { type: "own"; page: number }
	| { type: "follow"; surface_id: string };
export interface PlaybackSurfaceConfiguration {
	name: string;
	layout: PlaybackSurfaceLayout;
	page_source: PlaybackPageSource;
}
export interface PlaybackSurfaceCell
	extends Omit<PlaybackSurfaceRow, "first_position" | "count"> {
	position: number;
	row: number;
	column: number;
}

/** Expands visible geometry only, never materializes show assignments. */
export function playbackSurfaceCells(
	layout: PlaybackSurfaceLayout,
): PlaybackSurfaceCell[] {
	return layout.rows.flatMap((row, index) =>
		Array.from({ length: row.count }, (_, column) => ({
			position: row.first_position + column,
			row: index,
			column,
			presentation: row.presentation,
			button_count: row.button_count,
		})),
	);
}

export function gridPlaybackLayout(
	firstPosition: number,
	columns: number,
	rows: number,
	presentation: PlaybackPresentation = "buttons",
): PlaybackSurfaceLayout {
	return {
		rows: Array.from({ length: rows }, (_, index) => ({
			first_position: firstPosition + index * columns,
			count: columns,
			presentation,
			button_count: 1,
		})),
	};
}

/** Take from Screen copies presentation into a reviewable draft, never assignments. */
export function copyPlaybackLayout(
	layout: PlaybackSurfaceLayout,
): PlaybackSurfaceLayout {
	return { rows: layout.rows.map((row) => ({ ...row })) };
}

export function playbackLayoutError(
	layout: PlaybackSurfaceLayout,
): string | null {
	const positions = new Set<number>();
	for (const row of layout.rows) {
		if (!Number.isSafeInteger(row.count) || row.count < 1 || row.count > 1000)
			return "Each playback row needs between 1 and 1000 controls";
		if (
			!Number.isSafeInteger(row.first_position) ||
			row.first_position < 1 ||
			row.first_position + row.count - 1 > 1000
		)
			return "Playback positions must be between 1 and 1000";
		if (
			!Number.isSafeInteger(row.button_count) ||
			row.button_count < 0 ||
			row.button_count > 3
		)
			return "Playback button count must be between 0 and 3";
		for (
			let position = row.first_position;
			position < row.first_position + row.count;
			position++
		) {
			if (positions.has(position))
				return `Playback position ${position} appears twice`;
			positions.add(position);
		}
	}
	return null;
}
