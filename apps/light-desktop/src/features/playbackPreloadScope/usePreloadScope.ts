import { useEffect, useRef, useState } from "react";
import type {
	PlaybackPreloadScope,
	PlaybackPreloadScopeSnapshot,
} from "../../api/playbackConfigurationModels";
import { usePreloadScopeBoundary } from "./PreloadScopeContext";
/** Reads only while settings are mounted; all response installs stay within one authority epoch. */
export function usePreloadScope() {
	const boundary = usePreloadScopeBoundary();
	const [snapshot, setSnapshot] = useState<PlaybackPreloadScopeSnapshot | null>(
		null,
	);
	const [error, setError] = useState<string | null>(null),
		[saving, setSaving] = useState(false);
	const current = useRef<{
		abort: AbortController;
		latest: PlaybackPreloadScopeSnapshot | null;
		saving: boolean;
	} | null>(null);
	const authority = boundary?.authority,
		transport = boundary?.transport;
	useEffect(() => {
		setSnapshot(null);
		setError(null);
		setSaving(false);
		if (!authority || !transport) return;
		const epoch = {
			abort: new AbortController(),
			latest: null as PlaybackPreloadScopeSnapshot | null,
			saving: false,
		};
		current.current = epoch;
		const refresh = async () => {
			try {
				const next = await transport.loadSnapshot(
					authority.scope,
					epoch.abort.signal,
				);
				if (
					current.current === epoch &&
					(!epoch.latest || next.revision >= epoch.latest.revision)
				) {
					epoch.latest = next;
					setSnapshot(next);
					setError(null);
				}
			} catch (e) {
				if (current.current === epoch && !epoch.abort.signal.aborted)
					setError(message(e));
			}
		};
		const stream = transport.subscribe(authority.scope, {
			ready: () => void refresh(),
			changed: (c) => {
				if (
					c.showId === authority.scope.showId &&
					c.revision > (epoch.latest?.revision ?? -1)
				)
					void refresh();
			},
			gap: () => void refresh(),
			error: (e) => {
				if (current.current === epoch) setError(message(e));
			},
			closed: () => {
				if (current.current === epoch)
					setError(
						"Preload scope updates disconnected; reopen settings to reconnect",
					);
			},
		});
		void refresh();
		return () => {
			if (current.current === epoch) current.current = null;
			epoch.abort.abort();
			stream.close();
		};
	}, [authority?.authorityId, authority?.scope.showId, transport]);
	const save = async (scope: PlaybackPreloadScope) => {
		const epoch = current.current;
		if (!epoch?.latest || !authority || !transport || epoch.saving)
			return false;
		epoch.saving = true;
		setSaving(true);
		setError(null);
		try {
			const outcome = await transport.save(
				authority.scope,
				epoch.latest.revision,
				scope,
				crypto.randomUUID(),
				epoch.abort.signal,
			);
			if (current.current !== epoch) return false;
			if (outcome.revision >= epoch.latest.revision) {
				epoch.latest = outcome;
				setSnapshot(outcome);
			}
			return true;
		} catch (e) {
			if (current.current === epoch) setError(message(e));
			return false;
		} finally {
			if (current.current === epoch) {
				epoch.saving = false;
				setSaving(false);
			}
		}
	};
	return {
		key: `${authority?.authorityId ?? ""}:${authority?.scope.showId ?? ""}`,
		snapshot: snapshot?.show_id === authority?.scope.showId ? snapshot : null,
		error,
		saving,
		available: !!authority && !!transport,
		save,
	};
}
function message(e: unknown) {
	return e instanceof Error ? e.message : String(e);
}
