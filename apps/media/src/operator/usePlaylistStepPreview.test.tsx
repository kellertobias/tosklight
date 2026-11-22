import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Playlist } from "../shared/api/automation";
import { api } from "../shared/api/client";
import type { PlaylistPreviewResult, PreviewPlaylistStep } from "../shared/api/generated/media-wire";
import { cleanupPlaylistPreview, showPlaylistStep, stopPlaylistPreview } from "../shared/api/playlistPreview";
import type { Resource } from "../shared/api/resource";
import { anOutput } from "../testing/server";
import type { LibraryPreview } from "./libraryPreview";
import { usePlaylistStepPreview } from "./usePlaylistStepPreview";
import { usePreviewNavigation } from "./usePreviewNavigation";

vi.mock("../shared/api/playlistPreview", () => ({ showPlaylistStep: vi.fn(), stopPlaylistPreview: vi.fn(), cleanupPlaylistPreview: vi.fn(), acceptPreviewOutput: vi.fn() }));
const draft: Playlist = { id: "playlist", name: "Lobby", enabled: true, priority: 0, defaultFadeMillis: 0, layers: [1, 2], masterDimmer: 1, masterVolume: 1, auxiliaryLayers: [], entries: [] };
const resource = (generation = 0, takeover = false): Resource<ReturnType<typeof anOutput>[]> => ({ data: [anOutput({ id: "output", ownershipGeneration: generation, playbackTakeover: takeover })], loading: false, stale: false, failure: undefined, reload: vi.fn() });
const library: LibraryPreview = { outputId: null, setEnabled: vi.fn(async () => {}), show: vi.fn(async () => {}) };
function response(request: PreviewPlaylistStep): PlaylistPreviewResult {
	return { active: { outputId: request.outputId, previewId: request.previewId, sequence: request.sequence, ownershipGeneration: 1, playlistId: draft.id, entryId: request.entryId }, output: anOutput({ id: "output", ownershipGeneration: 1, playbackTakeover: true }) };
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
beforeEach(() => {
	vi.spyOn(api, "outputs").mockResolvedValue(resource().data!);
	vi.mocked(showPlaylistStep).mockImplementation(async request => response(request));
	vi.mocked(stopPlaylistPreview).mockResolvedValue({ active: null, output: anOutput() });
	vi.mocked(cleanupPlaylistPreview).mockResolvedValue({ active: null, output: anOutput() });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });

it("closes a pending Start, and every concurrent handoff waits for confirmed Stop", async () => {
	const start = deferred<PlaylistPreviewResult>(); const stopped = deferred<PlaylistPreviewResult>();
	let request!: PreviewPlaylistStep;
	vi.mocked(showPlaylistStep).mockImplementation(value => { request = value; return start.promise; });
	vi.mocked(stopPlaylistPreview).mockReturnValue(stopped.promise);
	const { result } = renderHook(() => usePlaylistStepPreview(resource(), "output", library));
	let show!: Promise<void>; act(() => { show = result.current.show(draft, "step"); });
	await waitFor(() => expect(showPlaylistStep).toHaveBeenCalledOnce());
	let first!: Promise<void>; let second!: Promise<void>; let finished = false;
	act(() => { first = result.current.stop(); second = result.current.stop().then(() => { finished = true; }); });
	await act(async () => { start.resolve(response(request)); await show; });
	await waitFor(() => expect(stopPlaylistPreview).toHaveBeenCalledOnce());
	expect(result.current.active).toBeNull(); expect(finished).toBe(false);
	await act(async () => { stopped.resolve({ active: null, output: anOutput() }); await Promise.all([first, second]); });
	expect(finished).toBe(true);
	expect(vi.mocked(stopPlaylistPreview).mock.calls[0][0].sequence).toBeGreaterThan(request.sequence);
});

it("ignores an older output poll, but abandons preview after a newer owner", async () => {
	const { result, rerender } = renderHook(({ outputs }) => usePlaylistStepPreview(outputs, "output", library), { initialProps: { outputs: resource() } });
	await act(async () => { await result.current.show(draft, "step"); });
	rerender({ outputs: resource(0, false) });
	expect(result.current.active?.entryId).toBe("step"); expect(stopPlaylistPreview).not.toHaveBeenCalled();
	rerender({ outputs: resource(2, true) });
	await waitFor(() => expect(stopPlaylistPreview).toHaveBeenCalledOnce());
	expect(result.current.active).toBeNull();
});

it("coalesces queued frame edits without replaying superseded steps", async () => {
	const start = deferred<PlaylistPreviewResult>(); let request!: PreviewPlaylistStep;
	vi.mocked(showPlaylistStep).mockImplementationOnce(value => { request = value; return start.promise; });
	const { result } = renderHook(() => usePlaylistStepPreview(resource(), "output", library));
	let first!: Promise<void>; act(() => { first = result.current.show(draft, "first"); });
	await waitFor(() => expect(showPlaylistStep).toHaveBeenCalledOnce());
	let middle!: Promise<void>; let last!: Promise<void>;
	act(() => { middle = result.current.show(draft, "middle"); last = result.current.show(draft, "last"); });
	await act(async () => { start.resolve(response(request)); await Promise.all([first, middle, last]); });
	expect(vi.mocked(showPlaylistStep).mock.calls.map(([value]) => value.entryId)).toEqual(["first", "last"]);
	expect(vi.mocked(showPlaylistStep).mock.calls[1][0].expectedOwnershipGeneration).toBe(1);
	expect(result.current.active?.entryId).toBe("last");
});

it("keeps Media controls unmounted while navigation waits for restoration", async () => {
	const stopped = deferred<void>(); const navigate = vi.fn(); const stop = vi.fn(() => stopped.promise);
	const { result } = renderHook(() => usePreviewNavigation(stop, navigate));
	let leaving!: Promise<void>; act(() => { leaving = result.current.navigate("/"); });
	expect(result.current.stopping).toBe(true); expect(navigate).not.toHaveBeenCalled();
	await act(async () => { stopped.resolve(); await leaving; });
	expect(navigate).toHaveBeenCalledWith("/"); expect(result.current.stopping).toBe(false);
});

it("waits for cleanup after an uncertain Start and permits a handoff once cleanup is confirmed", async () => {
	const cleaned = deferred<PlaylistPreviewResult>();
	vi.mocked(showPlaylistStep).mockRejectedValue(new Error("Timed out"));
	vi.mocked(cleanupPlaylistPreview).mockReturnValue(cleaned.promise);
	const { result } = renderHook(() => usePlaylistStepPreview(resource(), "output", library));
	let show!: Promise<void>;
	act(() => { show = result.current.show(draft, "step").catch(() => undefined); });
	await waitFor(() => expect(cleanupPlaylistPreview).toHaveBeenCalledOnce());
	let stop!: Promise<void>; let handedOff = false;
	act(() => { stop = result.current.stop().then(() => { handedOff = true; }); });
	expect(handedOff).toBe(false);
	await act(async () => { cleaned.resolve({ active: null, output: anOutput() }); await Promise.all([show, stop]); });
	expect(handedOff).toBe(true);
});

it("retries the exact failed Stop before permitting another handoff", async () => {
	const { result } = renderHook(() => usePlaylistStepPreview(resource(), "output", library));
	await act(async () => { await result.current.show(draft, "step"); });
	vi.mocked(stopPlaylistPreview).mockRejectedValue(new Error("Disconnected"));
	vi.mocked(cleanupPlaylistPreview).mockRejectedValueOnce(new Error("Disconnected"));
	await act(async () => { await result.current.stop().catch(() => undefined); });
	expect(result.current.failure).toBeDefined();
	await act(async () => { await result.current.stop(); });
	expect(cleanupPlaylistPreview).toHaveBeenCalledTimes(2);
	expect(vi.mocked(cleanupPlaylistPreview).mock.calls[1][0]).toEqual(vi.mocked(cleanupPlaylistPreview).mock.calls[0][0]);
	expect(result.current.failure).toBeUndefined();
});

it("leaves the current page visible when Stop cannot be confirmed", async () => {
	const navigate = vi.fn(); const stop = vi.fn(async () => { throw new Error("Disconnected"); });
	const { result } = renderHook(() => usePreviewNavigation(stop, navigate));
	await act(async () => { await result.current.navigate("/"); });
	expect(navigate).not.toHaveBeenCalled(); expect(result.current.stopping).toBe(false);
});
