// Editing stored configuration.
//
// Every editor on this interface behaves the same way, and that sameness is deliberate: an edit
// carries a request id so a retry cannot become a second edit, the panel is only re-read once the
// server confirms the change was stored, and a refusal is shown rather than swallowed.
//
// Nothing here is optimistic. An optimistic edit of stored configuration would show an operator a
// look the next start would not have.

import { useCallback, useState } from "react";
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
}

/** A client-generated identity for one edit. Resending it returns the first outcome. */
export function requestId(): string {
	return crypto.randomUUID();
}

export function useEditing(reload: () => void): Editing {
	const [editing, setEditing] = useState<string | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);

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
	};
}
