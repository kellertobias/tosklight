import { useRef, useState } from "react";
import { cadSession } from "./session";

/**
 * The document's fixture selection as this window sees it, and which changes the patch sheet
 * should reveal.
 *
 * The session is the authority: the drawing and every window replace the same selection, and each
 * write names the revision it replaces. A selection made outside the sheet — in the drawing, or in
 * another window — is revealed. One the sheet made itself is already where the operator is
 * looking, so it must never move the sheet to another layer; the session's echo of it is recognised
 * by its fixtures.
 */
export function useCadSelection(report: (reason: unknown) => void) {
	const [selected, setSelected] = useState<readonly string[]>([]);
	const [revision, setRevision] = useState(0);
	const [revealRequest, setRevealRequest] = useState(0);
	const revisionRef = useRef(0);
	const queue = useRef<Promise<void>>(Promise.resolve());
	const sheetSelection = useRef<string | null>(null);

	function settle(ids: readonly string[], next: number) {
		setSelected(ids);
		setRevision(next);
		revisionRef.current = next;
	}

	/** A selection the session reports: loaded with the document, or changed anywhere. */
	function receive(ids: readonly string[], next: number) {
		settle(ids, next);
		if (sheetSelection.current === selectionKey(ids))
			sheetSelection.current = null;
		else if (ids.length) setRevealRequest((current) => current + 1);
	}

	/** The sheet replaces the selection. */
	function replace(ids: readonly string[]) {
		sheetSelection.current = selectionKey(ids);
		setSelected(ids);
		queue.current = queue.current.then(async () => {
			try {
				const delta = await cadSession.replaceSelection(
					revisionRef.current,
					ids,
				);
				settle(delta.selectedIds, delta.revision);
			} catch (reason) {
				report(reason);
				try {
					const snapshot = await cadSession.snapshot();
					settle(snapshot.selectedIds, snapshot.selectionRevision);
				} catch (refreshReason) {
					report(refreshReason);
				}
			}
		});
	}

	return { selected, revision, revealRequest, receive, replace };
}

/** The same fixtures in any order are the same selection. */
function selectionKey(ids: readonly string[]) {
	return [...ids].sort().join("\n");
}
