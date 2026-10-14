import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../api/familyEncoderModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import { inFlightLaneWriter, flush } from "../../../../features/programmerValues/realWriterTestSupport";
import type { ParameterProjection } from "../useParameterProjection";
import { FIXTURE_A, FIXTURE_B, FOCUS, ZOOM, fakeWriter, pagesSnapshot } from "./familyEncoderTestSupport";
import { useFamilyEncoderBinding } from "./useFamilyEncoderBinding";

/**
 * TL-594 lease follow-up: an encoder gesture names the newest lease whose delivery covered the
 * edited slot's fixtures, not merely the lane's newest lease (another consumer's read of other
 * fixtures). A Preload capture holds an edit for a member its lease never delivered.
 */

const FIXTURE_C = "33333333-3333-4333-8333-333333333333";

const writers = vi.hoisted(() => ({ normal: null as unknown, preload: null as unknown }));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => writers.normal,
}));
vi.mock("../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView", () => ({
	useProgrammerPreloadValuesActions: () => writers.preload,
}));

afterEach(() => {
	writers.normal = null;
	writers.preload = null;
});

function leased(
	lane: "normal" | "preload",
	lease: number,
	fixtureIds: readonly string[],
): OutputReadoutSnapshot {
	return {
		lane,
		scope: { show_id: null },
		revision: 1,
		lease,
		owners: fixtureIds.map((fixture_id) => ({
			fixture_id,
			position: { available: false, commands: [] },
		})),
	} as unknown as OutputReadoutSnapshot;
}

function projection(route: "normal" | "preload"): ParameterProjection {
	return {
		active: true,
		selectedFixtureIds: [FIXTURE_A, FIXTURE_B],
		selectedFixtures: [],
		selectedGroupId: null,
		supportedFixtureIdsByAttribute: new Map([
			["focus", [FIXTURE_A, FIXTURE_B]],
			["zoom", [FIXTURE_A, FIXTURE_B]],
			["softness", [FIXTURE_A, FIXTURE_B]],
		]),
		encoderGroups: [],
		encoderPage: 1,
		encoderPageCount: 1,
		encoderSlots: ["focus", "zoom", "softness", null],
		encoderPushTurnSlots: [null, null, null, null],
		visibleEncoderCount: 4,
		programmerValuesRoute: route,
		programmerValuesReady: true,
		programmerValues: [],
		programmerFadeMillis: 3_000,
		attributeUnits: new Map(),
		normalized: new Map(),
		discrete: new Map(),
	} as unknown as ParameterProjection;
}

describe("family encoders name the covering displayed-source lease", () => {
	for (const lane of ["normal", "preload"] as const) {
		it(`${lane}: a Zoom detent names the lease that delivered the slot's fixtures`, async () => {
			const writer = fakeWriter();
			if (lane === "preload") writers.preload = writer;
			else writers.normal = writer;
			const readouts = new DisplayedSourceReadouts({ request: async () => leased(lane, 3, [FIXTURE_A, FIXTURE_B]) });
			// The encoders were shown lease 3 for A and B; another consumer then read C (lease 4).
			readouts.observe(leased(lane, 3, [FIXTURE_A, FIXTURE_B]));
			readouts.observe(leased(lane, 4, [FIXTURE_C]));
			const value: FamilyEncodersContextValue = {
				loadPages: async () => pagesSnapshot(true),
				readouts,
				session: null,
			};
			const wrapper = ({ children }: { children: ReactNode }) => (
				<FamilyEncodersContextProvider value={value}>{children}</FamilyEncodersContextProvider>
			);
			const view = projection(lane);
			const { result } = renderHook(() => useFamilyEncoderBinding(view, "Focus"), { wrapper });
			await waitFor(() => expect(result.current.componentSlot(1)?.component.kind).toBe("zoom"));
			act(() => result.current.step(1, 1));
			expect(writer.applyIntent).toHaveBeenCalledWith(
				expect.objectContaining({ attribute: "zoom", displayedSource: { lane, lease: 3 } }),
			);
		});
	}
});

it("re-reads a held Zoom lease while Focus encoders remain visible, without retrying the refused edit", async () => {
	const server = inFlightLaneWriter();
	writers.normal = server.writer;
	const request = vi.fn(async () =>
		leased("normal", 3, [FIXTURE_A, FIXTURE_B]),
	);
	const readouts = new DisplayedSourceReadouts({ request });
	readouts.observe(leased("normal", 3, [FIXTURE_A, FIXTURE_B]));
	const value: FamilyEncodersContextValue = {
		loadPages: async () => pagesSnapshot(true),
		readouts,
		session: null,
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const view = projection("normal");
	const { result } = renderHook(() => useFamilyEncoderBinding(view, "Focus"), {
		wrapper,
	});
	await waitFor(() =>
		expect(result.current.componentSlot(1)?.component.kind).toBe("zoom"),
	);
	await flush();
	const initialReads = request.mock.calls.length;
	request.mockImplementation(async () =>
		leased("normal", 5, [FIXTURE_A, FIXTURE_B]),
	);
	// An expired source refuses the edit. The current encoder family must explicitly refresh.
	act(() => result.current.set(1, 40));
	await flush();
	const first = server.pending.shift();
	if (!first) throw new Error("the Zoom request did not reach the writer");
	expect(first.request.action).toMatchObject({
		attribute: "zoom",
		displayedSource: { lane: "normal", lease: 3 },
	});
	first.resolve({
		requestId: first.request.requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change",
		revision: 1,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
		hold: "displayed_source_unavailable",
	});
	await server.drain();
	await waitFor(() => expect(request).toHaveBeenCalledTimes(initialReads + 1));
	await waitFor(() =>
		expect(readouts.displayedSource("normal", [FIXTURE_A, FIXTURE_B])).toEqual({
			lane: "normal",
			lease: 5,
		}),
	);
	expect(server.sent().map((action) => action.action)).toEqual([
		"apply_intent",
		"finish_gesture",
	]);
	// Only a second, explicit operator confirmation sends the new lease. No retry is invented.
	act(() => result.current.set(1, 40));
	await server.drain();
	expect(server.sent()[2]).toMatchObject({
		attribute: "zoom",
		displayedSource: { lane: "normal", lease: 5 },
	});
	server.writer.stop();
});

it("claims only visible Zoom fixtures, follows fresh stream leases, and releases on hide", async () => {
	const writer = fakeWriter();
	writers.normal = writer;
	const request = vi.fn(async () =>
		leased("normal", 3, [FIXTURE_A, FIXTURE_B]),
	);
	const readouts = new DisplayedSourceReadouts({ request });
	const release = vi.fn();
	let deliver: ((snapshot: OutputReadoutSnapshot) => void) | null = null;
	const claimReadouts = vi.fn(
		(
			_ids: readonly string[],
			onReadout: (snapshot: OutputReadoutSnapshot) => void,
		) => {
			deliver = onReadout;
			return release;
		},
	);
	const value: FamilyEncodersContextValue = {
		loadPages: async () => pagesSnapshot(true),
		readouts,
		session: {
			claimReadouts,
		} as unknown as FamilyEncodersContextValue["session"],
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const view = projection("normal");
	const { result, rerender } = renderHook(
		({ active }) => useFamilyEncoderBinding({ ...view, active }, "Focus"),
		{ wrapper, initialProps: { active: true } },
	);
	await waitFor(() => expect(claimReadouts).toHaveBeenCalledOnce());
	expect(claimReadouts.mock.calls[0]?.[0]).toEqual([FIXTURE_A, FIXTURE_B]);
	act(() => deliver?.(leased("normal", 5, [FIXTURE_A, FIXTURE_B])));
	act(() => result.current.set(1, 40));
	expect(writer.applyIntent).toHaveBeenCalledWith(
		expect.objectContaining({
			attribute: "zoom",
			displayedSource: { lane: "normal", lease: 5 },
		}),
	);
	rerender({ active: false });
	expect(release).toHaveBeenCalledOnce();
});

it("does not claim output for a Focus-only layout without Zoom", async () => {
	const snapshot = pagesSnapshot(true);
	const focus = snapshot.families.find((family) => family.family === "focus");
	if (!focus) throw new Error("the fixture has no Focus family");
	for (const page of focus.pages)
		page.slots = page.slots.map((slot) =>
			slot?.kind === "component" && slot.component.kind === "zoom"
				? null
				: slot,
		);
	const request = vi.fn();
	const claimReadouts = vi.fn();
	const value: FamilyEncodersContextValue = {
		loadPages: async () => snapshot,
		readouts: new DisplayedSourceReadouts({ request }),
		session: {
			claimReadouts,
		} as unknown as FamilyEncodersContextValue["session"],
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const { result } = renderHook(
		() => useFamilyEncoderBinding(projection("normal"), "Focus"),
		{ wrapper },
	);
	await waitFor(() =>
		expect(result.current.componentSlot(0)?.component.kind).toBe("focus"),
	);
	expect(request).not.toHaveBeenCalled();
	expect(claimReadouts).not.toHaveBeenCalled();
});


it.each([
	"normal",
	"preload",
] as const)("%s: claims only the current encoder page and releases Zoom when switching to Focus-only", async (lane) => {
	const snapshot = pagesSnapshot(true);
	const focus = snapshot.families.find((family) => family.family === "focus");
	if (!focus) throw new Error("the fixture has no Focus family");
	focus.pages = [
		{ number: 1, label: "Focus", slots: [{ kind: "component", ...FOCUS }] },
		{ number: 2, label: "Zoom", slots: [{ kind: "component", ...ZOOM }] },
	];
	const request = vi.fn(async () => leased(lane, 3, [FIXTURE_A, FIXTURE_B]));
	const release = vi.fn();
	const claimReadouts = vi.fn(() => release);
	const value: FamilyEncodersContextValue = {
		loadPages: async () => snapshot,
		readouts: new DisplayedSourceReadouts({ request }),
		session: {
			claimReadouts,
		} as unknown as FamilyEncodersContextValue["session"],
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const view = projection(lane);
	const { result } = renderHook(() => useFamilyEncoderBinding(view, "Focus"), {
		wrapper,
	});
	await waitFor(() =>
		expect(result.current.componentSlot(0)?.component.kind).toBe("focus"),
	);
	expect(result.current.pageCountFor("Focus")).toBe(2);
	expect(request).not.toHaveBeenCalled();
	expect(claimReadouts).not.toHaveBeenCalled();
	act(() => result.current.selectPage("Focus", 2));
	await waitFor(() =>
		expect(result.current.componentSlot(0)?.component.kind).toBe("zoom"),
	);
	await waitFor(() => expect(request).toHaveBeenCalledOnce());
	if (lane === "normal") expect(claimReadouts).toHaveBeenCalledOnce();
	else expect(claimReadouts).not.toHaveBeenCalled();
	act(() => result.current.selectPage("Focus", 1));
	await waitFor(() =>
		expect(result.current.componentSlot(0)?.component.kind).toBe("focus"),
	);
	if (lane === "normal") expect(release).toHaveBeenCalledOnce();
	expect(request).toHaveBeenCalledOnce();
});
