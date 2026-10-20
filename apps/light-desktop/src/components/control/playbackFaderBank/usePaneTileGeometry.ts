import type { PoolCardSizing } from "@tosklight/ui/pools";
import { useLayoutEffect, useState } from "react";
import type { PlaybackFaderBankProps } from "./controller";
import { paneTileGeometry } from "./paneTileGeometry";
import type { PlaybackTileViewport } from "./tileGeometry";

export function usePaneTileGeometry(
	layout: PlaybackFaderBankProps["tileLayout"],
	viewport: HTMLElement | null,
	bank: HTMLElement | null,
	count: number,
	sizing: PoolCardSizing,
) {
	const [size, setSize] = useState<PlaybackTileViewport | null>(null);
	useLayoutEffect(() => {
		if (!layout || !viewport || !bank) return;
		const measure = () => {
			const style = getComputedStyle(bank);
			const next = {
				width: viewport.clientWidth,
				height: viewport.clientHeight,
				columnGap: Number.parseFloat(style.columnGap) || 0,
				rowGap: Number.parseFloat(style.rowGap) || 0,
			};
			setSize((previous) =>
				previous &&
				previous.width === next.width &&
				previous.height === next.height &&
				previous.columnGap === next.columnGap &&
				previous.rowGap === next.rowGap
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
		return () => resize?.disconnect();
	}, [Boolean(layout), viewport, bank]);
	return layout && size ? paneTileGeometry(size, count, layout, sizing) : null;
}
