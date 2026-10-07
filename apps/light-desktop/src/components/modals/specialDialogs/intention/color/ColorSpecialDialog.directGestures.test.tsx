import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";

// TL-544 G12 and G6 on the Direct section: blur, hidden and drag release finish the open
// Direct gesture at once; continuous native controls take an ordered [THRU] spread.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ColorIntentReport } from "../../../../../api/client/attributeConfiguration";
import type { NativeColorPagesSnapshot } from "../../../../../api/nativeColorModels";
import { colorAdoptionNotice } from "../../../../../features/familyEncoders/colorAdoptionNotice";
import { nativeColorReference } from "../../../../../features/familyEncoders/nativeColorReference";
import { HEAD, nativePages } from "../../../../control/parameterControls/familyEncoders/nativeColorTestSupport";
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
	fireEvent.click(screen.getByRole("tab", { name: "Details" }));
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

const finishes = () =>
	(lane.current.writers.normal as unknown as { finishGesture: ReturnType<typeof vi.fn> })
		.finishGesture;

function typeKeys(dialogName: string, keys: readonly string[]) {
	const dialog = screen.getByRole("dialog", { name: dialogName });
	for (const key of keys)
		fireEvent.click(
			Array.from(dialog.querySelectorAll("button")).find(
				(button) => (button.getAttribute("aria-label") ?? button.textContent) === key,
			) ?? screen.getByRole("button", { name: key }),
		);
}

describe("Color modal: Direct gesture ends and spreads (TL-544)", () => {
	it("finishes an open Direct turn at once on window blur, keeping its steps", () => {
		const section = mount();
		const control = within(section).getByRole("group", { name: "Native 1 · Emitter 9" });
		fireEvent.keyDown(control, { key: "ArrowUp" });
		fireEvent.keyDown(control, { key: "ArrowUp" });
		expect(finishes()).not.toHaveBeenCalled();
		act(() => {
			window.dispatchEvent(new Event("blur"));
			window.dispatchEvent(new Event("blur"));
		});
		expect(finishes()).toHaveBeenCalledOnce();
		expect(finishes().mock.calls[0]?.[0]).toMatchObject({
			attribute: "color",
			undoGroup: applied[0]?.undoGroup,
			keepAdmittedEdits: true,
		});
		expect(applied[1]?.undoGroup).toBe(applied[0]?.undoGroup);
	});

	it("finishes an open Direct turn when the document becomes hidden", () => {
		const section = mount();
		fireEvent.keyDown(within(section).getByRole("group", { name: "Native 1 · Emitter 9" }), {
			key: "ArrowUp",
		});
		vi.spyOn(document, "hidden", "get").mockReturnValue(true);
		act(() => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		expect(finishes()).toHaveBeenCalledOnce();
	});

	it("finishes the Direct gesture when a stepping drag is released", () => {
		const section = mount();
		const control = within(section).getByRole("group", { name: "Native 1 · Emitter 9" });
		fireEvent.pointerDown(control, { pointerId: 3, button: 0, clientY: 200 });
		fireEvent.pointerMove(control, { pointerId: 3, clientY: 150 });
		expect(applied.length).toBeGreaterThan(0);
		fireEvent.pointerUp(control, { pointerId: 3, clientY: 150 });
		expect(finishes()).toHaveBeenCalledOnce();
	});

	it("spreads a continuous control over the selection with THRU in rounded raw integers", () => {
		const section = mount();
		fireEvent.click(
			within(section).getByRole("button", { name: "Set Native 1 · Emitter 9 value" }),
		);
		typeKeys("Native 1 · Emitter 9 value", ["1", "0", "THRU", "3", "0", "0", "ENTER"]);
		expect(applied).toHaveLength(1);
		expect(applied[0]).toMatchObject({
			attribute: "color",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			colorAdoption: { nativeReference: { fixtureId: FIXTURE_A, headId: HEAD } },
			operation: { edits: [{ kind: "native", operation: { kind: "spread", value: [10, 255] } }] },
		});
		expect(finishes()).toHaveBeenCalledOnce();
	});
});
