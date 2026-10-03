import { useEffect, useState } from "react";
import { useVisualizerViewActions } from "../../features/visualizerView/VisualizerViewContext";

/**
 * Whether an Architect is following the active show and keeping it in step with this desk.
 *
 * Read when the desk connects, then kept current by `architect_sync_changed`. A change that
 * arrives before the first read wins over it, so the indicator never shows a stale answer.
 */
export function useArchitectSyncActive(): boolean {
	const actions = useVisualizerViewActions();
	const [active, setActive] = useState(false);
	useEffect(() => {
		const read = actions?.architectSync;
		const follow = actions?.onArchitectSyncChanged;
		if (!read || !follow) return;
		let cancelled = false;
		let observed = false;
		const unsubscribe = follow((next) => {
			if (cancelled) return;
			observed = true;
			setActive(next);
		});
		read()
			.then((next) => {
				if (!cancelled && !observed) setActive(next);
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
			unsubscribe();
		};
	}, [actions]);
	return active;
}

/** The sentence the show button adds while an Architect follows the show. */
export function architectSyncDetail(active: boolean): string {
	return active
		? " An Architect is synchronized with this show; its edits arrive here automatically."
		: "";
}
