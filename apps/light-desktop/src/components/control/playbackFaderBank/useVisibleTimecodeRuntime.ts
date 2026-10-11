import { useEffect, useState } from "react";
import type { PlaybackDefinition } from "../../../api/types/playback";
import type { TimecodeTransportSnapshot } from "../../../api/types/timecode";
import { useTimecodeActions } from "../../../features/timecode/TimecodeActionsContext";

/** One initial read per visible bank; real transport events remain the live authority. */
export function useVisibleTimecodeRuntime(showId: string | null, ids: string[]) {
	const authority = useTimecodeActions();
	const api = authority?.api;
	const events = authority?.events;
	const key = [...new Set(ids)].sort().join(",");
	const [state, setState] = useState<{
		scope: string;
		snapshots: Map<string, TimecodeTransportSnapshot>;
		error: string | null;
	}>({ scope: "", snapshots: new Map(), error: null });
	const scope = `${showId}:${key}`;
	useEffect(() => {
		let mounted = true;
		const wanted = new Set(key ? key.split(",") : []);
		setState({ scope, snapshots: new Map(), error: null });
		if (!showId || wanted.size === 0) return;
		if (!api) {
			setState({ scope, snapshots: new Map(), error: "Timecode authority unavailable" });
			return;
		}
		const accept = (incoming: TimecodeTransportSnapshot[]) => {
			if (!mounted) return;
			setState((current) => {
				const snapshots = new Map(current.scope === scope ? current.snapshots : []);
				for (const snapshot of incoming) {
					if (!wanted.has(snapshot.timecode_id)) continue;
					const previous = snapshots.get(snapshot.timecode_id);
					if (!previous || snapshot.revision >= previous.revision)
						snapshots.set(snapshot.timecode_id, snapshot);
				}
				return {
					scope, snapshots,
					error: [...wanted].every((id) => snapshots.has(id)) ? null : current.error,
				};
			});
		};
		const unsubscribe = events?.onRuntimeChanged((snapshot) => accept([snapshot]));
		void api.runtime(showId).then(accept).catch(() => {
			if (mounted) setState((current) => ({
				...current,
				error: [...wanted].every((id) => current.snapshots.has(id))
					? null : "Timecode runtime read failed",
			}));
		});
		return () => {
			mounted = false;
			unsubscribe?.();
		};
	}, [api, events, scope, showId, key]);
	return state.scope === scope
		? state
		: { scope, snapshots: new Map<string, TimecodeTransportSnapshot>(), error: null };
}

export function visibleTimecodeIds(slots: { playback?: PlaybackDefinition | null }[]) {
	return slots.flatMap(({ playback }) =>
		playback?.target.type === "timecode" ? [playback.target.timecode_id] : [],
	);
}
