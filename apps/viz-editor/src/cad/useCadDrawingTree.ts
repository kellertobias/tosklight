/**
 * The open show's drawing arrangement, followed across windows and saved on every change.
 *
 * A change shows at once and is taken back, with the reason, when the show refuses it.
 */
import { useEffect, useRef, useState } from "react";
import {
	type DrawingTree,
	drawingTreeSession,
	EMPTY_DRAWING_TREE,
} from "./drawingTree";

export function useCadDrawingTree(documentKey: string | null) {
	const [tree, setTree] = useState<DrawingTree>(EMPTY_DRAWING_TREE);
	const [error, setError] = useState<string | null>(null);
	const current = useRef(tree);
	current.current = tree;

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		drawingTreeSession
			.get()
			.then((loaded) => !disposed && setTree(loaded ?? EMPTY_DRAWING_TREE))
			.catch(() => !disposed && setTree(EMPTY_DRAWING_TREE));
		drawingTreeSession
			.onDelta((next) => !disposed && setTree(next))
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => undefined);
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, [documentKey]);

	function change(next: DrawingTree) {
		const previous = current.current;
		setTree(next);
		setError(null);
		drawingTreeSession.save(next).catch((reason) => {
			setTree(previous);
			setError(String(reason));
		});
	}

	return { tree, change, error };
}
