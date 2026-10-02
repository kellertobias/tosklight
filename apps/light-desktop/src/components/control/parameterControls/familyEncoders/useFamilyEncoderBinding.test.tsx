import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FamilyEncoderPagesSnapshot } from "../../../../api/familyEncoderModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import { useHardwareParameterEncoders } from "../useHardwareParameterEncoders";
import type { ParameterProjection } from "../useParameterProjection";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
	pagesSnapshot,
} from "./familyEncoderTestSupport";
import { useFamilyEncoderBinding } from "./useFamilyEncoderBinding";

const writers = vi.hoisted(() => ({
	normal: null as unknown,
	preload: null as unknown,
}));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => writers.normal,
}));
vi.mock(
	"../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({ useProgrammerPreloadValuesActions: () => writers.preload }),
);

function projection(
	route: "normal" | "preload",
	overrides: Partial<ParameterProjection> = {},
): ParameterProjection {
	return {
		active: true,
		selectedFixtureIds: [FIXTURE_A, FIXTURE_B],
		selectedFixtures: [],
		selectedGroupId: null,
		supportedFixtureIdsByAttribute: new Map([
			["pan", [FIXTURE_A, FIXTURE_B]],
			["tilt", [FIXTURE_A, FIXTURE_B]],
		]),
		encoderGroups: [],
		encoderPage: 1,
		encoderPageCount: 1,
		encoderSlots: ["pan", "tilt", null, null],
		encoderPushTurnSlots: [null, null, null, null],
		visibleEncoderCount: 4,
		programmerValuesRoute: route,
		programmerValuesReady: true,
		programmerValues: [],
		programmerFadeMillis: 3_000,
		attributeUnits: new Map(),
		normalized: new Map([["pan", 0.5]]),
		discrete: new Map(),
		...overrides,
	} as unknown as ParameterProjection;
}

function context(snapshot: FamilyEncoderPagesSnapshot): FamilyEncodersContextValue {
	return {
		loadPages: async () => snapshot,
		readouts: new DisplayedSourceReadouts({
			request: async () => ({ lane: "normal", scope: {}, revision: 1, owners: [] }),
		}),
		session: null,
	};
}

/** The controller's own composition: binding, then the hardware handler with its delegation. */
function useComposedEncoders(
	view: ParameterProjection,
	legacy: {
		stepParameter: (attribute: string, delta: number) => Promise<unknown>;
		applyParameter: (attribute: string, level: number) => Promise<unknown>;
	},
) {
	const familyEncoders = useFamilyEncoderBinding(view, "Position");
	useHardwareParameterEncoders(
		{ ...view, ...familyEncoders.overrides },
		{
			canWriteValues: true,
			relativeSteps: true,
			programmerTarget: () => undefined,
			programmerDiscreteTarget: () => undefined,
			applyParameter: legacy.applyParameter,
			stepParameter: legacy.stepParameter,
			familyEncoderDetent: familyEncoders.detent,
		},
	);
	return familyEncoders;
}

function mount(snapshot: FamilyEncoderPagesSnapshot, route: "normal" | "preload") {
	const legacy = {
		stepParameter: vi.fn(async (_attribute: string, _delta: number) => null as unknown),
		applyParameter: vi.fn(async (_attribute: string, _level: number) => null as unknown),
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={context(snapshot)}>
			{children}
		</FamilyEncodersContextProvider>
	);
	const view = projection(route);
	const hook = renderHook(() => useComposedEncoders(view, legacy), { wrapper });
	return { hook, legacy, view };
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
	writers.normal = null;
	writers.preload = null;
});

describe("family encoder binding in the parameter controller", () => {
	it("leaves the legacy pages and normalized hardware steps untouched when not semantic", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = mount(pagesSnapshot(false), "normal");
		await act(async () => undefined);

		expect(hook.result.current.semantic).toBe(false);
		expect(hook.result.current.active).toBe(false);
		expect(hook.result.current.overrides).toEqual({});
		expect(hook.result.current.pageCountFor("Position")).toBeNull();
		encode(1, "up");

		expect(legacy.stepParameter).toHaveBeenCalledWith(
			"pan",
			0.01,
			expect.any(String),
			undefined,
		);
		expect(normal.applyIntent).not.toHaveBeenCalled();
	});

	it("routes hardware encode/N to the same component edit as a software step", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = mount(pagesSnapshot(true), "normal");
		await waitFor(() => expect(hook.result.current.active).toBe(true));
		expect(hook.result.current.overrides).toMatchObject({
			encoderPage: 1,
			encoderPageCount: 2,
			encoderSlots: [null, null, null, null],
		});

		encode(1, "up");
		act(() => hook.result.current.step(0, 1));

		expect(legacy.stepParameter).not.toHaveBeenCalled();
		expect(legacy.applyParameter).not.toHaveBeenCalled();
		const operations = normal.applyIntent.mock.calls.map(
			([input]) => (input as { operation: unknown }).operation,
		);
		expect(operations).toHaveLength(2);
		expect(operations[0]).toEqual(operations[1]);
		expect(operations[0]).toEqual({
			type: "component_edits",
			edits: [
				{
					kind: "scalar",
					component: { kind: "pan" },
					operation: { kind: "relative", value: 1 },
				},
			],
		});
	});

	it("pages to Point/X/Y/Z without writing and steps there on Preload", async () => {
		const preload = fakeWriter();
		writers.preload = preload;
		const { hook } = mount(pagesSnapshot(true), "preload");
		await waitFor(() => expect(hook.result.current.active).toBe(true));

		act(() => hook.result.current.selectPage("Position", 2));
		expect(hook.result.current.page).toBe(2);
		expect(preload.applyIntent).not.toHaveBeenCalled();
		expect(hook.result.current.componentSlot(1)?.id).toBe("position.target.x");

		encode(2, "down");
		expect(preload.applyIntent.mock.calls[0]?.[0]).toMatchObject({
			attribute: "position",
			operation: {
				type: "component_edits",
				edits: [
					{ kind: "target", reference: { kind: "origin" } },
					{
						kind: "scalar",
						component: { kind: "target_x" },
						operation: { kind: "relative", value: -0.1 },
					},
				],
			},
		});
	});
});
