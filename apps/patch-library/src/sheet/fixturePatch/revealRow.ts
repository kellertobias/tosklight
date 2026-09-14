/**
 * Scrolls a patch table up or down until the row is visible between the sticky header and any
 * `bottomInset` covering the table's foot.
 *
 * Only ever vertically: `scrollIntoView` also scrolls sideways towards a row wider than the table,
 * which moves the cell under the pointer between pressing and releasing the mouse, so the click
 * lands on whatever slid there instead.
 */
export function revealPatchRow(row: HTMLElement, bottomInset = 0) {
	const container = row.closest<HTMLElement>(".patch-table-wrap");
	if (!container) return;
	const area = container.getBoundingClientRect();
	// Without layout there is nothing to scroll.
	if (area.height <= 0) return;
	const header = container.querySelector("thead")?.getBoundingClientRect();
	const top = Math.max(area.top, header?.bottom ?? area.top);
	const bottom = area.bottom - bottomInset;
	const rect = row.getBoundingClientRect();
	if (rect.top < top) container.scrollTop -= top - rect.top;
	else if (rect.bottom > bottom)
		container.scrollTop += Math.min(rect.bottom - bottom, rect.top - top);
}
