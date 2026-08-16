// Editing a stored visualizer.
//
// This is an object-intent edit, not live control, so it goes through the write path that carries
// a request id — and the panel is only re-read once the server has confirmed the change was
// stored. An optimistic edit here would show an operator a look the next start would not have.

import { useCallback, useEffect, useRef, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import type {
	CreateVisualizer,
	UpdateVisualizer,
	VisualizerView,
} from "../../shared/api/generated/media-wire";

export interface VisualizerEditing {
	/** Which visualizer is open for editing, by address. */
	editing: string | undefined;
	busy: boolean;
	failure: ApiFailure | undefined;
	begin: (key: string) => void;
	cancel: () => void;
	dismiss: () => void;
	save: (visualizer: VisualizerView, edit: UpdateVisualizer) => Promise<void>;
	saveLive: (visualizer: VisualizerView, edit: UpdateVisualizer) => void;
	create: (edit: CreateVisualizer) => Promise<VisualizerView | undefined>;
}

export function useVisualizerEditing(reload: () => void): VisualizerEditing {
	const [editing, setEditing] = useState<string | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);
	const pendingLive = useRef<
		{ visualizer: VisualizerView; edit: UpdateVisualizer } | undefined
	>(undefined);
	const liveTimer = useRef<number | undefined>(undefined);
	const drainingLive = useRef(false);

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
	const create = useCallback(
		async (edit: CreateVisualizer) => {
			setBusy(true);
			try {
				const created = await api.createVisualizer(edit);
				setFailure(undefined);
				setEditing(undefined);
				reload();
				return created;
			} catch (error) {
				setFailure(
					error instanceof ApiFailure
						? error
						: new ApiFailure("unexpected-error", String(error), 0),
				);
				return undefined;
			} finally {
				setBusy(false);
			}
		},
		[reload],
	);
	const drainLive = useCallback(async () => {
		if (drainingLive.current) return;
		drainingLive.current = true;
		setBusy(true);
		let saved = false;
		try {
			while (pendingLive.current) {
				const { visualizer, edit } = pendingLive.current;
				pendingLive.current = undefined;
				try {
					await api.updateVisualizer(
						visualizer.address.folder,
						visualizer.address.file,
						edit,
					);
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
		(visualizer: VisualizerView, edit: UpdateVisualizer) => {
			pendingLive.current = { visualizer, edit };
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

	return {
		editing,
		busy,
		failure,
		begin: useCallback((key: string) => setEditing(key), []),
		cancel: useCallback(() => setEditing(undefined), []),
		dismiss: useCallback(() => setFailure(undefined), []),
		save,
		saveLive,
		create,
	};
}
