/**
 * Media In and Out points as operators read and type them: `mm:ss.ff`.
 *
 * The DMX channels stay 16-bit frame counts. The Media Server's frame rate turns a count into a
 * time, so the desk Media pane and the Media Server's own layer page show the same text for the
 * same channel value.
 */

/** The largest count a 16-bit In or Out point channel carries. */
export const MAXIMUM_POINT_FRAMES = 65535;

/** Digits of the frame field: two up to 100 fps, three above. */
function frameDigits(framesPerSecond: number) {
	return framesPerSecond > 100 ? 3 : 2;
}

function wholeRate(framesPerSecond: number) {
	return Math.max(1, Math.round(framesPerSecond));
}

/** `mm:ss.ff` for a frame count at `framesPerSecond`. Minutes grow past 59 rather than wrap. */
export function formatPointTime(frames: number, framesPerSecond: number) {
	const fps = wholeRate(framesPerSecond);
	const count = Math.max(0, Math.min(MAXIMUM_POINT_FRAMES, Math.round(frames)));
	const seconds = Math.floor(count / fps);
	const frame = count % fps;
	const minutes = Math.floor(seconds / 60);
	return `${String(minutes).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}.${String(frame).padStart(frameDigits(fps), "0")}`;
}

export type PointTimeParse =
	| { ok: true; frames: number }
	| { ok: false; error: string };

/**
 * Reads `mm:ss.ff`, `m:ss.ff`, `ss.ff`, `mm:ss`, or plain seconds. Seconds after a minute field
 * stay below 60, and frames stay below the frame rate, so a typo is refused instead of being
 * silently carried into the next field.
 */
export function parsePointTime(
	text: string,
	framesPerSecond: number,
): PointTimeParse {
	const fps = wholeRate(framesPerSecond);
	const match = /^\s*(?:(\d{1,4}):)?(\d{1,5})(?:\.(\d{1,3}))?\s*$/u.exec(text);
	if (!match)
		return {
			ok: false,
			error: `Enter the time as mm:ss.ff, for example 01:30.${String(Math.floor(fps / 2)).padStart(frameDigits(fps), "0")}`,
		};
	const minutes = match[1] === undefined ? 0 : Number(match[1]);
	const seconds = Number(match[2]);
	const frame = match[3] === undefined ? 0 : Number(match[3]);
	if (match[1] !== undefined && seconds >= 60)
		return { ok: false, error: "Seconds must be below 60" };
	if (frame >= fps)
		return {
			ok: false,
			error: `Frames must be below ${fps} at ${fps} fps`,
		};
	const frames = (minutes * 60 + seconds) * fps + frame;
	if (frames > MAXIMUM_POINT_FRAMES)
		return {
			ok: false,
			error: `The longest point is ${formatPointTime(MAXIMUM_POINT_FRAMES, fps)} at ${fps} fps`,
		};
	return { ok: true, frames };
}

/** A whole frame count between 0 and 65535, for entry while no frame rate is known. */
export function parsePointFrames(text: string): PointTimeParse {
	const trimmed = text.trim();
	if (!/^\d+$/u.test(trimmed))
		return { ok: false, error: "Enter a whole number of frames" };
	const frames = Number(trimmed);
	if (frames > MAXIMUM_POINT_FRAMES)
		return {
			ok: false,
			error: `Enter at most ${MAXIMUM_POINT_FRAMES} frames`,
		};
	return { ok: true, frames };
}

/** What a point control shows: the In point from the clip's start, the Out point before its end. */
export function pointDisplay(
	reference: "start" | "end",
	frames: number,
	framesPerSecond: number | null,
) {
	if (reference === "end" && frames === 0) return "End of clip";
	if (framesPerSecond == null)
		return reference === "start"
			? `Frame ${frames}`
			: `${frames} frames before end`;
	const time = formatPointTime(frames, framesPerSecond);
	return reference === "start" ? time : `${time} before end`;
}
