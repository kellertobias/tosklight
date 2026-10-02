import { act, cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../../api/familyEncoderModels";
import { FamilyEncodersContextProvider } from "../../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../../features/programmerValues/displayedSource";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
} from "../../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import type { ParameterValuesMutationPort } from "../../../../control/parameterControls/parameterValueMutations";
import type { RangeGesture } from "../HorizontalRangeFader";
import { CORE_COLOR_DESCRIPTORS } from "./colorDialogModel";
import type { ColorDialogLane } from "./useColorDialogLane";
import { useColorGestures } from "./useColorGestures";

/**
 * TL-594 lease follow-up: a Color dialog gesture names the newest lease whose delivery covered
 * the dialog's Color fixtures, not merely the lane's newest lease (another consumer's read).
 */

const FIXTURE_C = "33333333-3333-4333-8333-333333333333";

const leased = (lane: "normal" | "preload", lease: number, fixtureIds: readonly string[]) =>
	({
		lane,
		scope: { show_id: null },
		revision: 1,
		lease,
		owners: fixtureIds.map((fixture_id) => ({ fixture_id, position: { available: false, commands: [] } })),
	}) as unknown as OutputReadoutSnapshot;

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe("Color Special Dialog gestures name the covering displayed-source lease", () => {
	for (const name of ["normal", "preload"] as const) {
		it(`${name}: a fader step names the lease that delivered the Color fixtures`, () => {
			const writer = fakeWriter();
			const readouts = new DisplayedSourceReadouts({ request: vi.fn() });
			readouts.observe(leased(name, 3, [FIXTURE_A, FIXTURE_B]));
			readouts.observe(leased(name, 4, [FIXTURE_C]));
			const lane: ColorDialogLane = {
				lane: name,
				ready: true,
				fixtureIds: [FIXTURE_A, FIXTURE_B],
				groupId: null,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
				descriptors: CORE_COLOR_DESCRIPTORS,
				colorFixtureIds: [FIXTURE_A, FIXTURE_B],
				variant: "lamp",
				values: [],
				writers: {
					normal: name === "normal" ? (writer as unknown as ParameterValuesMutationPort) : null,
					preload: name === "preload" ? (writer as unknown as ParameterValuesMutationPort) : null,
				},
			};
			const wrapper = ({ children }: { children: ReactNode }) => (
				<FamilyEncodersContextProvider value={{ loadPages: vi.fn(), readouts, session: null }}>
					{children}
				</FamilyEncodersContextProvider>
			);
			const { result } = renderHook(() => useColorGestures(lane, () => undefined), { wrapper });
			act(() => {
				const started: RangeGesture = { control: "White Blend", source: "keyboard", shifted: false };
				const id = result.current.onGestureStart?.(started);
				const gesture = { ...started, ...(id === undefined ? {} : { id }) };
				result.current.change(gesture, [
					{ component: "white_blend", operation: { kind: "set", value: { kind: "value", value: 0.5 } } },
				]);
				result.current.onGestureEnd?.(gesture);
			});
			expect(writer.applyIntent).toHaveBeenCalledWith(
				expect.objectContaining({ attribute: "color", displayedSource: { lane: name, lease: 3 } }),
			);
		});
	}
});
