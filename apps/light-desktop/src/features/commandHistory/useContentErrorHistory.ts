import type { CommandHistoryItem } from "@tosklight/ui";
import { useEffect, useState } from "react";

const ALERT_SELECTOR = '[role="alert"]';
const CONTENT_ERROR_SELECTOR = `.workspace-view ${ALERT_SELECTOR}`;
const MAX_CONTENT_ERRORS = 50;

function paneName(alert: Element): string {
	const pane = alert.closest<HTMLElement>(".desk-pane");
	const label = pane?.getAttribute("aria-label")?.trim();
	return label?.replace(/\s+pane$/i, "") || "Content window";
}

function isActiveAlert(alert: Element): boolean {
	return (
		!alert.closest('[aria-hidden="true"]') &&
		!alert.closest("[hidden]") &&
		alert.textContent?.trim() !== ""
	);
}

/**
 * Copies semantic pane alerts into the local command-line history. The pane remains
 * responsible for displaying and clearing its own error.
 */
export function useContentErrorHistory(): readonly CommandHistoryItem[] {
	const [entries, setEntries] = useState<readonly CommandHistoryItem[]>([]);

	useEffect(() => {
		const seen = new WeakMap<Element, string>();
		let sequence = 0;
		const collectAlert = (alert: Element) => {
			if (!alert.closest(".workspace-view") || !isActiveAlert(alert)) return;
			const feedback = alert.textContent?.trim() ?? "";
			if (seen.get(alert) === feedback) return;
			seen.set(alert, feedback);
			const name = paneName(alert);
			const at = new Date().toISOString();
			const entry: CommandHistoryItem = {
				id: `content-error-${at}-${sequence++}`,
				command: `${name.toUpperCase()} ERROR`,
				status: "rejected",
				feedback,
				source: "window",
				at,
			};
			setEntries((current) => [entry, ...current].slice(0, MAX_CONTENT_ERRORS));
		};
		const collectChangedNode = (node: Node) => {
			const element =
				node.nodeType === Node.ELEMENT_NODE
					? (node as Element)
					: node.parentElement;
			if (!element) return;
			const containingAlert = element.matches(ALERT_SELECTOR)
				? element
				: element.closest(ALERT_SELECTOR);
			if (containingAlert) collectAlert(containingAlert);
			for (const alert of element.querySelectorAll(ALERT_SELECTOR))
				collectAlert(alert);
		};

		for (const alert of document.querySelectorAll(CONTENT_ERROR_SELECTOR))
			collectAlert(alert);
		const observer = new MutationObserver((mutations) => {
			for (const mutation of mutations) {
				collectChangedNode(mutation.target);
				for (const node of mutation.addedNodes) collectChangedNode(node);
			}
		});
		observer.observe(document.body, {
			subtree: true,
			childList: true,
			characterData: true,
			attributes: true,
			attributeFilter: ["role", "aria-hidden", "hidden"],
		});
		return () => observer.disconnect();
	}, []);

	return entries;
}

export function mergeCommandHistory(
	authoritative: readonly CommandHistoryItem[],
	contentErrors: readonly CommandHistoryItem[],
): readonly CommandHistoryItem[] {
	return [...authoritative, ...contentErrors]
		.sort((left, right) => Date.parse(right.at) - Date.parse(left.at))
		.slice(0, 50);
}
