import { useLayoutEffect, useState } from "react";
import {
	squarePlaybackGeometry,
	type PlaybackTileViewport,
} from "./tileGeometry";

/** Follow the actual pane viewport, including scrollbars and the bank's active CSS gaps. */
export function usePlaybackTileGeometry(
	enabled: boolean,
	viewport: HTMLElement | null,
	bank: HTMLElement | null,
	columns: number,
	rows: number,
	scrollable: boolean,
) {
	const [size, setSize] = useState<PlaybackTileViewport | null>(null);
	useLayoutEffect(() => {
		if (!enabled || !viewport || !bank) return;
		const measure = () => {
			const styles = getComputedStyle(bank);
			const next = {
				width: viewport.clientWidth,
				height: viewport.clientHeight,
				columnGap: Number.parseFloat(styles.columnGap) || 0,
				rowGap: Number.parseFloat(styles.rowGap) || 0,
			};
			setSize((previous) =>
				previous &&
				Object.keys(next).every(
					(key) =>
						previous[key as keyof PlaybackTileViewport] ===
						next[key as keyof PlaybackTileViewport],
				)
					? previous
					: next,
			);
		};
		measure();
		const resize =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(measure);
		resize?.observe(viewport);
		resize?.observe(bank);
		return () => resize?.disconnect();
	}, [enabled, viewport, bank]);
	return enabled && size
		? squarePlaybackGeometry(size, columns, rows, scrollable)
		: null;
}
