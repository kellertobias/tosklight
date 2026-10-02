import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RuntimeCapabilityEvent, SessionResponse } from "../../../../api/types";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import { routeOperatorEvent } from "../../../../features/server/operatorEventRouting";
import type { ServerState } from "../../../../features/server/useServerState";
import { useHardwareParameterEncoders } from "../useHardwareParameterEncoders";
import type { ParameterProjection } from "../useParameterProjection";
import type { ParameterController } from "../useParameterController";
import { FamilyEncoderSlotSurface } from "./FamilyEncoderSlotSurface";
import { FIXTURE_A, FIXTURE_B, fakeWriter, PAN, pagesSnapshot } from "./familyEncoderTestSupport";
import { useFamilyEncoderBinding } from "./useFamilyEncoderBinding";

/**
 * TL-549 Position encoders: the real hardware/OSC path (server `desk_action` operator event →
 * `routeOperatorEvent` → `light:encoder-action` → the controller's hardware handler → the family
 * binding), page navigation, the atomic first Target offset and the Target-to-Angle takeover.
 */

const writers = vi.hoisted(() => ({ normal: null as unknown, preload: null as unknown }));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => writers.normal,
}));
vi.mock("../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView", () => ({
	useProgrammerPreloadValuesActions: () => writers.preload,
}));

const LEASE = 17;
const session = { session_id: "session-1", desk: { id: "desk-1" } } as SessionResponse;

const target = (fixtureId: string) => ({
	fixtureId,
	attribute: "position",
	value: { kind: "position", value: { kind: "target", reference: { kind: "origin" }, offset_metres: [{ kind: "value", value: 1 }, { kind: "value", value: 0 }, { kind: "value", value: 0 }] } },
});
const angles = (fixtureId: string) => ({
	fixtureId,
	attribute: "position",
	value: { kind: "position", value: { kind: "angles", pan_degrees: { kind: "value", value: 10 }, tilt_degrees: { kind: "value", value: 20 } } },
});

function projection(route: "normal" | "preload", programmerValues: unknown[]): ParameterProjection {
	return {
		active: true,
		selectedFixtureIds: [FIXTURE_A, FIXTURE_B],
		selectedFixtures: [],
		selectedGroupId: null,
		supportedFixtureIdsByAttribute: new Map([["pan", [FIXTURE_A, FIXTURE_B]], ["tilt", [FIXTURE_A, FIXTURE_B]]]),
		encoderGroups: [],
		encoderPage: 1,
		encoderPageCount: 1,
		encoderSlots: ["pan", "tilt", null, null],
		encoderPushTurnSlots: [null, null, null, null],
		visibleEncoderCount: 4,
		programmerValuesRoute: route,
		programmerValuesReady: true,
		programmerValues,
		programmerFadeMillis: 3_000,
		attributeUnits: new Map(),
		normalized: new Map(),
		discrete: new Map(),
	} as unknown as ParameterProjection;
}

function context(lane: "normal" | "preload", available = true): FamilyEncodersContextValue {
	return {
		loadPages: async () => pagesSnapshot(true),
		readouts: new DisplayedSourceReadouts({
			request: async () => ({
				lane,
				scope: {},
				revision: 1,
				lease: LEASE,
				owners: [FIXTURE_A, FIXTURE_B].map((fixture_id) => ({
					fixture_id,
					position: available
						? { available: true, commands: [], common: { pan_degrees: 45, tilt_degrees: -30 } }
						: { available: false, commands: [], common: null },
				})),
			}),
		}),
		session: null,
	};
}

async function mount(route: "normal" | "preload", values: unknown[], available = true) {
	const legacy = vi.fn(async () => null as unknown);
	const value = context(route, available);
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>{children}</FamilyEncodersContextProvider>
	);
	const view = projection(route, values);
	const hook = renderHook(
		() => {
			const familyEncoders = useFamilyEncoderBinding(view, "Position");
			useHardwareParameterEncoders(
				{ ...view, ...familyEncoders.overrides },
				{
					canWriteValues: true,
					relativeSteps: true,
					programmerTarget: () => undefined,
					programmerDiscreteTarget: () => undefined,
					applyParameter: legacy,
					stepParameter: legacy,
					familyEncoderDetent: familyEncoders.detent,
				},
			);
			return familyEncoders;
		},
		{ wrapper },
	);
	await waitFor(() => expect(hook.result.current.active).toBe(true));
	await waitFor(() => expect(value.readouts.displayedSource(route)).toEqual({ lane: route, lease: LEASE }));
	return { hook, legacy };
}

/** One physical/OSC encoder detent as the server publishes it to this desk session. */
function hardware(control: string, value: string) {
	const event: RuntimeCapabilityEvent = {
		type: "operator_notification",
		notification: {
			type: "desk_action",
			revision: 1,
			notification: { action: null, control, value, request_id: null, session_id: null, desk_id: null, path: "desk" },
		},
	} as unknown as RuntimeCapabilityEvent;
	act(() => routeOperatorEvent(event, session, {} as ServerState));
}

type Writer = ReturnType<typeof fakeWriter>;
const sent = (writer: Writer) => writer.applyIntent.mock.calls.map(([input]) => input as { operation: { edits: unknown[] }; displayedSource?: unknown });

afterEach(() => {
	writers.normal = null;
	writers.preload = null;
});

describe("Position encoders under the semantic contract", () => {
	it("hardware encode/1 while Target is active adopts the displayed pose with one activate_angles", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = await mount("normal", [target(FIXTURE_A), target(FIXTURE_B)]);
		expect(hook.result.current.display(0)).toMatchObject({ value: 45, source: "resolved" });
		hardware("encode/1", "up");
		expect(legacy).not.toHaveBeenCalled();
		expect(sent(normal)[0]).toMatchObject({
			displayedSource: { lane: "normal", lease: LEASE },
			operation: { edits: [{ kind: "activate_angles" }, { kind: "scalar", component: { kind: "pan" }, operation: { kind: "relative", value: 1 } }] },
		});
	});

	it("visiting page 2 sends nothing; the first X detent while Angles sends Target Origin and the offset in one edit", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook } = await mount("normal", [angles(FIXTURE_A), angles(FIXTURE_B)]);
		act(() => hook.result.current.selectPage("Position", 2));
		act(() => hook.result.current.selectPage("Position", 1));
		act(() => hook.result.current.selectPage("Position", 2));
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
		hardware("encode/2", "up");
		expect(sent(normal)).toHaveLength(1);
		expect(sent(normal)[0]?.operation.edits).toEqual([
			{ kind: "target", reference: { kind: "origin" } },
			{ kind: "scalar", component: { kind: "target_x" }, operation: { kind: "relative", value: 0.1 } },
		]);
	});

	it("the first Y detent on a native-only selection is atomic too; with Target active only the offset is sent", async () => {
		const native = fakeWriter();
		writers.normal = native;
		const first = await mount("normal", []);
		act(() => first.hook.result.current.selectPage("Position", 2));
		hardware("encode/3", "down");
		expect(sent(native)[0]?.operation.edits).toEqual([
			{ kind: "target", reference: { kind: "origin" } },
			{ kind: "scalar", component: { kind: "target_y" }, operation: { kind: "relative", value: -0.1 } },
		]);
		first.hook.unmount();

		const active = fakeWriter();
		writers.normal = active;
		const second = await mount("normal", [target(FIXTURE_A), target(FIXTURE_B)]);
		act(() => second.hook.result.current.selectPage("Position", 2));
		hardware("encode/4", "right");
		expect(sent(active)[0]?.operation.edits).toEqual([
			{ kind: "scalar", component: { kind: "target_z" }, operation: { kind: "relative", value: 1 } },
		]);
	});

	it("routes Preload encoder detents to the Preload writer with the Preload lease", async () => {
		const normal = fakeWriter();
		const preload = fakeWriter();
		writers.normal = normal;
		writers.preload = preload;
		await mount("preload", [target(FIXTURE_A), target(FIXTURE_B)]);
		hardware("encode/2", "down");
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(sent(preload)[0]).toMatchObject({
			displayedSource: { lane: "preload", lease: LEASE },
			operation: { edits: [{ kind: "activate_angles" }, { kind: "scalar", component: { kind: "tilt" }, operation: { kind: "relative", value: -1 } }] },
		});
	});

	it.each([false, true])("labels resolved Pan as Resolved and requested values plainly (hardware %s)", (hardwareConnected) => {
		const controller = (source: "resolved" | "requested") =>
			({
				hardwareConnected,
				canWriteValues: true,
				hasProgrammerValue: () => true,
				releaseParameter: async () => undefined,
				familyEncoders: {
					componentSlot: () => ({ ...PAN, label: "Pan" }),
					display: () => ({ value: 45, text: "45.0°", source }),
					step: () => undefined,
					set: () => undefined,
				},
			}) as unknown as ParameterController;
		const view = render(<FamilyEncoderSlotSurface controller={controller("resolved")} index={0} />);
		expect(screen.getAllByText(/Pan · Resolved/).length).toBeGreaterThan(0);
		view.rerender(<FamilyEncoderSlotSurface controller={controller("requested")} index={0} />);
		expect(screen.queryByText(/Resolved/)).toBeNull();
	});

	it("sends nothing from hardware encode/N or software for movers without Position physical data (TL-637)", async () => {
		const normal = fakeWriter();
		writers.normal = normal;
		const { hook, legacy } = await mount("normal", [], false);
		await waitFor(() => expect(hook.result.current.display(0)).toMatchObject({ source: "none", unsupported: true }));
		hardware("encode/1", "up");
		hardware("encode/2", "right");
		act(() => hook.result.current.step(0, 1));
		act(() => hook.result.current.set(1, 10));
		act(() => hook.result.current.selectPage("Position", 2));
		hardware("encode/1", "up");
		hardware("encode/2", "up");
		await new Promise((resolve) => setTimeout(resolve, 300));
		// Nothing semantic and no legacy normalized fallback either: a quiet, consumed detent.
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
		expect(legacy).not.toHaveBeenCalled();
	});

	it.each([false, true])("shows Pan without Position physical data quietly as unsupported and not editable (hardware %s)", (hardwareConnected) => {
		cleanup();
		const step = vi.fn();
		const set = vi.fn();
		const controller = {
			hardwareConnected,
			canWriteValues: true,
			hasProgrammerValue: () => false,
			releaseParameter: async () => undefined,
			familyEncoders: {
				componentSlot: () => ({ ...PAN, label: "Pan" }),
				display: () => ({ value: null, text: "—", source: "none", unsupported: true }),
				step,
				set,
			},
		} as unknown as ParameterController;
		render(<FamilyEncoderSlotSurface controller={controller} index={0} />);
		expect(screen.getAllByText(/Pan · Unsupported/).length).toBeGreaterThan(0);
		expect(screen.queryByRole("alert")).toBeNull();
		if (!hardwareConnected) {
			expect(screen.getByRole("group", { name: "Enc 1 · Pan · Unsupported" })).toHaveAttribute("aria-disabled", "true");
			for (const button of screen.getAllByRole("button"))
				if (/Pan/.test(button.getAttribute("aria-label") ?? "")) expect(button).toBeDisabled();
		}
		expect(step).not.toHaveBeenCalled();
		expect(set).not.toHaveBeenCalled();
	});
});
