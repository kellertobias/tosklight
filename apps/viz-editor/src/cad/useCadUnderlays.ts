/**
 * The venue drawings the open show carries.
 *
 * Drawings are show data, not workspace state: they travel with the document, and every editor
 * window is told when one is added, placed or removed, so two windows over the same show never
 * disagree about what the plan sits on.
 */
import { useEffect, useState } from "react";
import type { CadViewDirection } from "./types";
import { type CadUnderlay, underlaySession } from "./underlays";

export interface CadUnderlays {
	underlays: CadUnderlay[];
	/** True while a file is being read, so the panel can say so instead of looking idle. */
	busy: boolean;
	error: string | null;
	clearError(): void;
	place(path: string, view: CadViewDirection): Promise<void>;
	change(underlay: CadUnderlay): Promise<void>;
	remove(id: string): Promise<void>;
}

export function useCadUnderlays(documentKey: string | null): CadUnderlays {
	const [underlays, setUnderlays] = useState<CadUnderlay[]>([]);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		underlaySession
			.all()
			.then((loaded) => !disposed && setUnderlays(loaded))
			.catch(() => !disposed && setUnderlays([]));
		underlaySession
			.onDelta((next) => !disposed && setUnderlays(next))
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

	async function run(action: () => Promise<unknown>) {
		setBusy(true);
		setError(null);
		try {
			await action();
		} catch (reason) {
			setError(String(reason));
		} finally {
			setBusy(false);
		}
	}

	return {
		underlays,
		busy,
		error,
		clearError: () => setError(null),
		place: (path, view) => run(() => underlaySession.import(path, view)),
		change: (underlay) => run(() => underlaySession.save(underlay)),
		remove: (id) => run(() => underlaySession.remove(id)),
	};
}
