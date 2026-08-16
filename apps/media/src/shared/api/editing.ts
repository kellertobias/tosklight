// Editing stored configuration.
//
// Every editor on this interface behaves the same way, and that sameness is deliberate: an edit
// carries a request id so a retry cannot become a second edit, the panel is only re-read once the
// server confirms the change was stored, and a refusal is shown rather than swallowed.
//
// Nothing here is optimistic. An optimistic edit of stored configuration would show an operator a
// look the next start would not have.

import { useCallback, useEffect, useRef, useState } from "react";
import { ApiFailure } from "./client";

export interface Editing {
	/** Which object is open for editing, by whatever key the feature uses. */
	editing: string | undefined;
	busy: boolean;
	failure: ApiFailure | undefined;
	begin: (key: string) => void;
	cancel: () => void;
	dismiss: () => void;
	/** Runs one edit, reloading on success and reporting the refusal otherwise. */
	save: (edit: () => Promise<unknown>) => Promise<void>;
	/** Coalesces rapid field changes, preserves their order, and keeps the editor open. */
	saveLive: (edit: () => Promise<unknown>) => void;
}

/** A client-generated identity for one edit. Resending it returns the first outcome. */
export function requestId(): string {
	return crypto.randomUUID();
}

export function useEditing(reload: () => void): Editing {
	const [editing, setEditing] = useState<string | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);
	const pendingLive = useRef<(() => Promise<unknown>) | undefined>(undefined);
	const liveTimer = useRef<number | undefined>(undefined);
	const drainingLive = useRef(false);

	const drainLive = useCallback(async () => {
		if (drainingLive.current) return;
		drainingLive.current = true;
		setBusy(true);
		let saved = false;
		try {
			while (pendingLive.current) {
				const edit = pendingLive.current;
				pendingLive.current = undefined;
				try {
					await edit();
					saved = true;
					setFailure(undefined);
				} catch (error) {
					setFailure(
						error instanceof ApiFailure
							? error
							: new ApiFailure("unexpected-error", String(error), 0),
					);
				}
			}
		} finally {
			drainingLive.current = false;
			setBusy(false);
			if (saved) reload();
		}
	}, [reload]);

	const saveLive = useCallback(
		(edit: () => Promise<unknown>) => {
			pendingLive.current = edit;
			setBusy(true);
			if (liveTimer.current !== undefined)
				window.clearTimeout(liveTimer.current);
			liveTimer.current = window.setTimeout(() => {
				liveTimer.current = undefined;
				void drainLive();
			}, 180);
		},
		[drainLive],
	);

	useEffect(
		() => () => {
			if (liveTimer.current !== undefined)
				window.clearTimeout(liveTimer.current);
		},
		[],
	);

	const save = useCallback(
		async (edit: () => Promise<unknown>) => {
			setBusy(true);
			try {
				await edit();
				setFailure(undefined);
				setEditing(undefined);
				reload();
			} catch (error) {
				setFailure(
					error instanceof ApiFailure
						? error
						: new ApiFailure("unexpected-error", String(error), 0),
				);
			} finally {
				setBusy(false);
			}
		},
		[reload],
	);

	return {
		editing,
		busy,
		failure,
		begin: useCallback((key: string) => setEditing(key), []),
		cancel: useCallback(() => setEditing(undefined), []),
		dismiss: useCallback(() => setFailure(undefined), []),
		save,
		saveLive,
	};
}
