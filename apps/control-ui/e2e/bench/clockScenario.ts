import type { DeskDriver } from "./desk";
import type { ClockFrame, ClockFreeRunResult, LightBench } from "./lightBench";

export type ClockDuration = `${number}ms` | `${number}s`;

export interface ClockCheckpoint {
	readonly name: string;
	/** Absolute offset from the start of `clock.at`, not a delta. */
	readonly at: ClockDuration;
}

export interface ClockCheckpointObservation {
	readonly name: string;
	readonly at: ClockDuration;
	readonly millis: number;
	readonly frame: ClockFrame;
}

export type ClockCheckpointCallback = (
	checkpoint: ClockCheckpointObservation,
) => Promise<void> | void;

const MAX_ADVANCE_MILLIS = 604_800_000;
const MAX_FREE_RUN_MILLIS = 60_000;

/** Operator-scenario clock controls. Lighting time and deliberate wall waits stay separate. */
export class BrowserClock {
	constructor(
		private readonly bench: LightBench,
		private readonly desk: DeskDriver,
	) {}

	/** Renders exactly one deterministic frame without moving application time. */
	async advanceStep(): Promise<ClockFrame> {
		await this.record(
			"CLOCK STEP",
			"Render one deterministic engine frame without advancing lighting time.",
		);
		return this.bench.tick(0);
	}

	/** Advances application time once by the exact duration and renders one frame. */
	async advanceBy(duration: ClockDuration): Promise<ClockFrame> {
		const millis = parseClockDuration(duration);
		if (millis > MAX_ADVANCE_MILLIS)
			throw new Error("Clock advance duration cannot exceed one week");
		await this.record(
			"CLOCK ADVANCE",
			`Advance lighting time by ${duration} and render the exact resulting frame.`,
		);
		return this.bench.tick(millis);
	}

	/**
	 * Keeps the server's test scheduler and browser rendering live for the complete wall interval.
	 * The request resolves only after the server freezes the manual clock again.
	 */
	async freeRunFor(duration: ClockDuration): Promise<ClockFreeRunResult> {
		const millis = parseClockDuration(duration);
		if (millis === 0)
			throw new Error("Clock free-run duration must be greater than zero");
		if (millis > MAX_FREE_RUN_MILLIS)
			throw new Error("Clock free-run duration cannot exceed 60 seconds");
		await this.record(
			"CLOCK FREE RUN",
			`Run lighting effects and browser rendering live for ${duration}.`,
		);
		return this.bench.freeRunClock(millis);
	}

	/**
	 * Visits strictly ordered absolute offsets. The first zero checkpoint still renders one frame;
	 * later checkpoints advance only by the delta from the preceding absolute offset.
	 */
	async at(
		checkpoints: readonly ClockCheckpoint[],
		callback: ClockCheckpointCallback,
	): Promise<void> {
		const normalized = normalizeClockCheckpoints(checkpoints);
		let previousMillis = 0;
		for (const checkpoint of normalized) {
			const delta = checkpoint.millis - previousMillis;
			await this.record(
				`CLOCK · ${checkpoint.name}`,
				delta === 0
					? `Render the ${checkpoint.name} boundary at ${checkpoint.at}.`
					: `Advance ${delta} ms to the absolute ${checkpoint.name} boundary at ${checkpoint.at}.`,
			);
			const frame = await this.bench.tick(delta);
			await callback({ ...checkpoint, frame });
			previousMillis = checkpoint.millis;
		}
	}

	/**
	 * Waits for browser gesture mechanics only. This never advances the lighting application clock.
	 */
	async waitWall(duration: ClockDuration): Promise<void> {
		const millis = parseClockDuration(duration);
		await new Promise<void>((resolve) => setTimeout(resolve, millis));
	}

	private async record(title: string, description: string): Promise<void> {
		await this.desk.recordStep(title, description);
	}
}

export function parseClockDuration(duration: ClockDuration | string): number {
	if (typeof duration !== "string")
		throw new Error("Clock duration must be a string ending in ms or s");
	const match = /^(0|[1-9]\d*)(?:\.(\d+))?(ms|s)$/.exec(duration);
	if (!match)
		throw new Error(
			`Invalid clock duration "${duration}"; use integer milliseconds or seconds such as "250ms" or "2.5s"`,
		);
	const [, wholeText, fraction = "", unit] = match;
	if (unit === "ms" && fraction)
		throw new Error(
			`Invalid clock duration "${duration}"; millisecond values must be integers`,
		);
	let millis = BigInt(wholeText);
	if (unit === "s") {
		millis *= 1_000n;
		const milliseconds = fraction.slice(0, 3).padEnd(3, "0");
		const subMilliseconds = fraction.slice(3);
		if (subMilliseconds && /[1-9]/.test(subMilliseconds))
			throw new Error(
				`Invalid clock duration "${duration}"; duration must resolve to whole milliseconds`,
			);
		millis += BigInt(milliseconds || "0");
	}
	if (millis > BigInt(Number.MAX_SAFE_INTEGER))
		throw new Error(
			`Clock duration "${duration}" exceeds safe integer milliseconds`,
		);
	return Number(millis);
}

function normalizeClockCheckpoints(
	checkpoints: readonly ClockCheckpoint[],
): Array<ClockCheckpoint & { millis: number }> {
	const names = new Set<string>();
	let previousMillis = -1;
	return checkpoints.map((checkpoint, index) => {
		const name = checkpoint.name.trim();
		if (!name)
			throw new Error(`Clock checkpoint ${index + 1} must have a name`);
		if (names.has(name))
			throw new Error(`Clock checkpoint name "${name}" is duplicated`);
		names.add(name);
		const millis = parseClockDuration(checkpoint.at);
		if (millis <= previousMillis)
			throw new Error(
				`Clock checkpoint "${name}" at ${checkpoint.at} must be later than the preceding checkpoint`,
			);
		if (millis - Math.max(previousMillis, 0) > MAX_ADVANCE_MILLIS)
			throw new Error(
				`Clock checkpoint "${name}" is more than one week after the preceding checkpoint`,
			);
		previousMillis = millis;
		return { name, at: checkpoint.at, millis };
	});
}
