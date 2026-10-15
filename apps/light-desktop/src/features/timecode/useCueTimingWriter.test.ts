import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CueList, VersionedObject } from "../../api/types";
import type { TimecodeCueListOption } from "./timecodeEditorShared";
import { useCueTimingWriter } from "./useCueTimingWriter";

function cueList(delayMillis: number): CueList {
	return {
		id: "list-1",
		name: "Main",
		mode: "sequence",
		priority: 100,
		looped: false,
		cues: [
			{
				id: "cue-1",
				number: "1",
				name: "Opening",
				fade_millis: 3_000,
				delay_millis: delayMillis,
				trigger: { type: "manual" },
				changes: [],
			},
		],
	};
}

function options(body = cueList(0)): TimecodeCueListOption[] {
	return [
		{
			id: body.id,
			name: body.name,
			cues: body.cues,
			objectId: "object-1",
			revision: 4,
			body,
		},
	];
}

function saved(body: CueList, revision: number): VersionedObject<CueList> {
	return {
		kind: "cue_list",
		id: "object-1",
		revision,
		updated_at: "",
		body,
	};
}

function drawnDelay(cueLists: TimecodeCueListOption[]): number | undefined {
	return cueLists[0]?.body?.cues[0]?.delay_millis;
}

describe("Cue timing writer", () => {
	it("draws the edit before the desk has answered", async () => {
		let answer: ((object: VersionedObject<CueList>) => void) | undefined;
		const save = vi.fn(
			() =>
				new Promise<VersionedObject<CueList>>((resolve) => {
					answer = resolve;
				}),
		);
		const { result } = renderHook(() => useCueTimingWriter(options(), save));

		act(() => {
			void result.current.save("list-1", cueList(500)).catch(() => undefined);
		});

		expect(drawnDelay(result.current.cueLists)).toBe(500);
		expect(result.current.saving).toBe(true);
		await act(async () => {
			answer?.(saved(cueList(500), 5));
		});
		await waitFor(() => expect(result.current.saving).toBe(false));
		expect(drawnDelay(result.current.cueLists)).toBe(500);
	});

	it("replaces a waiting edit rather than refusing it, and writes where the turn rested", async () => {
		const pending: Array<() => void> = [];
		const save = vi.fn(
			(_basis: unknown, body: CueList) =>
				new Promise<VersionedObject<CueList>>((resolve) => {
					pending.push(() => resolve(saved(body, 5 + pending.length)));
				}),
		);
		const { result } = renderHook(() => useCueTimingWriter(options(), save));

		act(() => {
			void result.current.save("list-1", cueList(100)).catch(() => undefined);
			void result.current.save("list-1", cueList(200)).catch(() => undefined);
			void result.current.save("list-1", cueList(300)).catch(() => undefined);
		});

		expect(drawnDelay(result.current.cueLists)).toBe(300);
		await act(async () => {
			pending[0]?.();
		});
		await waitFor(() => expect(save).toHaveBeenCalledTimes(2));
		// The turn passed through 200 on its way, so only where it rested is written.
		expect(save.mock.calls[1]?.[1].cues[0]?.delay_millis).toBe(300);
	});

	it("writes the second edit against the revision the first one produced", async () => {
		const save = vi.fn(async (_basis: unknown, body: CueList) =>
			saved(body, 5),
		);
		const { result } = renderHook(() => useCueTimingWriter(options(), save));

		await act(async () => {
			await result.current.save("list-1", cueList(100));
		});
		await act(async () => {
			await result.current.save("list-1", cueList(200));
		});

		expect(save.mock.calls[0]?.[0]).toMatchObject({ expectedRevision: 4 });
		expect(save.mock.calls[1]?.[0]).toMatchObject({ expectedRevision: 5 });
	});

	it("reports a refused save and stops drawing ahead", async () => {
		const save = vi.fn(async () => {
			throw new Error("The Cue List moved under you.");
		});
		const { result } = renderHook(() => useCueTimingWriter(options(), save));

		await act(async () => {
			await expect(result.current.save("list-1", cueList(700))).rejects.toThrow(
				"The Cue List moved under you.",
			);
		});

		expect(result.current.error).toBe("The Cue List moved under you.");
		expect(drawnDelay(result.current.cueLists)).toBe(0);
		expect(result.current.saving).toBe(false);
	});
});

it("does not acknowledge coalesced timing saves before the actual final write and rejects all affected callers", async () => {
	let answer: ((value: VersionedObject<CueList>) => void) | undefined;
	let reject: ((reason: unknown) => void) | undefined;
	const save = vi.fn(
		() =>
			new Promise<VersionedObject<CueList>>((resolve, fail) => {
				answer = resolve;
				reject = fail;
			}),
	);
	const { result } = renderHook(() => useCueTimingWriter(options(), save));
	let secondSettled = false;
	let first: Promise<unknown> = Promise.resolve();
	let second: Promise<unknown> = Promise.resolve();
	act(() => {
		first = result.current.save("list-1", cueList(100));
		second = result.current
			.save("list-1", cueList(200))
			.finally(() => {
				secondSettled = true;
			})
			.catch((error: unknown) => error);
	});
	await act(async () => {
		await Promise.resolve();
	});
	expect(secondSettled).toBe(false);
	await act(async () => {
		answer?.(saved(cueList(100), 5));
		await first;
	});
	expect(save).toHaveBeenCalledTimes(2);
	expect(secondSettled).toBe(false);
	await act(async () => {
		reject?.(new Error("stale timing revision"));
		await second;
	});
	expect(secondSettled).toBe(true);
	expect(result.current.error).toBe("stale timing revision");
});

it("restores only changed timing fields while preserving a newer Cue name and unrelated values", async () => {
	const before = cueList(500);
	const after = cueList(200);
	const current = structuredClone(after);
	current.name = "Renamed list";
	current.cues[0].name = "New authored Cue name";
	current.cues[0].fade_millis = 4321;
	const save = vi.fn(async (_basis: unknown, body: CueList) => saved(body, 6));
	const { result } = renderHook(() =>
		useCueTimingWriter(options(current), save),
	);
	await act(async () => {
		await result.current.restore("list-1", after, before);
	});
	expect(save.mock.calls[0]?.[1]).toMatchObject({
		name: "Renamed list",
		cues: [
			{ name: "New authored Cue name", fade_millis: 4321, delay_millis: 500 },
		],
	});
});

it("refuses an intervening edit to the same timing cell without calling the writer", async () => {
	const save = vi.fn();
	const { result } = renderHook(() =>
		useCueTimingWriter(options(cueList(999)), save),
	);
	await expect(
		result.current.restore("list-1", cueList(200), cueList(500)),
	).rejects.toThrow("Cue timings changed");
	expect(save).not.toHaveBeenCalled();
});
