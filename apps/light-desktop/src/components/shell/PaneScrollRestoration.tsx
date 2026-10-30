import { type PropsWithChildren, useLayoutEffect, useRef } from "react";
import {
	normalizeRecordedScroll,
	paneScrollers,
	restorePaneScroll,
} from "../../features/panePresentation/recordedPaneScroll";

/** Restore once after content/layout exists; explicit operator input always takes ownership. */
export function PaneScrollRestoration({
	saved,
	recall,
	children,
}: PropsWithChildren<{ saved: unknown; recall: number }>) {
	const root = useRef<HTMLDivElement>(null);
	useLayoutEffect(() => {
		const node = root.current;
		if (!node) return;
		const pending = new Map(Object.entries(normalizeRecordedScroll(saved)));
		const measured = new Map<string, string>();
		if (!pending.size) return;
		let frame = 0;
		let stopped = false;
		const stop = () => {
			stopped = true;
			cancelAnimationFrame(frame);
			resize?.disconnect();
			mutations.disconnect();
		};
		const restore = () => {
			frame = 0;
			if (stopped) return;
			for (const { key, node: scroller } of paneScrollers(node)) {
				const position = pending.get(key);
				if (!position) continue;
				resize?.observe(scroller);
				if (scroller.firstElementChild)
					resize?.observe(scroller.firstElementChild);
				if (restorePaneScroll(scroller, position)) {
					// Measured pool columns can change on the next ResizeObserver turn.
					// Finish only after two frames agree on the viewport/content extent.
					const extent = [
						scroller.clientWidth,
						scroller.clientHeight,
						scroller.scrollWidth,
						scroller.scrollHeight,
					].join(":");
					if (measured.get(key) === extent) pending.delete(key);
					else {
						measured.set(key, extent);
						schedule();
					}
				} else measured.delete(key);
			}
			if (!pending.size) stop();
		};
		const schedule = () => {
			if (!stopped && !frame) frame = requestAnimationFrame(restore);
		};
		const resize =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(schedule);
		const mutations = new MutationObserver(schedule);
		mutations.observe(node, { childList: true, subtree: true });
		node.addEventListener("wheel", stop, { passive: true, capture: true });
		node.addEventListener("pointerdown", stop, true);
		node.addEventListener("keydown", stop, true);
		schedule();
		return () => {
			stop();
			node.removeEventListener("wheel", stop, true);
			node.removeEventListener("pointerdown", stop, true);
			node.removeEventListener("keydown", stop, true);
		};
	}, [saved, recall]);
	return (
		<div ref={root} style={{ display: "contents" }}>
			{children}
		</div>
	);
}
