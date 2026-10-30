import { useCallback, useEffect, useRef, useState } from "react";
import type { TimecodeAudioAnalysisSnapshot } from "../../api/client/timecodes";

/** A host without song analysis (a story, an older test double) leaves these out. */
interface AnalysisApi {
	audioAnalysis?(showId: string, timecodeId: string): Promise<TimecodeAudioAnalysisSnapshot>;
	startAudioAnalysis?(showId: string, timecodeId: string): Promise<TimecodeAudioAnalysisSnapshot>;
	cancelAudioAnalysis?(showId: string, timecodeId: string): Promise<TimecodeAudioAnalysisSnapshot>;
}

/** How often a running analysis is re-read; it reports in five-percent steps. */
const RUNNING_REFRESH_MS = 400;

/**
 * The beat grid and suggested sections of a Timecode's saved audio.
 *
 * Reading never starts work: audio the desk has analysed before comes back Ready at once, so
 * reopening a show shows its grid without analysing again. Analysis starts only from `start`.
 */
export function useTimecodeAudioAnalysis({
	showId,
	timecodeId,
	savedAudio,
	api,
}: {
	showId: string | null;
	timecodeId: string;
	savedAudio: { asset_id: string; asset_revision: number } | null | undefined;
	api: AnalysisApi;
}) {
	const [snapshot, setSnapshot] = useState<TimecodeAudioAnalysisSnapshot | null>(null);
	const [error, setError] = useState<string | null>(null);
	const supported = typeof api.audioAnalysis === "function";
	const audioKey = savedAudio && supported ? `${savedAudio.asset_id}:${savedAudio.asset_revision}` : null;
	const current = useRef(audioKey);
	current.current = audioKey;
	const accept = useCallback(
		(key: string | null, next: TimecodeAudioAnalysisSnapshot) => {
			if (current.current === key) setSnapshot(next);
		},
		[],
	);
	useEffect(() => {
		setSnapshot(null);
		setError(null);
		if (!showId || !audioKey || !api.audioAnalysis) return;
		void api.audioAnalysis(showId, timecodeId).then(
			(next) => accept(audioKey, next),
			(reason) => current.current === audioKey && setError(String(reason)),
		);
	}, [accept, api, audioKey, showId, timecodeId]);
	const running = snapshot?.state === "running";
	useEffect(() => {
		if (!running || !showId || !audioKey || !api.audioAnalysis) return;
		const read = api.audioAnalysis.bind(api);
		const timer = setInterval(() => {
			void read(showId, timecodeId).then(
				(next) => accept(audioKey, next),
				(reason) => setError(String(reason)),
			);
		}, RUNNING_REFRESH_MS);
		return () => clearInterval(timer);
	}, [accept, api, audioKey, running, showId, timecodeId]);
	const run = (operation: "start" | "cancel") => {
		if (!showId || !audioKey) return;
		setError(null);
		const call = operation === "start" ? api.startAudioAnalysis : api.cancelAudioAnalysis;
		if (!call) return;
		void call.call(api, showId, timecodeId).then(
			(next) => accept(audioKey, next),
			(reason) => setError(String(reason)),
		);
	};
	return {
		snapshot,
		error,
		available: Boolean(showId && audioKey),
		start: () => run("start"),
		cancel: () => run("cancel"),
	};
}
