import type { PlaybackCardSummary } from "@tosklight/ui/playback";
import type { PlaybackDefinition } from "../../../api/types/playback";
import type { TimecodeTransportSnapshot } from "../../../api/types/timecode";

export function timecodePlaybackSummary(
	playback: PlaybackDefinition | null | undefined,
	summary: PlaybackCardSummary | undefined,
	runtime: {
		snapshots: Map<string, TimecodeTransportSnapshot>;
		error: string | null;
	},
	loadedCueNumber?: string | null,
) {
	if (playback?.target.type !== "timecode")
		return {
			summary: summary && loadedCueNumber != null
				? { ...summary, detail: "LOADED" } : summary,
			running: false,
			error: false,
		};
	const transport = runtime.snapshots.get(playback.target.timecode_id);
	return {
		summary: {
			...summary,
			label: transport?.state.toUpperCase() ??
				(runtime.error ? "Runtime unavailable" : "Loading transport…"),
			detail: transport
				? `Frame ${transport.frame} / ${transport.duration_frame}` : undefined,
		},
		running: transport?.state === "playing" || transport?.state === "paused",
		error: !transport && runtime.error !== null,
	};
}
