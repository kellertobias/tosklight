import { expect } from "@playwright/test";
import type { PaneHandle } from "../window-system/desktopScenario";
import type { PaneType } from "../window-system/paneTypes";

/** Authored tracks fill the available width and height independently; no rows hide below it. */
export async function expectPlaybackPaneFits(
	pane: PaneHandle<PaneType.VirtualPlaybacks>,
	rows: number,
	columns: number,
): Promise<void> {
	await expect(pane.root().locator("[data-playback-slot]")).toHaveCount(rows * columns);
	await expect.poll(() => pane.root().evaluate((root) => {
		const viewport = root.querySelector<HTMLElement>(".playback-bank-fixed, .playback-bank-viewport");
		const bank = root.querySelector<HTMLElement>(".playback-fader-bank");
		if (!viewport || !bank) return null;
		const paneBox = root.getBoundingClientRect();
		const viewportBox = viewport.getBoundingClientRect();
		const cards = [...bank.querySelectorAll<HTMLElement>("[data-playback-slot]")];
		const bounds = cards.map(card => card.getBoundingClientRect());
		const within = (box: DOMRect, outer: DOMRect) => box.left >= outer.left - 1
			&& box.top >= outer.top - 1 && box.right <= outer.right + 1 && box.bottom <= outer.bottom + 1;
		return {
			cells: cards.length,
			visibleRows: new Set(bounds.map(box => Math.round(box.top))).size,
			visibleColumns: new Set(bounds.map(box => Math.round(box.left))).size,
			allHaveArea: bounds.every(box => box.width > 0 && box.height > 0),
			fillsViewportWidth: Math.abs(Math.min(...bounds.map(box => box.left)) - viewportBox.left) <= 1
				&& Math.abs(Math.max(...bounds.map(box => box.right)) - viewportBox.right) <= 1,
			fillsViewportHeight: Math.abs(Math.min(...bounds.map(box => box.top)) - viewportBox.top) <= 1
				&& Math.abs(Math.max(...bounds.map(box => box.bottom)) - viewportBox.bottom) <= 1,
			equalColumnWidths: bounds.every(box => Math.abs(box.width - bounds[0].width) <= 1),
			equalRowHeights: bounds.every(box => Math.abs(box.height - bounds[0].height) <= 1),
			allInsidePane: bounds.every(box => within(box, paneBox)),
			allInsideViewport: bounds.every(box => within(box, viewportBox)),
			noHorizontalScroll: viewport.scrollWidth <= viewport.clientWidth + 1,
			noVerticalScroll: viewport.scrollHeight <= viewport.clientHeight + 1,
		};
	})).toEqual({
		cells: rows * columns,
		visibleRows: rows,
		visibleColumns: columns,
		allHaveArea: true,
		fillsViewportWidth: true,
		fillsViewportHeight: true,
		equalColumnWidths: true,
		equalRowHeights: true,
		allInsidePane: true,
		allInsideViewport: true,
		noHorizontalScroll: true,
		noVerticalScroll: true,
	});
}

/** The action type and cue insert share a visible bottom line in shallow authored rows. */
export async function expectShallowPlaybackFooter(
	pane: PaneHandle<PaneType.VirtualPlaybacks>,
): Promise<void> {
	await expect.poll(() => pane.root().evaluate(root => {
		const tile = [...root.querySelectorAll<HTMLElement>(".playback-tile-card")]
			.find(card => card.querySelector(".playback-tile-summary") && card.querySelector(".playback-tile-action-label"));
		const action = tile?.querySelector<HTMLElement>(".playback-tile-action-label");
		const cue = tile?.querySelector<HTMLElement>(".playback-tile-summary");
		const label = tile?.querySelector<HTMLElement>(".playback-tile-cue-label");
		if (!tile || !action || !cue || !label) return null;
		const outer = tile.getBoundingClientRect();
		const a = action.getBoundingClientRect();
		const c = cue.getBoundingClientRect();
		return {
			shallow: outer.height <= 100,
			cueTextHasSpace: label.getBoundingClientRect().width > 0,
			sameLine: Math.abs((a.top + a.bottom) / 2 - (c.top + c.bottom) / 2) <= 1,
			separateText: c.right <= a.left,
			visible: c.top >= outer.top && c.bottom <= outer.bottom && a.right <= outer.right,
		};
	})).toEqual({ shallow: true, cueTextHasSpace: true, sameLine: true, separateText: true, visible: true });
}
