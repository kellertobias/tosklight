import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../api/familyEncoderModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import type { ParameterProjection } from "../useParameterProjection";
import { FIXTURE_A, FIXTURE_B, fakeWriter, pagesSnapshot } from "./familyEncoderTestSupport";
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
			const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
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
