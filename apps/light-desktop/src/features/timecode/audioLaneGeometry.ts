import type { TimecodeLane } from "../../api/types/timecode";

export const VOLUME_VALUE_SPAN = 0.76;

/** Keep the entire volume handle inside its lane at both zero and full level. */
export function volumeKeyframePosition(value: number): number {
	return 88 - Math.max(0, Math.min(1, value)) * (VOLUME_VALUE_SPAN * 100);
}

export function audioVolumeLinePoints(
	keyframes: readonly { frame: number; value: number }[],
	duration: number,
): string {
	return keyframes.map((keyframe) =>
		`${(keyframe.frame / Math.max(1, duration)) * 100},${volumeKeyframePosition(keyframe.value)}`,
	).join(" ");
}

export function volumeDragTravel(laneHeight: number): number {
	return Math.max(1, laneHeight * VOLUME_VALUE_SPAN);
}

/** Resolve each displayed volume point once for its curve position and drag origin. */
export function volumeKeyframeValues(content: TimecodeLane["content"]) {
	return new Map(content.kind === "audio_volume" ? content.keyframes.map((keyframe) => [
		keyframe.id, { value: keyframe.value, position: volumeKeyframePosition(keyframe.value) },
	] as const) : []);
}
