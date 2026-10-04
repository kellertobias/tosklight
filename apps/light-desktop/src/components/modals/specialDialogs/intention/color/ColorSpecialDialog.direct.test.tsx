import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ColorIntentReport } from "../../../../../api/client/attributeConfiguration";
import type { NativeColorPagesSnapshot } from "../../../../../api/nativeColorModels";
import { colorAdoptionNotice } from "../../../../../features/familyEncoders/colorAdoptionNotice";
import { nativeColorReference } from "../../../../../features/familyEncoders/nativeColorReference";
import {
	HEAD,
	nativePages,
	wheelControl,
} from "../../../../control/parameterControls/familyEncoders/nativeColorTestSupport";
import type { ParameterValuesMutationPort } from "../../../../control/parameterControls/parameterValueMutations";
import { encoderAreaStore } from "../../../../control/parameterControls/useEncoderArea";
import { CORE_COLOR_DESCRIPTORS, type ColorValueEntry } from "./colorDialogModel";
import { ColorSpecialDialog } from "./ColorSpecialDialog";
import type { ColorDialogLane } from "./useColorDialogLane";

const FIXTURE_A = "11111111-1111-4111-8111-111111111111";
const FIXTURE_B = "22222222-2222-4222-8222-222222222222";
const lane = vi.hoisted(() => ({ current: null as unknown as ColorDialogLane }));
const report = vi.hoisted(() => ({ current: null as ColorIntentReport | null }));
const pages = vi.hoisted(() => ({
	current: null as NativeColorPagesSnapshot | null,
	reads: [] as unknown[],
}));

vi.mock("../../../../../state/AppContext", () => ({
	useApp: () => ({ state: { shiftArmed: false }, dispatch: vi.fn() }),
}));
vi.mock("./useColorDialogLane", () => ({ useColorDialogLane: () => lane.current }));
vi.mock("../../../../../features/colorReport/useAcceptedColorReport", async (original) => ({
	...(await original<typeof import("../../../../../features/colorReport/useAcceptedColorReport")>()),
	useAcceptedColorReport: () => report.current,
}));
vi.mock("../../../../../features/familyEncoders/useNativeColorPages", () => ({
	useNativeColorPages: (_ids: unknown, active: boolean) => {
		if (active) pages.reads.push(nativeColorReference.get());
		return active ? pages.current : null;
	},
}));

let applied: Record<string, unknown>[];
let outcome: Record<string, unknown>;

function writer(): ParameterValuesMutationPort {
	return {
		batch: vi.fn(),
		applyIntent: vi.fn(async (input) => {
			applied.push(input as unknown as Record<string, unknown>);
			return outcome;
		}),
		finishGesture: vi.fn(async () => ({ status: "no_change" })),
		cancelGesture: vi.fn(() => 0),
	} as unknown as ParameterValuesMutationPort;
}

function mount() {
	act(() => encoderAreaStore.publish(document.createElement("div"), { width: 100, height: 100 }));
	render(
		<ColorSpecialDialog family="Color" selectedFixtureIds={[FIXTURE_A, FIXTURE_B]} close={vi.fn()} />,
	);
	return screen.getByRole("region", { name: "Direct color" });
}

beforeEach(() => {
	applied = [];
	outcome = { status: "changed" };
	pages.current = nativePages(10);
	pages.reads = [];
	report.current = null;
	lane.current = {
		lane: "normal",
		ready: true,
		fixtureIds: [FIXTURE_A, FIXTURE_B],
		groupId: null,
		timing: { fade: false, fadeMillis: null, delayMillis: null },
		descriptors: CORE_COLOR_DESCRIPTORS,
		colorFixtureIds: [FIXTURE_A, FIXTURE_B],
		variant: "lamp",
		values: [] as ColorValueEntry[],
		writers: { normal: writer(), preload: writer() },
	};
});
afterEach(() => {
	cleanup();
	encoderAreaStore.reset();
	colorAdoptionNotice.reset();
	nativeColorReference.set(null);
	vi.restoreAllMocks();
});

describe("Color modal: Direct section (TL-554)", () => {
	it("identifies the reference head and offers every overflow control beyond pages 3/4", () => {
		const section = mount();
		expect(within(section).getByTestId("color-direct-reference")).toHaveTextContent(
			"Reference: 101 · Wash · Main",
		);
		const overflow = within(section).getByTestId("color-direct-overflow");
		expect(
			within(overflow)
				.getAllByRole("group")
				.map((group) => group.getAttribute("aria-label")),
		).toEqual(["Native 1 · Emitter 9", "Native 2 · Emitter 10"]);
		// Before any Direct value: the passive replay preview per fixture.
		const status = within(section).getByTestId("color-direct-status");
		expect(status).toHaveTextContent("Replays exactly");
		expect(status).toHaveTextContent("Best-effort match");
		expect(section.querySelector("[role=alert], [role=status], [aria-live]")).toBeNull();
	});

	it("choosing a reference head for inspection reads pages and never writes", () => {
		const section = mount();
		fireEvent.click(within(section).getByRole("button", { name: "102 · Wash · Main" }));
		expect(nativeColorReference.get()).toEqual({ fixtureId: FIXTURE_B, headId: HEAD });
		expect(applied).toEqual([]);
	});

	it("turns an overflow control into a native edit that names the reference head", () => {
		const section = mount();
		const control = within(section).getByRole("group", { name: "Native 2 · Emitter 10" });
		fireEvent.keyDown(control, { key: "ArrowUp" });
		expect(applied).toHaveLength(1);
		expect(applied[0]).toMatchObject({
			attribute: "color",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: { edits: [{ kind: "native", operation: { kind: "relative", value: 257 } }] },
		});
	});

	it("selects an overflow wheel choice and steps its slots (TL-544 G4)", () => {
		const base = nativePages(9);
		const wheel = wheelControl(9);
		pages.current = { ...base, overflow: [wheel] };
		const section = mount();
		const choices = within(section).getByRole("list", { name: "Color wheel functions" });
		const open = within(choices).getByRole("button", { name: "Open" });
		expect(open).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(within(choices).getByRole("button", { name: "Blue" }));
		expect(applied).toHaveLength(1);
		expect(applied[0]).toMatchObject({
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: {
				edits: [
					{
						kind: "native",
						binding: {
							channel_id: wheel.channel_id,
							function_id: wheel.functions[2].function_id,
						},
						operation: { kind: "set", value: 20 },
					},
				],
			},
		});
		// The encoder of the same control steps one choice (Open → Red), never a relative raw.
		const control = within(section).getByRole("group", { name: "Native 1 · Color wheel" });
		fireEvent.keyDown(control, { key: "ArrowUp" });
		expect(applied[1]).toMatchObject({
			operation: {
				edits: [
					{
						binding: { function_id: wheel.functions[1].function_id },
						operation: { kind: "set", value: 10 },
					},
				],
			},
		});
	});

	it("shows per-head Direct status passively; native replay never claims an exact colour", () => {
		report.current = {
			color_model: "dynamic",
			accepted_frame: { state: "accepted", frame: null },
			heads: [
				{
					fixture_id: FIXTURE_A, fixture_number: 101, fixture_name: "Wash", owner_id: FIXTURE_A,
					head_name: "Main", has_target: true, quality: "approximate", engine: null,
					delta_uv: 0.01, calibration_revision: null,
					direct: { replay: "exact", origin: "forward", drive_limit: "within", limitations: [] },
				},
				{
					fixture_id: FIXTURE_B, fixture_number: 102, fixture_name: "Spot", owner_id: FIXTURE_B,
					head_name: "Main", has_target: true, quality: "unsupported", engine: null,
					delta_uv: null, calibration_revision: null,
					direct: {
						replay: "native_only", compatibility: "different_source", uv: "park_off",
						origin: "recorded", drive_limit: "unknown", limitations: [],
					},
				},
			],
		} as unknown as ColorIntentReport;
		const section = mount();
		const rows = within(within(section).getByTestId("color-direct-status")).getAllByRole("row");
		expect(rows.map((row) => row.textContent)).toEqual([
			"101 · Wash · MainNative replay",
			"102 · Spot · MainNative only · appearance unknowndifferent fixture type · UV parked off · recorded estimate",
		]);
		expect(section.textContent).not.toMatch(/exact colou?r/i);
	});

	it("holds for an explicit start quietly and sends the chosen start with the next edit", async () => {
		outcome = { status: "no_change", hold: "explicit_color_start_required" };
		mount();
		const fader = () => {
			const input = screen.getByRole("slider", { name: "White Blend" });
			vi.spyOn(input, "getBoundingClientRect").mockReturnValue({
				x: 0, y: 0, left: 0, top: 0, right: 100, bottom: 40, width: 100, height: 40, toJSON: () => ({}),
			} as DOMRect);
			return input;
		};
		const drag = async (to: number) => {
			const input = fader();
			const at = (x: number) => ({ pointerId: 1, clientX: x, clientY: 20, button: 0, pointerType: "touch" });
			await act(async () => {
				fireEvent.pointerDown(input, at(20));
				fireEvent.pointerMove(input, at(to));
				fireEvent.pointerUp(input, at(to));
			});
		};
		await drag(40);
		expect(applied.length).toBeGreaterThan(0);
		expect(applied[0].colorAdoption).toBeUndefined();
		const notice = screen.getByTestId("color-explicit-start");
		expect(notice).toHaveTextContent("appearance is unknown");
		expect(document.querySelector("[role=alert], [aria-live]")).toBeNull();
		fireEvent.click(within(notice).getByRole("button", { name: "Start from black" }));
		outcome = {
			status: "changed",
			colorAdoption: {
				fixtures: [{ fixtureId: FIXTURE_A, start: "explicit", uvUnknown: false }],
				limitations: [],
			},
		};
		const before = applied.length;
		await drag(60);
		expect(applied[before]).toMatchObject({ colorAdoption: { explicitStart: { rgb: [0, 0, 0] } } });
		expect(screen.getByTestId("color-adoption-report")).toHaveTextContent(
			"Started from your explicit colour.",
		);
		expect(colorAdoptionNotice.semanticInput()).toBeNull();
		expect(screen.queryByTestId("color-explicit-start")).toBeNull();
	});
});
