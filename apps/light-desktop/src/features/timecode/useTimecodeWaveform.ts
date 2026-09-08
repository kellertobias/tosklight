import { useCallback, useEffect, useRef, useState } from "react";
import type {
	TimecodeAudio,
	TimecodeAudioWaveform,
} from "../../api/types/timecode";

interface WaveformApi {
	waveform(showId: string, timecodeId: string): Promise<TimecodeAudioWaveform>;
}

function assetKey(
	showId: string | null,
	timecodeId: string,
	audio?: TimecodeAudio | null,
) {
	return showId && audio
		? JSON.stringify([showId, timecodeId, audio.asset_id, audio.asset_revision])
		: null;
}

/** The server waveform route reads the saved Timecode, so wait for that audio to match. */
export function useTimecodeWaveform({
	showId,
	timecodeId,
	audio,
	savedAudio,
	api,
}: {
	showId: string | null;
	timecodeId: string;
	audio?: TimecodeAudio | null;
	savedAudio?: TimecodeAudio | null;
	api: WaveformApi;
}) {
	const key = assetKey(showId, timecodeId, audio);
	const savedKey = assetKey(showId, timecodeId, savedAudio);
	const cache = useRef(new Map<string, readonly number[]>());
	const [seedVersion, setSeedVersion] = useState(0);
	const [attempt, setAttempt] = useState(0);
	const [request, setRequest] = useState<{
		key: string;
		loading: boolean;
		error: string | null;
	} | null>(null);
	useEffect(() => {
		if (!showId || !key || key !== savedKey || cache.current.has(key)) return;
		let cancelled = false;
		setRequest({ key, loading: true, error: null });
		void api.waveform(showId, timecodeId).then(
			(waveform) => {
				if (cancelled) return;
				cache.current.set(key, waveform.peaks);
				setRequest({ key, loading: false, error: null });
			},
			(reason) => {
				if (cancelled) return;
				setRequest({
					key,
					loading: false,
					error: `Waveform could not be loaded: ${reason instanceof Error ? reason.message : String(reason)}`,
				});
			},
		);
		return () => {
			cancelled = true;
		};
	}, [api, showId, timecodeId, key, savedKey, attempt, seedVersion]);

	const seedWaveform = useCallback(
		(asset: TimecodeAudio, peaks: readonly number[] | undefined) => {
			const importedKey = assetKey(showId, timecodeId, asset);
			if (!importedKey || !peaks) return;
			cache.current.set(importedKey, [...peaks]);
			setSeedVersion((value) => value + 1);
		},
		[showId, timecodeId],
	);
	const retryWaveform = useCallback(() => setAttempt((value) => value + 1), []);
	const waveformPeaks = key ? cache.current.get(key) : undefined;
	return {
		waveformPeaks,
		waveformError:
			!waveformPeaks && request?.key === key ? request.error : null,
		waveformLoading: Boolean(
			key && !waveformPeaks && (request?.key !== key || request.loading),
		),
		seedWaveform,
		retryWaveform,
	};
}
