// Editing a stored visualizer.
//
// This is an object-intent edit, not live control, so it goes through the write path that carries
// a request id — and the panel is only re-read once the server has confirmed the change was
// stored. An optimistic edit here would show an operator a look the next start would not have.

import { useCallback, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import type { UpdateVisualizer, VisualizerView } from "../../shared/api/generated/media-wire";

export interface VisualizerEditing {
	/** Which visualizer is open for editing, by address. */
	editing: string | undefined;
	busy: boolean;
	failure: ApiFailure | undefined;
	begin: (key: string) => void;
	cancel: () => void;
	dismiss: () => void;
	save: (visualizer: VisualizerView, edit: UpdateVisualizer) => Promise<void>;
}

export function useVisualizerEditing(reload: () => void): VisualizerEditing {
	const [editing, setEditing] = useState<string | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);

	const save = useCallback(
		async (visualizer: VisualizerView, edit: UpdateVisualizer) => {
			setBusy(true);
			try {
				await api.updateVisualizer(
					visualizer.address.folder,
					visualizer.address.file,
					edit,
				);
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
