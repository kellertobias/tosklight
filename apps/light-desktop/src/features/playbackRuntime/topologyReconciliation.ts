import { useEffect, useRef, useState } from "react";
import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../api/types";
import { usePlaybackRuntimeAuthority } from "./PlaybackRuntimeView";

export function playbackTargetMatches(
	target: PlaybackDefinition["target"],
	projection: PlaybackRuntimeProjection | undefined,
): boolean {
	if (!projection) return false;
	if (target.type === "cue_list")
		return (
			projection.target === "cue_list" &&
			projection.cue_list_id === target.cue_list_id
		);
	if (target.type === "group")
		return (
			projection.target === "group" && projection.group_id === target.group_id
		);
	if (target.type === "speed_group")
		return (
			projection.target === "speed_group" && projection.group === target.group
		);
	if (target.type === "dynamic")
		return (
			projection.target === "dynamic" &&
			projection.dynamic_id === target.assignment.dynamic_id
		);
	return projection.target === target.type;
}

/** Assignment changes keep runtime identity unchanged. Refresh only the active runtime scope,
 * once per visible target generation; a fresh contradictory authority remains a real error. */
export function usePlaybackTopologyRuntimeReconciliation(
	key: string,
	loaded: boolean,
	matches: boolean,
): boolean {
	const authority = usePlaybackRuntimeAuthority();
	const attempted = useRef<{ authority: typeof authority; key: string } | null>(
		null,
	);
	const queue = useRef<Promise<void>>(Promise.resolve());
	const [pending, setPending] = useState<string | null>(null);
	const needsRefresh =
		!!authority &&
		loaded &&
		!matches &&
		(attempted.current?.authority !== authority ||
			attempted.current?.key !== key);
	useEffect(() => {
		if (matches) {
			setPending(null);
			return;
		}
		if (!needsRefresh || !authority) return;
		attempted.current = { authority, key };
		setPending(key);
		// A later assignment gets its own snapshot after any earlier repair completes.
		const request = queue.current
			.catch(() => {})
			.then(() => authority.refreshAuthority());
		queue.current = request;
		void request
			.catch(() => {})
			.finally(() => {
				if (
					attempted.current?.authority === authority &&
					attempted.current?.key === key
				)
					setPending(null);
			});
		// Completion follows the latest target even across effect replay or loading changes.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [authority, key, loaded, matches]);
	return !matches && (needsRefresh || pending === key);
}
