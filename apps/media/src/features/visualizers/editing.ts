// Editing a stored visualizer.
//
// This is an object-intent edit, not live control, so it goes through the write path that carries
// a request id — and the panel is only re-read once the server has confirmed the change was
// stored. An optimistic edit here would show an operator a look the next start would not have.

import { useCallback, useEffect, useRef, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import { useEditingFailure } from "../../shared/api/editingFailure";
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
	const reportFailure = useEditingFailure(setFailure);
	const pendingLive = useRef(
		new Map<string, { visualizer: VisualizerView; edit: UpdateVisualizer }>(),
	);
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
				reportFailure(error);
			} finally {
				setBusy(false);
			}
		},
		[reload, reportFailure],
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
				reportFailure(error);
				return undefined;
			} finally {
				setBusy(false);
			}
		},
		[reload, reportFailure],
	);
	const drainLive = useCallback(async () => {
		if (drainingLive.current) return;
		drainingLive.current = true;
		setBusy(true);
		let saved = false;
		try {
			while (pendingLive.current.size > 0) {
				const [key, { visualizer, edit }] =
					pendingLive.current.entries().next().value!;
				pendingLive.current.delete(key);
				try {
					await api.updateVisualizer(
						visualizer.address.folder,
						visualizer.address.file,
						edit,
					);
					saved = true;
					setFailure(undefined);
				} catch (error) {
					reportFailure(error);
				}
			}
		} finally {
			drainingLive.current = false;
			setBusy(false);
			if (saved) reload();
		}
	}, [reload, reportFailure]);
	const saveLive = useCallback(
		(visualizer: VisualizerView, edit: UpdateVisualizer) => {
			pendingLive.current.set(
				`${visualizer.address.folder}/${visualizer.address.file}`,
				{ visualizer, edit },
			);
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

	const flushLive = useRef(drainLive);
	flushLive.current = drainLive;
	useEffect(
		() => () => {
			if (liveTimer.current !== undefined)
				window.clearTimeout(liveTimer.current);
			if (pendingLive.current.size > 0) void flushLive.current();
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
