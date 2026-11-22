/** Temporary output audition, serialized with cancellation and server-owned restoration. */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiFailure, api } from "../shared/api/client";
import type { Playlist } from "../shared/api/automation";
import type { OutputView, PlaylistPreviewState, StopPlaylistPreview } from "../shared/api/generated/media-wire";
import { acceptPreviewOutput, cleanupPlaylistPreview, showPlaylistStep, stopPlaylistPreview } from "../shared/api/playlistPreview";
import type { Resource } from "../shared/api/resource";
import { newIdentity } from "../shared/api/identity";
import type { LibraryPreview } from "./libraryPreview";

export interface PlaylistStepPreview {
	active: Pick<PlaylistPreviewState, "outputId" | "previewId" | "playlistId" | "entryId"> | null;
	busy: boolean;
	failure: ApiFailure | undefined;
	show: (playlist: Playlist, entryId: string) => Promise<void>;
	stop: () => Promise<void>;
	dismissFailure: () => void;
	/** Reads pending session state synchronously so ordinary controls keep immediate feedback. */
	needsHandoff?: () => boolean;
}
interface Session {
	previewId: string;
	outputId: string;
	sequence: number;
	generation: number | undefined;
	closed: boolean;
}
const asFailure = (error: unknown) => error instanceof ApiFailure ? error : new ApiFailure("playlist-preview-failed", error instanceof Error ? error.message : "The playlist preview could not be confirmed", 0);

export function usePlaylistStepPreview(
	outputs: Resource<OutputView[]>, selectedOutputId: string, library: LibraryPreview,
): PlaylistStepPreview {
	const [active, setActive] = useState<PlaylistPreviewState | null>(null);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<ApiFailure>();
	const session = useRef<Session | null>(null);
	const queue = useRef<Promise<unknown>>(Promise.resolve());
	const intent = useRef(0);
	const pending = useRef(0);
	const latest = useRef({ outputs, selectedOutputId, library });
	latest.current = { outputs, selectedOutputId, library };
	const closing = useRef(new Map<string, StopPlaylistPreview>());

	const enqueue = useCallback(<T,>(operation: () => Promise<T>) => {
		pending.current++; setBusy(true);
		const result = queue.current.catch(() => undefined).then(operation);
		queue.current = result;
		void result.then(() => { pending.current--; setBusy(pending.current > 0); }, () => { pending.current--; setBusy(pending.current > 0); });
		return result;
	}, []);

	const stop = useCallback(async () => {
		intent.current++;
		const held = session.current;
		// A second handoff must await the first Stop, too, before it can write controls.
		if (!held || held.closed) {
			try { await queue.current; return; }
			catch { if (!closing.current.size) return; }
			// A failed cleanup remains retryable by Stop, navigation, or a manual handoff.
			return enqueue(async () => {
				for (const [id, request] of closing.current) {
					const result = await cleanupPlaylistPreview(request);
					acceptPreviewOutput(result, latest.current.outputs.data);
					closing.current.delete(id);
				}
				setFailure(undefined);
			});
		}
		held.closed = true;
		session.current = null;
		setActive(null);
		const request = { outputId: held.outputId, previewId: held.previewId, sequence: ++held.sequence };
		closing.current.set(held.previewId, request);
		return enqueue(async () => {
			try {
				let result;
				try { result = await stopPlaylistPreview(request); }
				catch { result = await cleanupPlaylistPreview(request); }
				acceptPreviewOutput(result, latest.current.outputs.data);
				closing.current.delete(held.previewId);
				setFailure(undefined);
			} catch (error) {
				setFailure(asFailure(error));
				latest.current.outputs.reload();
				throw error;
			}
		});
	}, [enqueue]);

	const show = useCallback(async (playlist: Playlist, entryId: string) => {
		const outputId = latest.current.selectedOutputId;
		if (!outputId) {
			const error = new ApiFailure("preview-output-required", "Choose a Pixel output before previewing a step", 400);
			setFailure(error); throw error;
		}
		if (session.current && session.current.outputId !== outputId) await stop();
		let held = session.current;
		if (!held || held.closed) {
			held = { previewId: newIdentity(), outputId, sequence: 0, generation: undefined, closed: false };
			session.current = held;
		}
		const requested = held;
		const version = ++intent.current;
		return enqueue(async () => {
			if (requested.closed || version !== intent.current) return;
			try {
				if (latest.current.library.outputId !== null) await latest.current.library.setEnabled(false);
				if (requested.closed || version !== intent.current) return;
				if (requested.generation === undefined) {
					const fresh = (await api.outputs()).find(output => output.id === outputId);
					if (!fresh) throw new ApiFailure("output-not-found", "The selected preview output no longer exists", 404);
					requested.generation = fresh.ownershipGeneration;
				}
				if (requested.closed || version !== intent.current) return;
				const result = await showPlaylistStep({ outputId, previewId: requested.previewId, sequence: ++requested.sequence, expectedOwnershipGeneration: requested.generation, playlist, entryId });
				if (!result.active) throw new ApiFailure("preview-not-started", "The server did not start this preview", 409);
				requested.generation = result.active.ownershipGeneration;
				acceptPreviewOutput(result, latest.current.outputs.data);
				if (!requested.closed && version === intent.current) { setActive(result.active); setFailure(undefined); }
			} catch (error) {
				setFailure(asFailure(error));
				// A timed-out Start may have been applied: close this exact ID, never retry it.
				requested.closed = true;
				if (session.current === requested) { session.current = null; setActive(null); }
				const request = { outputId, previewId: requested.previewId, sequence: ++requested.sequence };
				closing.current.set(requested.previewId, request);
				try {
					const result = await cleanupPlaylistPreview(request);
					acceptPreviewOutput(result, latest.current.outputs.data);
					closing.current.delete(requested.previewId);
				} catch { latest.current.outputs.reload(); }
				throw error;
			}
		});
	}, [enqueue, stop]);

	useEffect(() => {
		const held = session.current;
		if (held && held.outputId !== selectedOutputId) void stop().catch(() => undefined);
	}, [selectedOutputId, stop]);
	useEffect(() => {
		const held = session.current;
		if (!held || held.generation === undefined) return;
		const output = outputs.data?.find(each => each.id === held.outputId);
		// An older in-flight poll may arrive after the Start acknowledgement; ignore it.
		if (output && (output.ownershipGeneration > held.generation || (output.ownershipGeneration === held.generation && !output.playbackTakeover))) void stop().catch(() => undefined);
	}, [outputs.data, stop]);
	useEffect(() => {
		const cleanup = () => {
			intent.current++;
			const held = session.current;
			if (held && !held.closed) {
				held.closed = true; session.current = null;
				closing.current.set(held.previewId, { outputId: held.outputId, previewId: held.previewId, sequence: ++held.sequence });
			}
			for (const request of closing.current.values()) void cleanupPlaylistPreview(request).catch(() => undefined);
		};
		window.addEventListener("pagehide", cleanup);
		return () => { window.removeEventListener("pagehide", cleanup); cleanup(); };
	}, []);
	const dismissFailure = useCallback(() => setFailure(undefined), []);
	const needsHandoff = useCallback(() => session.current !== null || closing.current.size > 0 || pending.current > 0, []);
	return useMemo(() => ({ active, busy, failure, show, stop, dismissFailure, needsHandoff }), [active, busy, failure, show, stop, dismissFailure, needsHandoff]);
}
