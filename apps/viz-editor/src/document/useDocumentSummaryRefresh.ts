import { useEffect, useMemo, useRef } from "react";
import type { CadSceneSnapshot } from "../cad/types";
import { type DocumentSummary, documentSession } from "./session";

/** Focused reads for the visible Show; scene events invalidate, never approximate its metadata. */
export function useDocumentSummaryRefresh({
	visible,
	document,
	sceneRevision,
	onSummary,
	onError,
}: {
	visible: boolean;
	document: DocumentSummary | null;
	sceneRevision: number | null;
	onSummary(summary: DocumentSummary): void;
	onError(reason: unknown): void;
}) {
	const showId = document?.showId ?? null;
	const latest = useRef({ visible, showId, document, onSummary, onError });
	latest.current = { visible, showId, document, onSummary, onError };
	const context = useMemo(
		() => ({ active: true, running: false, dirty: false, sceneBound: false }),
		[showId],
	);
	const previous = useRef({ visible, showId, sceneRevision });

	useEffect(() => {
		context.active = true;
		return () => {
			context.active = false;
		};
	}, [context]);

	useEffect(() => {
		const before = previous.current;
		previous.current = { visible, showId, sceneRevision };
		const firstScene = visible && sceneRevision !== null && !context.sceneBound;
		if (firstScene) context.sceneBound = true;
		const changed = firstScene || (before.showId === showId
			&& sceneRevision !== null
			&& before.sceneRevision !== sceneRevision);
		// Validate once summary and matching scene first meet, whichever arrived first.
		// Later reads follow Show entry or scene edits, never an idle polling loop.
		if (!visible || !showId || (before.visible && !changed)) return;
		context.dirty = true;
		if (context.running) return;
		context.running = true;
		const current = () => context.active
			&& latest.current.visible
			&& latest.current.showId === showId;
		void (async () => {
			try {
				while (current() && context.dirty) {
					context.dirty = false;
					const baseline = latest.current.document;
					try {
						const summary = await documentSession.current();
						if (current() && !context.dirty
							&& latest.current.document === baseline
							&& summary?.showId === showId) {
							latest.current.onSummary(summary);
						}
					} catch (reason) {
						if (current() && !context.dirty && latest.current.document === baseline) {
							latest.current.onError(reason);
						}
					}
				}
			} finally {
				context.running = false;
			}
		})();
	}, [visible, showId, sceneRevision, context]);
}

/** Bind scene revisions only after both independently loaded snapshots identify the same show. */
export function useSceneDocumentSummaryRefresh(
	visible: boolean,
	document: DocumentSummary | null,
	scene: CadSceneSnapshot | null,
	onSummary: (summary: DocumentSummary) => void,
	onError: (reason: unknown) => void,
) {
	useDocumentSummaryRefresh({
		visible,
		document,
		sceneRevision: scene?.showId === document?.showId ? scene?.sceneRevision ?? null : null,
		onSummary,
		onError,
	});
}
