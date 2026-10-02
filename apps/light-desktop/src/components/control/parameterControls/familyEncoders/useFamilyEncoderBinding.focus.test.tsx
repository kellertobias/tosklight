import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

/** TL-551: the Focus family encoder pages (Focus, Zoom, Softness) through the foundation binding. */
import type { FamilyEncoderPagesSnapshot } from "../../../../api/familyEncoderModels";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import { useHardwareParameterEncoders } from "../useHardwareParameterEncoders";
import type { ParameterController } from "../useParameterController";
import { FamilyEncoderSlotSurface } from "./FamilyEncoderSlotSurface";
import type { ParameterProjection } from "../useParameterProjection";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
	pagesSnapshot,
	ZOOM_UNKNOWN_CONVENTION,
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
		normalized: new Map([["zoom", 0.5]]),
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
	const familyEncoders = useFamilyEncoderBinding(view, "Focus");
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
	vi.useRealTimers();
});

const scalarStep = (kind: "focus" | "zoom", value: number) => ({
	type: "component_edits",
	edits: [{ kind: "scalar", component: { kind }, operation: { kind: "relative", value } }],
});

/** The semantic snapshot with the Zoom slot's convention withdrawn (unknown or mixed). */
function unknownConventionSnapshot(): FamilyEncoderPagesSnapshot {
	const snapshot = structuredClone(pagesSnapshot(true));
	for (const page of snapshot.families.find((group) => group.family === "focus")?.pages ?? [])
		for (const slot of page.slots)
			if (slot?.kind === "component" && slot.component.kind === "zoom") slot.convention = null;
	return snapshot;
}

describe("Focus family encoders", () => {
	it("keeps the legacy normalized Zoom step at contract 0", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = mount(pagesSnapshot(false), "normal");
		await act(async () => undefined);
		expect(hook.result.current.overrides).toEqual({});
		encode(2, "up");
		expect(legacy.stepParameter).toHaveBeenCalledWith("zoom", 0.01, expect.any(String), undefined);
		expect(normal.applyIntent).not.toHaveBeenCalled();
	});

	it("orders Focus, Zoom, Softness and gives hardware encode/N and software steps the same unit edits", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = mount(pagesSnapshot(true), "normal");
		await waitFor(() => expect(hook.result.current.active).toBe(true));
		expect(hook.result.current.componentSlot(0)?.id).toBe("focus");
		expect(hook.result.current.componentSlot(1)?.id).toBe("zoom");
		expect(hook.result.current.overrides).toMatchObject({ encoderSlots: [null, null, "softness", null] });

		encode(1, "up");
		act(() => hook.result.current.step(0, 0.01));
		encode(2, "down");
		act(() => hook.result.current.step(1, -1));
		encode(2, "right");

		expect(legacy.stepParameter).not.toHaveBeenCalled();
		const calls = normal.applyIntent.mock.calls.map(([input]) => input as { attribute: string; operation: unknown; undoGroup: string });
		expect(calls.map((call) => [call.attribute, call.operation])).toEqual([
			["focus", scalarStep("focus", 0.01)],
			["focus", scalarStep("focus", 0.01)],
			["zoom", scalarStep("zoom", -1)],
			["zoom", scalarStep("zoom", -1)],
			["zoom", scalarStep("zoom", 10)],
		]);
		// Separate owners: Focus and Zoom detents never share an Undo group.
		expect(calls[0]?.undoGroup).toBe(calls[1]?.undoGroup);
		expect(calls[2]?.undoGroup).toBe(calls[4]?.undoGroup);
		expect(calls[0]?.undoGroup).not.toBe(calls[2]?.undoGroup);
	});

	it("ends each owner's encoder gesture with its own Finish on Preload", async () => {
		const preload = fakeWriter();
		writers.preload = preload;
		const { hook } = mount(pagesSnapshot(true), "preload");
		await waitFor(() => expect(hook.result.current.active).toBe(true));
		vi.useFakeTimers();
		encode(1, "up");
		encode(2, "up");
		act(() => vi.advanceTimersByTime(300));
		expect(preload.applyIntent.mock.calls.map(([input]) => (input as { attribute: string }).attribute)).toEqual(["focus", "zoom"]);
		expect(preload.finishGesture.mock.calls.map(([input]) => (input as { attribute: string }).attribute).sort()).toEqual(["focus", "zoom"]);
	});

	it("sends no Zoom edit from hardware encode/N or software while the convention is unknown", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = mount(unknownConventionSnapshot(), "normal");
		await waitFor(() => expect(hook.result.current.active).toBe(true));
		vi.useFakeTimers();
		expect(hook.result.current.componentSlot(1)?.convention).toBeNull();

		encode(2, "up");
		encode(2, "right");
		act(() => hook.result.current.step(1, 1));
		act(() => hook.result.current.set(1, 30));
		act(() => vi.advanceTimersByTime(300));

		// Nothing semantic and no legacy normalized fallback either.
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
		expect(legacy.stepParameter).not.toHaveBeenCalled();
		expect(legacy.applyParameter).not.toHaveBeenCalled();

		// Focus on the same page still edits.
		encode(1, "up");
		expect(normal.applyIntent.mock.calls.map(([input]) => (input as { attribute: string }).attribute)).toEqual(["focus"]);
	});

	it.each([false, true])("shows an unknown-convention Zoom quietly as unsupported and not editable (hardware %s)", (hardwareConnected) => {
		const step = vi.fn();
		const set = vi.fn();
		const controller = {
			hardwareConnected,
			canWriteValues: true,
			hasProgrammerValue: () => false,
			releaseParameter: async () => undefined,
			familyEncoders: {
				componentSlot: () => ({ ...ZOOM_UNKNOWN_CONVENTION, label: "Zoom" }),
				display: () => ({ value: 25, text: "25.0°", source: "requested" }),
				step,
				set,
			},
		} as unknown as ParameterController;
		render(<FamilyEncoderSlotSurface controller={controller} index={1} />);
		expect(screen.getAllByText(/Zoom · Unsupported/).length).toBeGreaterThan(0);
		expect(screen.getAllByText("25.0°").length).toBeGreaterThan(0);
		expect(screen.queryByRole("alert")).toBeNull();
		if (!hardwareConnected) {
			for (const button of screen.getAllByRole("button"))
				if (/Zoom/.test(button.getAttribute("aria-label") ?? ""))
					expect(button).toBeDisabled();
		}
		expect(step).not.toHaveBeenCalled();
		expect(set).not.toHaveBeenCalled();
	});
});
