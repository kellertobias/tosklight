import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FamilyEncoderPagesSnapshot } from "../../../../api/familyEncoderModels";
import type { NativeColorPagesSnapshot } from "../../../../api/nativeColorModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { nativeColorReference } from "../../../../features/familyEncoders/nativeColorReference";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import { useHardwareParameterEncoders } from "../useHardwareParameterEncoders";
import type { ParameterProjection } from "../useParameterProjection";
import { FIXTURE_A, FIXTURE_B, fakeWriter, pagesSnapshot } from "./familyEncoderTestSupport";
import { HEAD, nativePages } from "./nativeColorTestSupport";
import { useFamilyEncoderBinding } from "./useFamilyEncoderBinding";

const writers = vi.hoisted(() => ({ normal: null as unknown, preload: null as unknown }));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => writers.normal,
}));
vi.mock(
	"../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({ useProgrammerPreloadValuesActions: () => writers.preload }),
);

function projection(hardware = false, visibleEncoderCount = 4): ParameterProjection {
	return {
		active: true,
		selectedFixtureIds: [FIXTURE_A, FIXTURE_B],
		selectedFixtures: [],
		selectedGroupId: null,
		supportedFixtureIdsByAttribute: new Map(),
		encoderGroups: [],
		encoderPage: 1,
		encoderPageCount: 1,
		encoderSlots: [null, null, null, null],
		encoderPushTurnSlots: [null, null, null, null],
		visibleEncoderCount,
		programmerValuesRoute: "normal",
		programmerValuesReady: true,
		programmerValues: [],
		programmerFadeMillis: 0,
		attributeUnits: new Map(),
		normalized: new Map(),
		discrete: new Map(),
		hardwareConnected: hardware,
	} as unknown as ParameterProjection;
}

function context(
	snapshot: FamilyEncoderPagesSnapshot,
	native: NativeColorPagesSnapshot,
	reads: string[],
): FamilyEncodersContextValue {
	return {
		loadPages: async () => snapshot,
		loadNativePages: async (_ids, reference) => {
			reads.push(reference?.fixtureId ?? "first");
			return native;
		},
		readouts: new DisplayedSourceReadouts({
			request: async () => ({ lane: "normal", scope: {}, revision: 1, owners: [] }),
		}),
		session: null,
	};
}

function mount(view: ParameterProjection, native = nativePages(9)) {
	const reads: string[] = [];
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={context(pagesSnapshot(true), native, reads)}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const hook = renderHook(
		() => {
			const encoders = useFamilyEncoderBinding(view, "Color");
			useHardwareParameterEncoders(
				{ ...view, ...encoders.overrides },
				{
					canWriteValues: true,
					relativeSteps: true,
					programmerTarget: () => undefined,
					programmerDiscreteTarget: () => undefined,
					applyParameter: async () => null,
					stepParameter: async () => null,
					familyEncoderDetent: encoders.detent,
				},
			);
			return encoders;
		},
		{ wrapper },
	);
	return { hook, reads };
}

function encode(slot: number, value: string) {
	act(() => {
		window.dispatchEvent(
			new CustomEvent("light:encoder-action", {
				detail: { control: `encode/${slot}`, value },
			}),
		);
	});
}

afterEach(() => {
	cleanup();
	writers.normal = null;
	writers.preload = null;
	nativeColorReference.set(null);
});

describe("Direct Color pages 3/4 in the encoder binding (TL-554)", () => {
	it("pages through 1–4 and back without any Programmer request", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, reads } = mount(projection());
		await waitFor(() => expect(hook.result.current.pageCountFor("Color")).toBe(4));
		for (const page of [3, 4, 2, 1, 3]) {
			act(() => hook.result.current.selectPage("Color", page));
			expect(hook.result.current.page).toBe(page);
		}
		expect(hook.result.current.overrides.encoderPageCount).toBe(4);
		expect(hook.result.current.componentSlot(0)?.label).toBe("Emitter 1 · 101");
		expect(hook.result.current.display(1)).toMatchObject({ value: 32_767, text: "32767" });
		// Choosing another reference head for inspection is a read, never a write.
		act(() => nativeColorReference.set({ fixtureId: FIXTURE_B, headId: HEAD }));
		await waitFor(() => expect(reads).toContain(FIXTURE_B));
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
	});

	it("routes hardware encode/N on page 3 to the same native edit as a software step", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook } = mount(projection(true));
		await waitFor(() => expect(hook.result.current.pageCountFor("Color")).toBe(4));
		act(() => hook.result.current.selectPage("Color", 3));
		encode(2, "up");
		act(() => hook.result.current.step(1, hook.result.current.componentSlot(1)!.descriptor.step));
		const sent = normal.applyIntent.mock.calls.map(([input]) => {
			const { operation, colorAdoption } = input as Record<string, unknown>;
			return { operation, colorAdoption };
		});
		expect(sent).toHaveLength(2);
		expect(sent[0]).toEqual(sent[1]);
		expect(sent[0]).toMatchObject({
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: { edits: [{ kind: "native", operation: { kind: "relative", value: 257 } }] },
		});
	});

	it("has no native pages without a verified reference head", async () => {
		const { hook, reads } = mount(projection(), { ...nativePages(4), reference: null });
		await waitFor(() => expect(reads.length).toBeGreaterThan(0));
		await act(async () => undefined);
		expect(hook.result.current.pageCountFor("Color")).toBe(1);
	});
});
