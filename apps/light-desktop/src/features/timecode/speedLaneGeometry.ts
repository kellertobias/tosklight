import type { TimecodeDefinition } from "../../api/types/timecode";

type SpeedKeyframe = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "speed_group" }
>["keyframes"][number];

export function speedKeyframePositions(
	keyframes: readonly SpeedKeyframe[],
): Map<string, number> {
	if (!keyframes.length) return new Map();
	const bpms = keyframes.map((keyframe) => keyframe.bpm);
	const minimum = Math.min(...bpms);
	const maximum = Math.max(...bpms);
	const span = maximum - minimum;
	return new Map(
		keyframes.map((keyframe) => [
			keyframe.id,
			span === 0 ? 50 : 88 - ((keyframe.bpm - minimum) / span) * 76,
		]),
	);
}

export function speedGroupLinePoints(
	keyframes: readonly SpeedKeyframe[],
	durationFrames: number,
): string {
	const sorted = [...keyframes].sort((left, right) => left.frame - right.frame);
	const first = sorted[0];
	if (!first) return "";
	const positions = speedKeyframePositions(sorted);
	const duration = Math.max(1, durationFrames);
	const point = (frame: number, keyframe: SpeedKeyframe) =>
		`${Math.max(0, Math.min(100, (frame / duration) * 100))},${positions.get(keyframe.id) ?? 50}`;
	const points = [point(0, first), point(first.frame, first)];
	for (let index = 1; index < sorted.length; index += 1) {
		const previous = sorted[index - 1];
		const current = sorted[index];
		if (!previous || !current) continue;
		points.push(point(current.frame, previous), point(current.frame, current));
	}
	const last = sorted.at(-1);
	if (last) points.push(point(duration, last));
	return points.join(" ");
}
