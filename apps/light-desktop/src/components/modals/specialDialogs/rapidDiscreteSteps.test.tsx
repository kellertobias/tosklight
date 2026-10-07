import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FIXTURE_1 } from "../../../features/programmerValues/testFixtures";
import { inFlightLaneWriter } from "../../../features/programmerValues/realWriterTestSupport";
import type { ParameterValuesMutationPort } from "../../control/parameterControls/parameterValueMutations";
import type { RangeGesture } from "./intention/HorizontalRangeFader";
import type { PositionGesture } from "./intention/PositionDialog";
import { CORE_COLOR_DESCRIPTORS } from "./intention/color/colorDialogModel";
import type { ColorDialogLane } from "./intention/color/useColorDialogLane";
import { useColorGestures } from "./intention/color/useColorGestures";
import {
	positionDraftValue,
	usePositionDialogGestures,
} from "./semanticPosition/usePositionDialogGestures";

/**
 * TL-637 follow-up, the shared cause of dropped rapid steps: a discrete step (key, button,
 * native assistive step) is one complete gesture. Pressed while the previous step or its Finish
 * is still in flight on the mounted lane writer, it queues behind it in order; it is never
 * dropped as an "unsent row" of its own end. Drags and a held joystick keep their motion
 * semantics (unsent samples stop with the release).
 */

const TIMING = { fade: false, fadeMillis: null, delayMillis: null };

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

const actions = (server: ReturnType<typeof inFlightLaneWriter>) =>
	server.sent().map((action) => action.action);

describe("Position Special Dialog: rapid key and button steps", () => {
	function mount(server: ReturnType<typeof inFlightLaneWriter>) {
		return renderHook(
			({ pan }: { pan: string }) =>
				usePositionDialogGestures(
					{
						lane: "normal",
						fixtureIds: [FIXTURE_1],
						groupId: null,
						timing: TIMING,
						representation: "angles",
						modes: { pan: "absolute", tilt: "absolute" },
						keys: { pan, tilt: "tilt" },
					},
					{ writerFor: () => server.writer },
				),
			{ initialProps: { pan: "pan@10" } },
		);
	}

	it("three fast Pan steps are all sent in order, each finished once", async () => {
		const server = inFlightLaneWriter();
		const { result } = mount(server);
		const step = (id: number, pan: number, source: PositionGesture["source"]) => {
			const gesture: PositionGesture = { id, control: "pan", source, initialPan: pan - 1, initialTilt: 0 };
			act(() => {
				result.current.callbacks.onGestureStart?.(gesture);
				result.current.callbacks.onChange({ pan }, gesture);
				result.current.callbacks.onGestureEnd?.(gesture, { changed: true });
			});
		};
		step(1, 11, "keyboard");
		step(2, 12, "keyboard");
		step(3, 102, "button");
		await act(server.drain);
		expect(actions(server)).toEqual([
			"apply_intent",
			"finish_gesture",
			"apply_intent",
			"finish_gesture",
			"apply_intent",
			"finish_gesture",
		]);
		const pans = server
			.sent()
			.filter((action) => action.action === "apply_intent")
			.map((action) => (action.operation as { edits: Array<{ operation: unknown }> }).edits.at(-1)?.operation);
		expect(pans).toEqual([
			{ kind: "set", value: { kind: "value", value: 11 } },
			{ kind: "set", value: { kind: "value", value: 12 } },
			{ kind: "set", value: { kind: "value", value: 102 } },
		]);
	});

	it("a reflection of an older step mid-burst keeps showing the latest requested step", async () => {
		const server = inFlightLaneWriter();
		const { result, rerender } = mount(server);
		for (const [id, pan] of [[1, 11], [2, 12]] as const) {
			const gesture: PositionGesture = { id, control: "pan", source: "keyboard", initialPan: pan - 1, initialTilt: 0 };
			act(() => {
				result.current.callbacks.onGestureStart?.(gesture);
				result.current.callbacks.onChange({ pan }, gesture);
				result.current.callbacks.onGestureEnd?.(gesture, { changed: true });
			});
		}
		// The store reflects step 1 (11°) while step 2 (12°) is unanswered: still 12° shown.
		rerender({ pan: "pan@11" });
		expect(positionDraftValue(result.current.draft, "pan", "pan@11", 11)).toBe(12);
		await act(server.drain);
		// Every step answered: the reflected model value is authoritative again.
		rerender({ pan: "pan@12" });
		expect(positionDraftValue(result.current.draft, "pan", "pan@12", 12)).toBe(12);
		rerender({ pan: "pan@15" });
		expect(positionDraftValue(result.current.draft, "pan", "pan@15", 15)).toBe(15);
	});

	it("Undo of a settled step back to its basis shows the model, not the old step", async () => {
		const server = inFlightLaneWriter();
		const { result, rerender } = mount(server);
		const gesture: PositionGesture = { id: 1, control: "pan", source: "button", initialPan: 10, initialTilt: 0 };
		act(() => {
			result.current.callbacks.onGestureStart?.(gesture);
			result.current.callbacks.onChange({ pan: 100 }, gesture);
			result.current.callbacks.onGestureEnd?.(gesture, { changed: true });
		});
		await act(server.drain);
		rerender({ pan: "pan@100" });
		expect(positionDraftValue(result.current.draft, "pan", "pan@100", 100)).toBe(100);
		// UND restores the Programmer to the step's basis: the dialog follows the model.
		rerender({ pan: "pan@10" });
		expect(positionDraftValue(result.current.draft, "pan", "pan@10", 10)).toBe(10);
	});
	it("Undo that lands while the step is still settling shows the model once it settles", async () => {
		const server = inFlightLaneWriter();
		const { result, rerender } = mount(server);
		const gesture: PositionGesture = { id: 1, control: "pan", source: "button", initialPan: 10, initialTilt: 0 };
		act(() => {
			result.current.callbacks.onGestureStart?.(gesture);
			result.current.callbacks.onChange({ pan: 100 }, gesture);
			result.current.callbacks.onGestureEnd?.(gesture, { changed: true });
		});
		// The step is reflected, then undone, before its requests are all answered.
		rerender({ pan: "pan@100" });
		rerender({ pan: "pan@10" });
		await act(server.drain);
		expect(positionDraftValue(result.current.draft, "pan", "pan@10", 10)).toBe(10);
	});
	it("a pointer drag released behind an in-flight edit still drops its stale unsent sample", async () => {
		const server = inFlightLaneWriter();
		const { result } = mount(server);
		const drag: PositionGesture = { id: 1, control: "pan", source: "pointer", pointerId: 4, initialPan: 0, initialTilt: 0 };
		act(() => {
			result.current.callbacks.onGestureStart?.(drag);
			result.current.callbacks.onChange({ pan: 5 }, drag);
			result.current.callbacks.onChange({ pan: 9 }, drag);
			result.current.callbacks.onGestureEnd?.(drag, { changed: true });
		});
		await act(server.drain);
		expect(actions(server)).toEqual(["apply_intent", "finish_gesture"]);
	});
});

describe("Color Special Dialog: rapid keyboard steps", () => {
	it("three fast fader key steps are all sent in order, each finished once", async () => {
		const server = inFlightLaneWriter();
		const port = server.writer as unknown as ParameterValuesMutationPort;
		const lane: ColorDialogLane = {
			lane: "normal",
			ready: true,
			fixtureIds: [FIXTURE_1],
			groupId: null,
			timing: TIMING,
			descriptors: CORE_COLOR_DESCRIPTORS,
			colorFixtureIds: [FIXTURE_1],
			variant: "lamp",
			values: [],
			writers: { normal: port, preload: null },
		};
		const { result } = renderHook(() => useColorGestures(lane, () => undefined));
		for (const value of [0.1, 0.2, 0.3]) {
			act(() => {
				const started: RangeGesture = { control: "White Blend", source: "keyboard", shifted: false };
				const id = result.current.onGestureStart?.(started);
				const gesture = { ...started, ...(id === undefined ? {} : { id }) };
				result.current.change(gesture, [
					{ component: "white_blend", operation: { kind: "set", value: { kind: "value", value } } },
				]);
				result.current.onGestureEnd?.(gesture);
			});
		}
		await act(server.drain);
		expect(actions(server)).toEqual([
			"apply_intent",
			"finish_gesture",
			"apply_intent",
			"finish_gesture",
			"apply_intent",
			"finish_gesture",
		]);
	});

	it("reports settling while key steps are unanswered, so a reflection keeps the draft", async () => {
		const server = inFlightLaneWriter();
		const port = server.writer as unknown as ParameterValuesMutationPort;
		const lane: ColorDialogLane = {
			lane: "normal",
			ready: true,
			fixtureIds: [FIXTURE_1],
			groupId: null,
			timing: TIMING,
			descriptors: CORE_COLOR_DESCRIPTORS,
			colorFixtureIds: [FIXTURE_1],
			variant: "lamp",
			values: [],
			writers: { normal: port, preload: null },
		};
		const { result } = renderHook(() => useColorGestures(lane, () => undefined));
		expect(result.current.settling).toBe(false);
		act(() => {
			const started: RangeGesture = { control: "White Blend", source: "keyboard", shifted: false };
			const id = result.current.onGestureStart?.(started);
			const gesture = { ...started, ...(id === undefined ? {} : { id }) };
			result.current.change(gesture, [
				{ component: "white_blend", operation: { kind: "set", value: { kind: "value", value: 0.4 } } },
			]);
			result.current.onGestureEnd?.(gesture);
		});
		expect(result.current.settling).toBe(true);
		await act(server.drain);
		expect(result.current.settling).toBe(false);
	});
});

