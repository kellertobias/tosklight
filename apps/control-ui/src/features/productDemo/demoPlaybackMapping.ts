import type { PlaybackProjection } from "../playbackRuntime/contracts";
import type { ShowObject } from "../showObjects/contracts";

/** Fader strips the demo desk renders, in visible order. */
export const DEMO_PLAYBACK_STRIP_SLOTS = [1, 2, 3, 4] as const;
/** Momentary-only slots rendered above the strips, in visible order. */
export const DEMO_PLAYBACK_TOP_SLOTS = [21, 22, 23, 24] as const;
/** Buttons every strip renders, in visible order. */
export const DEMO_PLAYBACK_STRIP_BUTTONS = [1, 2, 3] as const;

export const DEMO_PLAYBACK_SLOTS: readonly number[] = [
	...DEMO_PLAYBACK_STRIP_SLOTS,
	...DEMO_PLAYBACK_TOP_SLOTS,
];

/**
 * Resolves the demo slots against the portable Page assignments of one exact
 * active desk Page. An absent Page maps nothing, so the surface sends nothing.
 */
export function demoSlotPlaybackNumbers(
	pages: readonly ShowObject<"playback_page">[],
	activePage: number | null,
): ReadonlyMap<number, number> {
	const mapped = new Map<number, number>();
	if (activePage == null) return mapped;
	const page = pages.find((candidate) => candidate.body.number === activePage);
	if (!page) return mapped;
	for (const slot of DEMO_PLAYBACK_SLOTS) {
		const playbackNumber = page.body.slots[String(slot)];
		if (playbackNumber != null) mapped.set(slot, playbackNumber);
	}
	return mapped;
}

/** The exact mapped Playback numbers the demo desk must subscribe to. */
export function demoMappedPlaybackNumbers(
	mapped: ReadonlyMap<number, number>,
): readonly number[] {
	return [...new Set(mapped.values())].sort((left, right) => left - right);
}

/** Mirrors the desk fader reading: authoritative position, then master. */
export function demoFaderLevel(projection: PlaybackProjection | undefined) {
	const runtime =
		projection?.target === "cue_list" ? (projection.runtime ?? null) : null;
	const level = runtime?.fader_position ?? runtime?.master ?? 0;
	return Math.max(0, Math.min(1, level));
}
