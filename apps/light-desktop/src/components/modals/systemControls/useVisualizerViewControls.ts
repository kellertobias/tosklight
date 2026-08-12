import { useCallback, useEffect, useState } from "react";
import type {
	VisualizerRenderQuality,
	VisualizerView,
	VisualizerViewMode,
} from "../../../api/client/visualizerView";
import { useVisualizerViewActions } from "../../../features/visualizerView/VisualizerViewContext";

const DEFAULT_TARGET = "main";

/**
 * The desk's view for the renderer being addressed.
 *
 * Read when the surface opens, written when the operator selects something. The authoritative
 * answer to a write is what is displayed, so a desk that refused an edit is never shown as having
 * accepted it.
 */
export function useVisualizerViewControls(open: boolean) {
	const actions = useVisualizerViewActions();
	const [views, setViews] = useState<VisualizerView[]>([]);
	const [connected, setConnected] = useState(false);
	const [target, setTarget] = useState(DEFAULT_TARGET);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (!open || !actions) return;
		let cancelled = false;
		let connectionEventObserved = false;
		const unsubscribe = actions.onConnectionChanged((next) => {
			if (cancelled) return;
			connectionEventObserved = true;
			setConnected(next);
		});
		actions
			.snapshot()
			.then((loaded) => {
				if (cancelled) return;
				if (!connectionEventObserved) setConnected(loaded.connected);
				setViews(loaded.views);
				setError(null);
			})
			.catch((reason: unknown) => {
				if (cancelled) return;
				setError(describe(reason));
			});
		return () => {
			cancelled = true;
			unsubscribe();
		};
	}, [actions, open]);

	const apply = useCallback(
		async (patch: {
			mode?: VisualizerViewMode;
			quality?: VisualizerRenderQuality;
		}) => {
			if (!actions) return;
			setBusy(true);
			try {
				const updated = await actions.update(target, patch);
				setViews((current) => {
					const rest = current.filter((view) => view.target !== updated.target);
					return [...rest, updated].sort((left, right) =>
						left.target.localeCompare(right.target),
					);
				});
				setError(null);
			} catch (reason) {
				setError(describe(reason));
			} finally {
				setBusy(false);
			}
		},
		[actions, target],
	);

	return {
		connected,
		view: views.find((view) => view.target === target) ?? null,
		targets: views.map((view) => view.target),
		target,
		busy,
		error,
		selectTarget: setTarget,
		selectMode: (mode: VisualizerViewMode) => void apply({ mode }),
		selectQuality: (quality: VisualizerRenderQuality) =>
			void apply({ quality }),
	};
}

function describe(reason: unknown): string {
	return reason instanceof Error ? reason.message : String(reason);
}
