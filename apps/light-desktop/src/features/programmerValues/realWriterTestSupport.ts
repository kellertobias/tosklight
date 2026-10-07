import { vi } from "vitest";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import { captureModeSnapshot } from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
} from "../programmerPreloadValues/contracts";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import { preloadSnapshot } from "../programmerPreloadValues/testFixtures";
import { ProgrammerPreloadValuesWriter } from "../programmerPreloadValues/writer";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "./contracts";
import { ProgrammerValuesStore } from "./store";
import { SESSION_ID, SHOW_ID, valuesSnapshot } from "./testFixtures";
import { ProgrammerValuesWriter } from "./writer";

/**
 * The mounted Normal and Preload lane writers over in-memory stores, with a controllable server.
 * Every request waits for the test to answer it (`respond`), so a test can press the next step
 * while the previous request, or its Finish, is still in flight: exactly an operator's rapid
 * key presses against a real desk.
 */
export function inFlightLaneWriter(lane: "normal" | "preload" = "normal") {
	const pending: Array<{
		request: ProgrammerValuesActionRequest | ProgrammerPreloadValuesActionRequest;
		resolve(outcome: unknown): void;
	}> = [];
	const applyAction = vi.fn(
		(_scope: { showId: string }, request: ProgrammerValuesActionRequest) =>
			new Promise<never>((resolve) => {
				pending.push({ request, resolve: resolve as (outcome: unknown) => void });
			}),
	);
	const onError = vi.fn();
	const captureModeStore = new ProgrammerCaptureModeStore();
	captureModeStore.reset(SHOW_ID, SESSION_ID, "session-a");
	captureModeStore.installSnapshot(
		lane === "preload"
			? captureModeSnapshot({ blind: true, preloadCaptureProgrammer: true })
			: captureModeSnapshot(),
	);
	const options = {
		scope: { showId: SHOW_ID },
		captureModeStore,
		applyAction: applyAction as never,
		repair: async () => undefined,
		repairCaptureMode: async () => undefined,
		onError,
	};
	let writer: ProgrammerValuesWriter | ProgrammerPreloadValuesWriter;
	if (lane === "preload") {
		const store = new ProgrammerPreloadValuesStore();
		store.reset(SHOW_ID, SESSION_ID, "session-a");
		store.installSnapshot(preloadSnapshot());
		writer = new ProgrammerPreloadValuesWriter({ ...options, store });
	} else {
		const store = new ProgrammerValuesStore();
		store.reset(SHOW_ID, SESSION_ID, "session-a");
		store.installSnapshot(valuesSnapshot());
		writer = new ProgrammerValuesWriter({ ...options, store });
	}
	const outcome = (requestId: string) =>
		lane === "preload"
			? ({
					requestId,
					correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
					status: "no_change",
					preloadRevision: 1,
					captureModeRevision: 1,
					replayed: false,
					warning: null,
				} as unknown as ProgrammerPreloadValuesActionOutcome)
			: ({
					requestId,
					correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
					status: "no_change",
					revision: 1,
					captureModeRevision: 1,
					replayed: false,
					warning: null,
				} satisfies ProgrammerValuesActionOutcome);
	/** Answers the oldest unanswered request with a quiet `no_change`; false when none waits. */
	const respondNext = () => {
		const next = pending.shift();
		if (!next) return false;
		next.resolve(outcome(next.request.requestId));
		return true;
	};
	/** Answers every request, including the ones the answers release, until the FIFO drains. */
	const drain = async () => {
		for (let turn = 0; turn < 200; turn++) {
			await flush();
			if (!respondNext()) {
				await flush();
				if (!pending.length) return;
			}
		}
		throw new Error("the lane writer never drained");
	};
	/** The actions the server received, in order. */
	const sent = () =>
		applyAction.mock.calls.map(([, request]) => request.action as unknown as Record<string, unknown>);
	return { writer, applyAction, onError, pending, respondNext, drain, sent };
}

export async function flush() {
	for (let turn = 0; turn < 10; turn++)
		await new Promise((resolve) => setTimeout(resolve, 0));
}
