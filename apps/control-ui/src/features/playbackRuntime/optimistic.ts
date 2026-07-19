import type { PlaybackProjection } from "./contracts";

export function optimisticMaster(
	projection: PlaybackProjection,
	value: number,
): PlaybackProjection | null {
	if (projection.target === "cue_list" && projection.runtime)
		return {
			...projection,
			runtime: { ...projection.runtime, master: value, fader_position: value },
		};
	if (projection.target === "group") return { ...projection, master: value };
	if (projection.target === "grand_master")
		return {
			...projection,
			runtime: {
				...projection.runtime,
				level: value,
				effective_level: projection.runtime.blackout ? 0 : value,
			},
		};
	return null;
}
