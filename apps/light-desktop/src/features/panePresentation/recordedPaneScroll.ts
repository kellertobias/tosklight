import type { DeskModel } from "../../types";

export interface RecordedScrollPosition {
	top: number;
	left: number;
	poolPosition?: number;
	poolOffset?: number;
}
export type RecordedPaneScroll = Record<string, RecordedScrollPosition>;

/** Shared WindowScrollArea viewports are the supported presentation boundary. */
export function paneScrollers(root: ParentNode) {
	const occurrences = new Map<string, number>();
	return Array.from(
		root.querySelectorAll<HTMLElement>(".ui-window-scroller"),
	).map((node) => {
		const name = node.dataset.viewScrollKey ?? "main";
		const index = occurrences.get(name) ?? 0;
		occurrences.set(name, index + 1);
		return { key: `${name}:${index}`, node };
	});
}

export function normalizeRecordedScroll(value: unknown): RecordedPaneScroll {
	if (!value || typeof value !== "object" || Array.isArray(value)) return {};
	return Object.fromEntries(
		Object.entries(value)
			.slice(0, 32)
			.flatMap(([key, raw]) => {
				if (!raw || typeof raw !== "object") return [];
				const item = raw as RecordedScrollPosition;
				if (
					!Number.isFinite(item.top) ||
					!Number.isFinite(item.left) ||
					item.top < 0 ||
					item.left < 0
				)
					return [];
				return [
					[
						key,
						{
							top: item.top,
							left: item.left,
							...(Number.isInteger(item.poolPosition) &&
							item.poolPosition! >= 0 &&
							Number.isFinite(item.poolOffset)
								? {
										poolPosition: item.poolPosition,
										poolOffset: item.poolOffset,
									}
								: {}),
						},
					],
				];
			}),
	);
}

export function capturePaneScroll(root: ParentNode): RecordedPaneScroll {
	return Object.fromEntries(
		paneScrollers(root)
			.filter(({ node }) => node.clientHeight > 0)
			.map(({ key, node }) => {
				const bounds = node.getBoundingClientRect();
				const anchor = Array.from(
					node.querySelectorAll<HTMLElement>("[data-pool-position]"),
				).find((slot) => slot.getBoundingClientRect().bottom > bounds.top + 1);
				return [
					key,
					{
						top: node.scrollTop,
						left: node.scrollLeft,
						...(anchor
							? {
									poolPosition: Number(anchor.dataset.poolPosition),
									poolOffset: anchor.getBoundingClientRect().top - bounds.top,
								}
							: {}),
					},
				];
			}),
	);
}

export function captureDesktopScroll(desk: DeskModel) {
	const roots = new Map(
		Array.from(document.querySelectorAll<HTMLElement>("[data-pane-id]")).map(
			(node) => [node.dataset.paneId, node],
		),
	);
	return Object.fromEntries(
		desk.panes.flatMap((pane) => {
			const root = roots.get(pane.id);
			return root ? [[pane.id, capturePaneScroll(root)]] : [];
		}),
	);
}

/** Returns false while delayed content has not yet supplied the requested pool slot/extent. */
export function restorePaneScroll(
	node: HTMLElement,
	position: RecordedScrollPosition,
) {
	if (node.clientHeight <= 0) return false;
	const anchor =
		position.poolPosition === undefined
			? null
			: node.querySelector<HTMLElement>(
					`[data-pool-position="${position.poolPosition}"]`,
				);
	if (anchor && anchor.getBoundingClientRect().height <= 0) return false;
	const top = anchor
		? node.scrollTop +
			anchor.getBoundingClientRect().top -
			node.getBoundingClientRect().top -
			(position.poolOffset ?? 0)
		: position.top;
	const maxTop = Math.max(0, node.scrollHeight - node.clientHeight);
	const maxLeft = Math.max(0, node.scrollWidth - node.clientWidth);
	node.scrollTop = Math.max(0, Math.min(top, maxTop));
	node.scrollLeft = Math.max(0, Math.min(position.left, maxLeft));
	return (
		(position.poolPosition !== undefined
			? Boolean(anchor)
			: maxTop >= position.top) && maxLeft >= position.left
	);
}
