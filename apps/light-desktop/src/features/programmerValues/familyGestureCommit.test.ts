import { describe, expect, it } from "vitest";
import { createZoomGestureSession, scalarSet, scalarStep } from "./familyGestureFamilies";
import type { FamilyGestureTimers } from "./familyGestureSession";
import { flush, inFlightLaneWriter } from "./realWriterTestSupport";
import { FIXTURE_1 } from "./testFixtures";

/**
 * TL-637 follow-up: a discrete step's end keeps its admitted edits on both mounted lane writers.
 * `commit()` and the encoder idle end queue the gesture's unsent edits, in order, then exactly
 * one Finish; `end()` (motion release) and cancels still drop unsent rows (TL-623/625).
 */

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };
const START = { fixtureIds: [FIXTURE_1], timing: TIMING };

function manualTimers() {
	const pending = new Map<number, () => void>();
	let next = 0;
	const timers: FamilyGestureTimers = {
		setTimeout: (callback) => {
			pending.set(++next, callback);
			return next;
		},
		clearTimeout: (handle) => {
			pending.delete(handle as number);
		},
	};
	const fire = () => {
		for (const [handle, callback] of [...pending]) {
			pending.delete(handle);
			callback();
		}
	};
	return { timers, fire };
}

const kinds = (server: ReturnType<typeof inFlightLaneWriter>) =>
	server.sent().map((action) => `${action.action}:${String(action.undoGroup).slice(-1)}`);

describe("discrete gesture steps keep their admitted edits", () => {
	for (const lane of ["normal", "preload"] as const) {
		it(`${lane}: commit queues a step behind the previous step's Finish, never dropping it`, async () => {
			const server = inFlightLaneWriter(lane);
			const session = createZoomGestureSession({ writerFor: () => server.writer });
			const first = session.start({ lane, ...START })!;
			const firstSent = first.change({ zoom: scalarSet(21) })!;
			expect(first.commit()).toBe(true);
			expect(first.commit()).toBe(false);
			expect(first.endReason).toBe("commit");
			// Pressed while the first edit is in flight and its Finish is queued.
			const second = session.start({ lane, ...START })!;
			const secondSent = second.change({ zoom: scalarSet(22) })!;
			second.commit();
			await server.drain();
			await expect(firstSent).resolves.toMatchObject({ status: "no_change" });
			await expect(secondSent).resolves.toMatchObject({ status: "no_change" });
			await expect(second.finished).resolves.toMatchObject({ status: "no_change" });
			expect(server.sent().map((action) => action.action)).toEqual([
				"apply_intent",
				"finish_gesture",
				"apply_intent",
				"finish_gesture",
			]);
			expect(server.sent()[2]).toMatchObject({ undoGroup: second.undoGroup });
			expect(server.sent()[3]).toMatchObject({ undoGroup: second.undoGroup });
			expect(server.onError.mock.calls.filter(([error]) => error instanceof Error)).toEqual([]);
		});

		it(`${lane}: the encoder idle end keeps every queued detent, in order`, async () => {
			const server = inFlightLaneWriter(lane);
			const clock = manualTimers();
			let id = 0;
			const session = createZoomGestureSession({
				writerFor: () => server.writer,
				createId: () => `id-${++id}`,
				timers: clock.timers,
			});
			const gesture = session.start({ lane, ...START, idleEndMillis: 250 })!;
			for (let detent = 0; detent < 3; detent++) gesture.change({ zoom: scalarStep(1) });
			await flush();
			clock.fire();
			expect(gesture.endReason).toBe("idle");
			await server.drain();
			expect(kinds(server)).toEqual([
				`apply_intent:${gesture.undoGroup.slice(-1)}`,
				`apply_intent:${gesture.undoGroup.slice(-1)}`,
				`apply_intent:${gesture.undoGroup.slice(-1)}`,
				`finish_gesture:${gesture.undoGroup.slice(-1)}`,
			]);
		});

		it(`${lane}: a motion release still drops the gesture's stale unsent samples`, async () => {
			const server = inFlightLaneWriter(lane);
			const session = createZoomGestureSession({ writerFor: () => server.writer });
			const drag = session.start({ lane, ...START })!;
			drag.change({ zoom: scalarSet(21) });
			const stale = drag.change({ zoom: scalarSet(22) })!;
			drag.end();
			await expect(stale).resolves.toBeNull();
			await server.drain();
			expect(server.sent().map((action) => action.action)).toEqual([
				"apply_intent",
				"finish_gesture",
			]);
		});
	}
});
