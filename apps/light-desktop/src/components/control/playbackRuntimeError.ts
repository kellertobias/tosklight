import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../api/types";

/** Missing controllers are normal while Off; only authoritative failures are errors. */
export function playbackHasRuntimeError(
	playback: PlaybackDefinition | null | undefined,
	projection: PlaybackRuntimeProjection | undefined,
	boundCueListExists?: boolean,
): boolean {
	if (!playback || !projection) return false;
	if (projection.target === "missing") return true;
	if (playback.target.type === "cue_list" && boundCueListExists === false)
		return true;
	if (projection.target !== "dynamic" || !projection.runtime) return false;
	return (
		projection.runtime.state === "failed" ||
		projection.runtime.missing_target_count > 0
	);
}
