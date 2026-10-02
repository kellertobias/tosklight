import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ColorIntentReport } from "../../../../../api/client/attributeConfiguration";
import type { ParameterValuesMutationPort } from "../../../../control/parameterControls/parameterValueMutations";
import { encoderAreaStore } from "../../../../control/parameterControls/useEncoderArea";
import { CORE_COLOR_DESCRIPTORS, type ColorValueEntry } from "./colorDialogModel";
import { ColorSpecialDialog } from "./ColorSpecialDialog";
import type { ColorDialogLane } from "./useColorDialogLane";

const app = vi.hoisted(() => ({ shiftArmed: false, dispatch: vi.fn() }));
const lane = vi.hoisted(() => ({ current: null as unknown as ColorDialogLane }));
const report = vi.hoisted(() => ({ current: null as ColorIntentReport | null }));

vi.mock("../../../../../state/AppContext", () => ({
	useApp: () => ({ state: { shiftArmed: app.shiftArmed }, dispatch: app.dispatch }),
}));
vi.mock("./useColorDialogLane", () => ({ useColorDialogLane: () => lane.current }));
vi.mock("../../../../../features/colorReport/useAcceptedColorReport", async (original) => ({
	...(await original<typeof import("../../../../../features/colorReport/useAcceptedColorReport")>()),
	useAcceptedColorReport: () => report.current,
}));

interface Recorded {
	apply: { attribute: string; undoGroup: string; fixtureIds: readonly string[]; operation: { edits: unknown[] } }[];
	finish: { attribute: string; undoGroup: string }[];
}

function writer(recorded: Recorded): ParameterValuesMutationPort {
	return {
		batch: vi.fn(),
		applyIntent: vi.fn(async (input) => {
			recorded.apply.push(input as unknown as Recorded["apply"][number]);
		}),
		finishGesture: vi.fn(async (input) => {
			recorded.finish.push(input);
		}),
		cancelGesture: vi.fn(() => 0),
	} as unknown as ParameterValuesMutationPort;
}

let normal: Recorded;
let preload: Recorded;

function setLane(overrides: Partial<ColorDialogLane> = {}) {
	lane.current = {
		lane: "normal",
		ready: true,
		fixtureIds: ["a", "b"],
		groupId: null,
		timing: { fade: false, fadeMillis: null, delayMillis: null },
		descriptors: CORE_COLOR_DESCRIPTORS,
		colorFixtureIds: ["a", "b"],
		variant: "lamp",
		values: [] as ColorValueEntry[],
		writers: { normal: writer(normal), preload: writer(preload) },
		...overrides,
	};
}

const rect = (width: number, height: number) =>
	({ x: 0, y: 0, left: 0, top: 0, right: width, bottom: height, width, height, toJSON: () => ({}) }) as DOMRect;
const sized = <T extends Element>(element: T, width: number, height = 40) => {
	vi.spyOn(element as Element, "getBoundingClientRect").mockReturnValue(rect(width, height));
	return element;
};
const at = (pointerId: number, clientX: number, clientY = 20, extra: Record<string, unknown> = {}) => ({
	pointerId, clientX, clientY, button: 0, pointerType: "touch", ...extra,
});
const tap = (element: Element, x: number, y = 20, extra: Record<string, unknown> = {}, pointerId = 1) => {
	fireEvent.pointerDown(element, at(pointerId, x, y, extra));
	fireEvent.pointerUp(element, at(pointerId, x, y, extra));
};
const edits = (recorded: Recorded) => recorded.apply.map((entry) => entry.operation.edits);
const fader = (label: string) => screen.getByRole("slider", { name: label }) as HTMLInputElement;

let area: HTMLDivElement;
function measure(width: number, height: number) {
	act(() => encoderAreaStore.publish(area, { width, height }));
}

beforeEach(() => {
	normal = { apply: [], finish: [] };
	preload = { apply: [], finish: [] };
	app.shiftArmed = false;
	app.dispatch.mockReset();
	report.current = null;
	area = document.createElement("div");
	area.className = "parameter-surfaces";
	document.body.append(area);
	setLane();
});
afterEach(() => {
	cleanup();
	window.innerWidth = 1024;
	window.innerHeight = 768;
	area.remove();
	encoderAreaStore.reset();
	vi.restoreAllMocks();
});

describe("semantic Color dialog: placement by the measured encoder area", () => {
	it("renders compact inside the measured lower area at 680×210 and claims it", () => {
		measure(680, 210);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		const dialog = screen.getByRole("dialog", { name: "Color Special Dialog" });
		expect(area.contains(dialog)).toBe(true);
		expect(encoderAreaStore.get().inline).toBe("color");
		const page = within(dialog).getByTestId("editor-page");
		expect(page).toHaveAttribute("data-page", "mix");
		expect(within(dialog).getByTestId("color-picker")).toBeInTheDocument();
		expect(within(dialog).getByRole("slider", { name: "White Blend" })).toBeInTheDocument();
		expect(within(dialog).getAllByRole("button").map((button) => button.textContent)).toEqual([
			"White balance",
			"Expand",
		]);
		expect(dialog.querySelector("header, footer, [role=status], [role=alert], [aria-live]")).toBeNull();
		expect(dialog.textContent).not.toMatch(/Encoders|fixtures? selected|approximat/i);
	});

	it("opens the full modal when the measured area is below the budget, whatever the viewport", () => {
		window.innerWidth = 2560;
		window.innerHeight = 1440;
		measure(679, 400);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a"]} close={vi.fn()} />);
		const modal = screen.getByRole("dialog", { name: "Color Special Dialog" });
		expect(area.contains(modal)).toBe(false);
		expect(modal.closest(".ui-modal-stack-layer")).not.toBeNull();
		expect(screen.getByText("Color", { selector: "h2, h1, .ui-modal-title, [class*=title] *" })).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "Hue" })).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "Saturation" })).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "White Blend" })).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "Temperature" })).toBeInTheDocument();
		expect(screen.getByRole("slider", { name: "Duv" })).toBeInTheDocument();
		expect(screen.getByRole("region", { name: "Color approximation" })).toBeInTheDocument();
		expect(encoderAreaStore.get().inline).toBeNull();
	});

	it("switches from compact to the modal when the area shrinks, and back", () => {
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a"]} close={vi.fn()} />);
		expect(area.querySelector(".color-dialog-compact")).not.toBeNull();
		measure(600, 260);
		expect(area.querySelector(".color-dialog-compact")).toBeNull();
		expect(encoderAreaStore.get().inline).toBeNull();
		expect(screen.getByRole("slider", { name: "Hue" })).toBeInTheDocument();
		measure(900, 260);
		expect(area.querySelector(".color-dialog-compact")).not.toBeNull();
	});

	it("shows Temperature and Duv with white centres on page two and cycles on Special presses", () => {
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a"]} close={vi.fn()} />);
		act(() => encoderAreaStore.requestCycle());
		const page = screen.getByTestId("editor-page");
		expect(page).toHaveAttribute("data-page", "white");
		const sliders = within(page).getAllByRole("slider").map((slider) => slider.getAttribute("aria-label"));
		expect(sliders).toEqual(["Temperature", "Duv"]);
		for (const label of ["Temperature", "Duv"]) {
			const field = page.querySelector<HTMLElement>(`[data-control="${label === "Duv" ? "duv" : "temperature"}"]`);
			expect(field?.style.getPropertyValue("--range-fader-gradient")).toContain("#fff 50%");
		}
		act(() => encoderAreaStore.requestCycle());
		expect(screen.getByTestId("editor-page")).toHaveAttribute("data-page", "mix");
	});
});

describe("semantic Color dialog: edits through the Color family gesture session", () => {
	it("sends one gesture of Color component edits on the pinned lane with one Finish", () => {
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		const input = sized(fader("White Blend"), 100);
		fireEvent.pointerDown(input, at(1, 40));
		fireEvent.pointerMove(input, at(1, 60));
		fireEvent.pointerUp(input, at(1, 60));
		expect(edits(normal)).toEqual([
			[{ kind: "scalar", component: { kind: "color", component: "white_blend" }, operation: { kind: "set", value: { kind: "value", value: 0.4 } } }],
			[{ kind: "scalar", component: { kind: "color", component: "white_blend" }, operation: { kind: "set", value: { kind: "value", value: 0.6 } } }],
		]);
		expect(normal.apply.every((entry) => entry.attribute === "color")).toBe(true);
		expect(normal.apply[0].fixtureIds).toEqual(["a", "b"]);
		expect(new Set(normal.apply.map((entry) => entry.undoGroup)).size).toBe(1);
		expect(normal.finish).toEqual([
			expect.objectContaining({ attribute: "color", undoGroup: normal.apply[0].undoGroup }),
		]);
		expect(preload.apply).toEqual([]);
	});

	it("routes edits to Preload while capture mode captures Programmer writes", () => {
		setLane({ lane: "preload", timing: { fade: true, fadeMillis: 2000, delayMillis: null } });
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		tap(sized(fader("White Blend"), 100), 25);
		expect(normal.apply).toEqual([]);
		expect(preload.apply).toHaveLength(1);
		expect(preload.finish).toHaveLength(1);
	});

	it("is a quiet no-op with nothing selected: no request, no Finish, no notice", () => {
		setLane({ fixtureIds: [], colorFixtureIds: [] });
		const notices = vi.fn();
		window.addEventListener("light:desk-notice", notices);
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={[]} close={vi.fn()} />);
		tap(sized(fader("White Blend"), 100), 70);
		tap(sized(screen.getByTestId("color-picker"), 360, 100), 120, 40);
		window.removeEventListener("light:desk-notice", notices);
		expect(normal.apply).toEqual([]);
		expect(normal.finish).toEqual([]);
		expect(notices).not.toHaveBeenCalled();
		expect(document.querySelector("[role=alert], [aria-live]")).toBeNull();
	});
});

function redProgram(fixtureId: string): ColorValueEntry {
	return {
		fixtureId,
		attribute: "color",
		value: {
			kind: "color_program",
			value: {
				kind: "semantic",
				intent: {
					base_xyz: { x: 0.41, y: 0.21, z: 0.02 },
					recipe: { version: 1, rgb: [1, 0, 0], amber: 0, approximate: false },
					white_blend: 0,
					white_target: { kelvin: 6500, duv: 0 },
					uv: { amount: 0 },
					relative_output: 1,
					allocation: "preserve_recipe",
				},
			},
		},
	};
}

describe("semantic Color dialog: SHIFT ranges", () => {
	const spread = (component: string, value: number[]) => [
		{ kind: "scalar", component: { kind: "color", component }, operation: { kind: "set", value: { kind: "spread", value } } },
	];

	it("software SHIFT: the first tap marks endpoint 1 without writing, the last writes the descending range", () => {
		app.shiftArmed = true;
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		const input = sized(fader("White Blend"), 100);
		tap(input, 80);
		expect(normal.apply).toEqual([]);
		expect(screen.getByText("Shift-click the last value")).toBeInTheDocument();
		tap(sized(fader("White Blend"), 100), 20, 20, {}, 2);
		expect(edits(normal)).toEqual([spread("white_blend", [0.8, 0.2])]);
		expect(fader("White Blend")).toHaveAttribute("aria-valuetext", "80% through 20%");
	});

	it("physical Shift held for both endpoints sends the same ordered range", () => {
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		const input = sized(fader("White Blend"), 100);
		tap(input, 30, 20, { shiftKey: true });
		tap(sized(fader("White Blend"), 100), 90, 20, { shiftKey: true }, 2);
		expect(edits(normal)).toEqual([spread("white_blend", [0.3, 0.9])]);
	});

	it("an ordinary edit after a range collapses only that component", () => {
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		tap(sized(fader("White Blend"), 100), 30, 20, { shiftKey: true });
		tap(sized(fader("White Blend"), 100), 90, 20, { shiftKey: true }, 2);
		tap(sized(fader("White Blend"), 100), 50, 20, {}, 3);
		expect(edits(normal).at(-1)).toEqual([
			{ kind: "scalar", component: { kind: "color", component: "white_blend" }, operation: { kind: "set", value: { kind: "value", value: 0.5 } } },
		]);
		expect(edits(normal).flat().every((edit) => (edit as { component: { component: string } }).component.component === "white_blend")).toBe(true);
	});

	it("a hue range across 0° is sent as touched and shown along the shortest arc", () => {
		app.shiftArmed = true;
		setLane({ values: [redProgram("a"), redProgram("b")] });
		measure(600, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		const ring = sized(screen.getByRole("slider", { name: "Hue" }), 200, 200);
		// 0° is at the top, clockwise: 350° is just left of the top, 10° just right.
		const point = (degrees: number) => {
			const radians = (degrees * Math.PI) / 180;
			return [100 + Math.sin(radians) * 80, 100 - Math.cos(radians) * 80] as const;
		};
		tap(ring, ...point(350));
		tap(ring, ...point(10), {}, 2);
		expect(edits(normal)).toEqual([spread("hue", [350, 10])]);
		const strip = screen.getByTestId("color-requested-range");
		const stops = strip.style.background.match(/rgb\([^)]*\)/g) ?? [];
		expect(stops).toHaveLength(7);
		// Every sample stays red: the short arc never passes through cyan (180°).
		for (const stop of stops) {
			const [red, green, blue] = stop.match(/\d+/g)!.map(Number);
			expect(red).toBe(255);
			expect(Math.max(green, blue)).toBeLessThan(90);
		}
	});
});

describe("semantic Color dialog: readouts, approximation and Media", () => {
	it("shows requested values and per-fixture approximation with UV separate from the visible match", () => {
		setLane({
			values: [
				{
					fixtureId: "a",
					attribute: "color",
					value: {
						kind: "color_program",
						value: {
							kind: "semantic",
							intent: {
								base_xyz: { x: 0.4, y: 0.2, z: 0.02 },
								recipe: { version: 1, rgb: [1, 0, 1], amber: 0, approximate: false },
								white_blend: 0.25,
								white_target: { kelvin: 3200, duv: 0.004 },
								uv: { amount: 0.5 },
								relative_output: 1,
								allocation: "preserve_recipe",
							},
						},
					},
				},
			],
		});
		report.current = {
			color_model: "intent",
			accepted_frame: { state: "accepted", frame: null },
			heads: [
				{ fixture_id: "a", fixture_number: 101, fixture_name: "JBLED A7", owner_id: "a", head_name: "", has_target: true, quality: "approximate", engine: null, delta_uv: 0.012, calibration_revision: null, uv: { status: "applied", clipped: false } },
				{ fixture_id: "b", fixture_number: 102, fixture_name: "ROOT PAR 6", owner_id: "b", head_name: "", has_target: true, quality: "out_of_gamut", engine: null, delta_uv: 0.03, calibration_revision: null, uv: { status: "unsupported", clipped: false } },
			],
		};
		measure(600, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["a", "b"]} close={vi.fn()} />);
		expect(fader("White Blend")).toHaveAttribute("aria-valuenow", "25");
		expect(fader("Temperature")).toHaveAttribute("aria-valuenow", "3200");
		const approximation = screen.getByRole("region", { name: "Color approximation" });
		expect(within(approximation).getByText(/Requested · UV 50%/)).toBeInTheDocument();
		const rows = within(approximation).getAllByRole("row").slice(1);
		expect(rows).toHaveLength(2);
		expect(rows[0]).toHaveTextContent("Approximate");
		expect(rows[0]).toHaveTextContent("UV applied");
		expect(rows[1]).toHaveTextContent("Out of gamut");
		expect(rows[1]).toHaveTextContent("UV unavailable on this fixture");
		expect(approximation.querySelector("[role=alert], [role=status], [aria-live]")).toBeNull();
	});

	it("uses the Media variant for an all-Media selection: White Blend greys the shared tint", () => {
		setLane({ variant: "media" });
		measure(900, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["layer-1"]} close={vi.fn()} />);
		expect(screen.getByRole("slider", { name: "White Blend" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Switch to Preview" })).toBeInTheDocument();
		act(() => encoderAreaStore.requestCycle());
		expect(screen.getByTestId("media-color-preview")).toBeInTheDocument();
		expect(screen.queryByRole("slider", { name: "Temperature" })).toBeNull();
		expect(screen.queryByRole("slider", { name: /Intensity/ })).toBeNull();
	});

	it("keeps the lamp dialog for a mixed lamp and Media selection", () => {
		setLane({ variant: "lamp" });
		measure(600, 260);
		render(<ColorSpecialDialog family="Color" selectedFixtureIds={["lamp", "layer-1"]} close={vi.fn()} />);
		expect(screen.queryByTestId("media-color-preview")).toBeNull();
		expect(screen.getByRole("slider", { name: "Temperature" })).toBeInTheDocument();
	});
});
