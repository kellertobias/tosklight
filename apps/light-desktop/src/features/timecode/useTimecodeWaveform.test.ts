// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	TimecodeAudio,
	TimecodeAudioWaveform,
} from "../../api/types/timecode";
import { useTimecodeWaveform } from "./useTimecodeWaveform";

afterEach(cleanup);
const first: TimecodeAudio = { asset_id: "one", asset_revision: 1 };
const second: TimecodeAudio = { asset_id: "two", asset_revision: 1 };
function deferred() {
	let resolve!: (value: TimecodeAudioWaveform) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<TimecodeAudioWaveform>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

describe("useTimecodeWaveform", () => {
	it("replaces peaks by asset identity, supports undo and clears removed audio", async () => {
		const api = {
			waveform: vi
				.fn()
				.mockResolvedValueOnce({ peaks: [0.1] })
				.mockResolvedValueOnce({ peaks: [0.8] }),
		};
		const { result, rerender } = renderHook(
			({ audio }: { audio: TimecodeAudio | null }) =>
				useTimecodeWaveform({
					showId: "show",
					timecodeId: "timecode",
					audio,
					savedAudio: audio,
					api,
				}),
			{ initialProps: { audio: first as TimecodeAudio | null } },
		);
		await waitFor(() => expect(result.current.waveformPeaks).toEqual([0.1]));
		rerender({ audio: second });
		expect(result.current.waveformPeaks).toBeUndefined();
		expect(result.current.waveformLoading).toBe(true);
		await waitFor(() => expect(result.current.waveformPeaks).toEqual([0.8]));
		rerender({ audio: first });
		expect(result.current.waveformPeaks).toEqual([0.1]);
		expect(api.waveform).toHaveBeenCalledTimes(2);
		rerender({ audio: null });
		expect(result.current.waveformPeaks).toBeUndefined();
		expect(result.current.waveformError).toBeNull();
		expect(result.current.waveformLoading).toBe(false);
	});

	it("ignores stale success and failure after identity changes", async () => {
		const old = deferred();
		const current = deferred();
		const api = {
			waveform: vi
				.fn()
				.mockReturnValueOnce(old.promise)
				.mockReturnValueOnce(current.promise),
		};
		const { result, rerender } = renderHook(
			({ audio }) =>
				useTimecodeWaveform({
					showId: "show",
					timecodeId: "timecode",
					audio,
					savedAudio: audio,
					api,
				}),
			{ initialProps: { audio: first } },
		);
		rerender({ audio: second });
		await act(async () => {
			old.reject(new Error("old asset unavailable"));
		});
		expect(result.current.waveformError).toBeNull();
		expect(result.current.waveformLoading).toBe(true);
		await act(async () => {
			current.resolve({ peaks: [0.9] });
		});
		expect(result.current.waveformPeaks).toEqual([0.9]);
	});

	it("does not fetch an unsaved replacement from the route serving the previous audio", async () => {
		const api = { waveform: vi.fn().mockResolvedValue({ peaks: [0.3] }) };
		const { result, rerender } = renderHook(
			({ savedAudio }) =>
				useTimecodeWaveform({
					showId: "show",
					timecodeId: "timecode",
					audio: second,
					savedAudio,
					api,
				}),
			{ initialProps: { savedAudio: first } },
		);
		expect(api.waveform).not.toHaveBeenCalled();
		expect(result.current.waveformPeaks).toBeUndefined();
		rerender({ savedAudio: second });
		await waitFor(() => expect(result.current.waveformPeaks).toEqual([0.3]));
		expect(api.waveform).toHaveBeenCalledWith("show", "timecode");
	});

	it("keeps failures stable until explicit retry and clears errors for another asset", async () => {
		const api = {
			waveform: vi
				.fn()
				.mockRejectedValueOnce(new Error("offline"))
				.mockResolvedValueOnce({ peaks: [0.4] }),
		};
		const { result, rerender } = renderHook(
			({ audio }) =>
				useTimecodeWaveform({
					showId: "show",
					timecodeId: "timecode",
					audio,
					savedAudio: audio,
					api,
				}),
			{ initialProps: { audio: first } },
		);
		await waitFor(() =>
			expect(result.current.waveformError).toContain("offline"),
		);
		expect(result.current.waveformLoading).toBe(false);
		rerender({ audio: { ...first } });
		expect(api.waveform).toHaveBeenCalledTimes(1);
		act(() => result.current.retryWaveform());
		await waitFor(() => expect(result.current.waveformPeaks).toEqual([0.4]));
		expect(result.current.waveformError).toBeNull();
		expect(api.waveform).toHaveBeenCalledTimes(2);
	});

	it("seeds imported peaks for their exact asset and isolates show, timecode and revision", async () => {
		const stale = deferred();
		const api = { waveform: vi.fn().mockReturnValue(stale.promise) };
		const initial = { showId: "show", timecodeId: "timecode", audio: first };
		const { result, rerender } = renderHook(
			({ showId, timecodeId, audio }) =>
				useTimecodeWaveform({
					showId,
					timecodeId,
					audio,
					savedAudio: audio,
					api,
				}),
			{ initialProps: initial },
		);
		act(() => result.current.seedWaveform(second, [0.7]));
		expect(result.current.waveformPeaks).toBeUndefined();
		rerender({ ...initial, audio: second });
		expect(result.current.waveformPeaks).toEqual([0.7]);
		await act(async () => stale.resolve({ peaks: [0.2] }));
		expect(result.current.waveformPeaks).toEqual([0.7]);
		rerender({ ...initial, audio: { ...second, asset_revision: 2 } });
		expect(result.current.waveformPeaks).toBeUndefined();
		rerender({ ...initial, showId: "other-show", audio: second });
		expect(result.current.waveformPeaks).toBeUndefined();
		rerender({ ...initial, timecodeId: "other-timecode", audio: second });
		expect(result.current.waveformPeaks).toBeUndefined();
	});
});
