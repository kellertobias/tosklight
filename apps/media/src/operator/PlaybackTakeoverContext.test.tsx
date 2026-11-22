import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../shared/api/client";
import { anOutput } from "../testing/server";
import { PlaybackTakeoverProvider, usePlaybackTakeover } from "./PlaybackTakeoverContext";

const mocks = vi.hoisted(() => ({ stop: vi.fn(), library: vi.fn(), takeover: vi.fn(), needsHandoff: vi.fn() }));
vi.mock("./usePlaylistStepPreview", () => ({ usePlaylistStepPreview: () => ({ stop: mocks.stop, needsHandoff: mocks.needsHandoff, active: null, busy: false, failure: undefined }) }));
vi.mock("./libraryPreview", () => ({ useLibraryPreview: () => ({ outputId: null, setEnabled: mocks.library }) }));
vi.mock("../shared/api/layerControl", async () => {
	const { anOutput } = await import("../testing/server");
	return { useOutputsForControl: () => ({ data: [anOutput({ id: "output" })] }), useLayerControl: () => ({ setTakeover: mocks.takeover }) };
});
beforeEach(() => { mocks.needsHandoff.mockReturnValue(true); vi.spyOn(api, "outputs").mockResolvedValue([anOutput({ id: "output", ownershipGeneration: 3 })]); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.clearAllMocks(); });
function pendingStop() { let resolve!: () => void; mocks.stop.mockImplementation(() => new Promise<void>(done => { resolve = done; })); return () => resolve(); }

it("waits for playlist restoration before enabling Library against the fresh generation", async () => {
	const release = pendingStop();
	const { result } = renderHook(usePlaybackTakeover, { wrapper: PlaybackTakeoverProvider });
	await waitFor(() => expect(result.current.selectedOutputId).toBe("output"));
	let enable!: Promise<void>; act(() => { enable = result.current.preview.setEnabled(true); });
	expect(mocks.library).not.toHaveBeenCalled(); expect(api.outputs).not.toHaveBeenCalled();
	await act(async () => { release(); await enable; });
	expect(mocks.library).toHaveBeenCalledWith(true, expect.objectContaining({ ownershipGeneration: 3 }));
});

it("waits for restoration before explicit manual takeover uses the resulting generation", async () => {
	const release = pendingStop();
	const { result } = renderHook(usePlaybackTakeover, { wrapper: PlaybackTakeoverProvider });
	let handoff!: Promise<unknown>;
	act(() => { handoff = result.current.control.setTakeover(anOutput({ id: "output", ownershipGeneration: 1 }), true, 1); });
	expect(mocks.takeover).not.toHaveBeenCalled();
	await act(async () => { release(); await handoff; });
	expect(mocks.takeover).toHaveBeenCalledWith(expect.objectContaining({ ownershipGeneration: 3 }), true, 3);
});

it("preserves immediate ordinary takeover feedback when there is no playlist cleanup", async () => {
	mocks.needsHandoff.mockReturnValue(false);
	const { result } = renderHook(usePlaybackTakeover, { wrapper: PlaybackTakeoverProvider });
	const output = anOutput({ id: "output", ownershipGeneration: 1 });
	const handoff = result.current.control.setTakeover(output, true, 1);
	expect(mocks.takeover).toHaveBeenCalledWith(output, true, 1);
	expect(mocks.stop).not.toHaveBeenCalled(); expect(api.outputs).not.toHaveBeenCalled();
	await handoff;
});
