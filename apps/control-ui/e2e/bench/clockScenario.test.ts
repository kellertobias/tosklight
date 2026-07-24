import { describe, expect, it, vi } from "vitest";
import {
	BrowserClock,
	type ClockCheckpointObservation,
	parseClockDuration,
} from "./clockScenario";
import type { DeskDriver } from "./desk";
import type { ClockFrame, ClockFreeRunResult, LightBench } from "./lightBench";

describe("clock duration parsing", () => {
	it.each([
		["0ms", 0],
		["250ms", 250],
		["0s", 0],
		["2s", 2_000],
		["2.5s", 2_500],
		["1.001s", 1_001],
		["1.0010s", 1_001],
	])("parses %s as integer milliseconds", (duration, expected) => {
		expect(parseClockDuration(duration)).toBe(expected);
	});

	it.each([
		"",
		"250",
		" 250ms",
		"250ms ",
		"-1ms",
		"+1s",
		"01s",
		"0.5ms",
		"0.0001s",
		"1m",
		"Infinitys",
	])("rejects non-canonical or sub-millisecond duration %j", (duration) => {
		expect(() => parseClockDuration(duration)).toThrow();
	});
});

describe("BrowserClock", () => {
	it("keeps deterministic step, duration, and free-run operations distinct", async () => {
		const harness = clockHarness();
		await expect(harness.clock.advanceStep()).resolves.toMatchObject({
			now: "step",
		});
		await expect(harness.clock.advanceBy("2.5s")).resolves.toMatchObject({
			now: "advance",
		});
		await expect(harness.clock.freeRunFor("8s")).resolves.toEqual({
			now: "free",
			wall_millis: 8_005,
		});

		expect(harness.tick).toHaveBeenNthCalledWith(1, 0);
		expect(harness.tick).toHaveBeenNthCalledWith(2, 2_500);
		expect(harness.freeRun).toHaveBeenCalledWith(8_000);
		expect(harness.record).toHaveBeenCalledTimes(3);
	});

	it("advances ordered absolute checkpoints by their deltas", async () => {
		const harness = clockHarness();
		const observed: ClockCheckpointObservation[] = [];
		await harness.clock.at(
			[
				{ name: "start", at: "0ms" },
				{ name: "half", at: "1.5s" },
				{ name: "end", at: "3s" },
			],
			(checkpoint) => {
				observed.push(checkpoint);
			},
		);

		expect(harness.tick.mock.calls.map(([millis]) => millis)).toEqual([
			0, 1_500, 1_500,
		]);
		expect(observed.map(({ name, millis }) => [name, millis])).toEqual([
			["start", 0],
			["half", 1_500],
			["end", 3_000],
		]);
	});

	it("rejects unordered and duplicated checkpoints before advancing", async () => {
		const harness = clockHarness();
		await expect(
			harness.clock.at(
				[
					{ name: "end", at: "2s" },
					{ name: "earlier", at: "1s" },
				],
				() => undefined,
			),
		).rejects.toThrow(/must be later/);
		await expect(
			harness.clock.at(
				[
					{ name: "same", at: "1s" },
					{ name: "same", at: "2s" },
				],
				() => undefined,
			),
		).rejects.toThrow(/duplicated/);
		expect(harness.tick).not.toHaveBeenCalled();
	});

	it("enforces the server duration bounds before making a request", async () => {
		const harness = clockHarness();
		await expect(harness.clock.advanceBy("604800.001s")).rejects.toThrow(
			/one week/,
		);
		await expect(harness.clock.freeRunFor("0ms")).rejects.toThrow(
			/greater than zero/,
		);
		await expect(harness.clock.freeRunFor("60.001s")).rejects.toThrow(
			/60 seconds/,
		);
		expect(harness.tick).not.toHaveBeenCalled();
		expect(harness.freeRun).not.toHaveBeenCalled();
	});

	it("uses wall time without ticking the lighting clock", async () => {
		vi.useFakeTimers();
		try {
			const harness = clockHarness();
			const waiting = harness.clock.waitWall("250ms");
			await vi.advanceTimersByTimeAsync(249);
			let settled = false;
			void waiting.then(() => {
				settled = true;
			});
			await Promise.resolve();
			expect(settled).toBe(false);
			await vi.advanceTimersByTimeAsync(1);
			await waiting;
			expect(harness.tick).not.toHaveBeenCalled();
			expect(harness.record).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
	});
});

function clockHarness() {
	const frames = [
		frame("step"),
		frame("advance"),
		frame("checkpoint-1"),
		frame("checkpoint-2"),
		frame("checkpoint-3"),
	];
	const tick = vi.fn(async () => frames.shift() ?? frame("later"));
	const freeRun = vi.fn(
		async (): Promise<ClockFreeRunResult> => ({
			now: "free",
			wall_millis: 8_005,
		}),
	);
	const record = vi.fn(async () => undefined);
	const bench = {
		tick,
		freeRunClock: freeRun,
	} as unknown as LightBench;
	const desk = { recordStep: record } as unknown as DeskDriver;
	return {
		clock: new BrowserClock(bench, desk),
		tick,
		freeRun,
		record,
	};
}

function frame(now: string): ClockFrame {
	return {
		now,
		revision: 1,
		packets_sent: 2,
		universes: [],
	};
}
