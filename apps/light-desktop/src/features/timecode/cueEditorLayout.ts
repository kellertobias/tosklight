/** One compact vertical rhythm shared by Cue envelopes, drag controls, and name strips. */
export const HEADER = 24;
export const ENVELOPE = 88;
export const NAME = 44;
export const NAME_TOP = HEADER + ENVELOPE;
export const DIVIDER_CENTER = NAME_TOP + NAME / 2;
export const END_GRIP_CENTER = DIVIDER_CENTER;
export const EXPANDED_HEIGHT = NAME_TOP + NAME + 6;
export const PEAK_Y = HEADER + 8;
export const BASE_Y = HEADER + ENVELOPE - 8;

/** Manual Cue boundaries reserve their actual name-band touch targets. */
export function cueBoundaryFrames(
	rows: readonly { startFrame: number; transition?: string }[],
): number[] {
	return [
		...rows
			.filter((row, index) => index === 0 || row.transition)
			.map((row) => row.startFrame),
	];
}
