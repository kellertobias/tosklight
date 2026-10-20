import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useSyncExternalStore } from "react";
import type { VisualizationSnapshot } from "../../api/types";
import type { ProgrammerValuesProjection } from "../programmerValues/contracts";
import type { PresetRecallOutcome } from "./contracts";
import { PresetCountFeedbackStore } from "./countFeedback";
import { usePresetCountValues } from "./usePresetCountValues";
import { presetFixtureCounts } from "../presetRecording/presetFixtureCounts";
import { recalledCountValues } from "./countProjection";

const model = vi.hoisted(() => ({
	store: null as PresetCountFeedbackStore | null,
	programmer: null as ProgrammerValuesProjection | null,
	capture: {
		revision: 1,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: false,
	},
}));
vi.mock("./PresetRecallProvider", () => ({
	usePresetCountFeedback: () => ({
		store: model.store,
		feedback: useSyncExternalStore(
			model.store!.subscribe,
			model.store!.getSnapshot,
		),
	}),
}));
vi.mock("../programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesView: () => model.programmer,
}));
vi.mock("../programmerCaptureMode/ProgrammerCaptureModeView", () => ({
	useProgrammerCaptureModeView: () => model.capture,
}));
const groups = new Map<string, readonly string[]>();
function values(
	revision: number,
	entries: Record<string, number>,
): ProgrammerValuesProjection {
	return {
		revision,
		fixtureValues: Object.entries(entries).map(([fixtureId, value], index) => ({
			fixtureId,
			attribute: "intensity",
			value: { kind: "normalized", value },
			programmerOrder: index + 1,
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		})),
		groupValues: [],
	};
}
function accepted(projection: ProgrammerValuesProjection): PresetRecallOutcome {
	return {
		target: "programmer",
		status: "changed",
		disposition: "recalled",
		projection,
	} as PresetRecallOutcome;
}
function output(
	time: number,
	entries: Record<string, number>,
): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: new Date(time).toISOString(),
		grand_master: 1,
		blackout: false,
		values: Object.entries(entries).map(([fixture_id, value]) => ({
			fixture_id,
			attribute: "intensity",
			value: { kind: "normalized", value },
		})),
	};
}
function count(
	resolved: ReturnType<typeof usePresetCountValues>,
	level: number,
) {
	return presetFixtureCounts(
		{
			values: {},
			universal_values: { intensity: { kind: "normalized", value: level } },
		},
		resolved,
		groups,
	).active;
}
beforeEach(() => {
	vi.useFakeTimers();
	model.store = new PresetCountFeedbackStore();
	model.programmer = values(1, { a: 0.2 });
	model.capture = {
		revision: 1,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: false,
	};
});
afterEach(() => {
	cleanup();
	model.store?.stop();
	vi.useRealTimers();
});

it("updates new and previous preset counts on the accepted turn without waiting for a slow frame", () => {
	const baseline = output(1000, { a: 0.2 });
	const { result, rerender } = renderHook(
		({ snapshot }) => usePresetCountValues(snapshot, groups, true),
		{ initialProps: { snapshot: baseline } },
	);
	expect(count(result.current, 0.2)).toBe(1);
	const started = performance.now();
	act(() => {
		const before = model.programmer!;
		model.programmer = values(2, { a: 0.8 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	expect(performance.now() - started).toBe(0);
	expect(count(result.current, 0.8)).toBe(1);
	expect(count(result.current, 0.2)).toBe(0);
	// A frame delivered later can still have been generated before the accepted change.
	rerender({ snapshot: output(1500, { a: 0.2 }) });
	expect(count(result.current, 0.8)).toBe(1);
	expect(model.store!.getSnapshot()).not.toBeNull();
	// Even matching data from before the captured output frame cannot settle the bridge.
	rerender({ snapshot: output(500, { a: 0.8 }) });
	expect(model.store!.getSnapshot()).not.toBeNull();
	rerender({ snapshot: output(2000, { a: 0.8 }) });
	expect(model.store!.getSnapshot()).toBeNull();
	expect(count(result.current, 0.8)).toBe(1);
});

it("preserves rapid accepted recalls to disjoint fixtures until output catches up", () => {
	const baseline = output(1000, { a: 0.2, b: 0.2 });
	model.programmer = values(1, { a: 0.2, b: 0.2 });
	const { result, rerender } = renderHook(
		({ snapshot }) => usePresetCountValues(snapshot, groups, true),
		{ initialProps: { snapshot: baseline } },
	);
	act(() => {
		const before = model.programmer!;
		model.programmer = values(2, { a: 0.8, b: 0.2 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	act(() => {
		const before = model.programmer!;
		model.programmer = values(3, { a: 0.8, b: 0.6 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	expect(count(result.current, 0.8)).toBe(1);
	expect(count(result.current, 0.6)).toBe(1);
	rerender({ snapshot: output(2000, { a: 0.8, b: 0.2 }) });
	expect(count(result.current, 0.6)).toBe(1);
	rerender({ snapshot: output(3000, { a: 0.8, b: 0.6 }) });
	expect(model.store!.getSnapshot()).toBeNull();
	expect(count(result.current, 0.8)).toBe(1);
	expect(count(result.current, 0.6)).toBe(1);
});

it("does not bridge unaccepted, selection-only, Blind, Preview or Preload recalls", () => {
	const before = model.programmer!,
		after = values(2, { a: 0.8 });
	expect(model.store!.getSnapshot()).toBeNull();
	model.store!.accept(before, accepted(after), false);
	model.store!.accept(
		before,
		{ ...accepted(after), target: "preload" } as PresetRecallOutcome,
		true,
	);
	model.store!.accept(
		before,
		{ ...accepted(after), disposition: "targets_selected" },
		true,
	);
	expect(model.store!.getSnapshot()).toBeNull();
});

it("retires feedback on a superseding Programmer revision or bounded expiry", () => {
	const baseline = output(1000, { a: 0.2 });
	const { result, rerender } = renderHook(() =>
		usePresetCountValues(baseline, groups, true),
	);
	act(() => {
		const before = model.programmer!;
		model.programmer = values(2, { a: 0.8 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	expect(count(result.current, 0.8)).toBe(1);
	model.programmer = values(3, { a: 0.6 });
	rerender();
	expect(model.store!.getSnapshot()).toBeNull();
	expect(count(result.current, 0.8)).toBe(0);
	act(() => {
		const before = model.programmer!;
		model.programmer = values(4, { a: 0.8 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	act(() => vi.advanceTimersByTime(5000));
	expect(model.store!.getSnapshot()).toBeNull();
	expect(count(result.current, 0.8)).toBe(0);
});

it("projects overlapping live Groups and fixtures in authoritative LTP order", () => {
	const before = values(1, {});
	const after = values(2, { A: 0.7 });
	after.fixtureValues[0].programmerOrder = 2;
	const projection = {
		...after,
		groupValues: [
			{
				groupId: "g",
				attribute: "intensity",
				value: { kind: "normalized" as const, value: 0.9 },
				programmerOrder: 3,
				fade: false,
				fadeMillis: null,
				delayMillis: null,
			},
		],
	};
	const changed = recalledCountValues(
		{ before, after: projection, baseline: null },
		new Map([["g", ["b", "A"]]]),
	);
	expect([...changed.keys()]).toEqual(["a", "b"]);
	expect(changed.get("a")?.get("intensity")).toEqual({
		kind: "normalized",
		value: 0.9,
	});
});

it.each([
	"blind",
	"preview",
] as const)("does not leave live count feedback visible when %s is enabled", (mode) => {
	const baseline = output(1000, { a: 0.2 });
	const { result, rerender } = renderHook(() =>
		usePresetCountValues(baseline, groups, true),
	);
	act(() => {
		const before = model.programmer!;
		model.programmer = values(2, { a: 0.8 });
		model.store!.accept(before, accepted(model.programmer), true);
	});
	expect(count(result.current, 0.8)).toBe(1);
	model.capture = { ...model.capture, [mode]: true };
	rerender();
	expect(count(result.current, 0.8)).toBe(0);
	expect(model.store!.getSnapshot()).toBeNull();
});
