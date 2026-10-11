// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FamilyEncoderComponentSlot } from "../../../api/familyEncoderModels";
import { CompactEncoderAssignments } from "./CompactEncoderAssignments";
import { compactEncoderReadout, compactEncoderValueText } from "./CompactEncoderAssignments.readouts";
import { familySlotDisplay, type ProgrammerValueEntry } from "./familyEncoders/familyEncoderDisplay";
import { FIXTURE_A, FIXTURE_B, PAN, POINT, ZOOM_UNKNOWN_CONVENTION } from "./familyEncoders/familyEncoderTestSupport";
import { nativeColorSlot, nativeValueText } from "./familyEncoders/nativeColorSlots";
import { nativeControl, wheelControl } from "./familyEncoders/nativeColorTestSupport";
import { discreteParameterDisplay, discreteParameterTarget, normalizedParameterDisplay, normalizedParameterTarget, parameterSemanticDisplay } from "./parameterProgrammerState";
import type { ParameterFamily } from "./model";
import type { ParameterController } from "./useParameterController";

afterEach(cleanup);

function controller(overrides: Partial<ParameterController> = {}): ParameterController {
	const result = {
		family: "Intensity",
		encoderPage: 1,
		encoderSlots: ["intensity", null, null, null],
		encoderPushTurnSlots: [null, null, null, null],
		visibleEncoderCount: 4,
		encoderGroups: [],
		attributeLabels: new Map(),
		attributeUnits: new Map(),
		selectedFixtures: [],
		selectedFixtureIds: [FIXTURE_A, FIXTURE_B],
		selectedGroupId: null,
		programmerValues: [],
		groupProgrammerValues: [],
		dynamicProgrammerValues: [],
		normalized: new Map(),
		normalizedByFixture: new Map(),
		discrete: new Map(),
		discreteByFixture: new Map(),
		selectEncoderGroup: vi.fn(),
		familyEncoders: { componentSlot: () => null, pageCountFor: () => null, pointChoices: [] },
		...overrides,
	} as unknown as ParameterController;
	result.programmerTarget = (attribute) => normalizedParameterTarget(result, attribute);
	result.programmerDiscreteTarget = (attribute) => discreteParameterTarget(result, attribute);
	result.encoderNormalizedDisplay = (attribute, domain) => normalizedParameterDisplay(result, attribute, domain);
	result.encoderDiscreteDisplay = (attribute) => discreteParameterDisplay(result, attribute);
	result.encoderSemanticDisplay = (attribute) => parameterSemanticDisplay(result, attribute);
	return result;
}

function semantic(slot: FamilyEncoderComponentSlot, values: readonly ProgrammerValueEntry[]) {
	return controller({
		family: "Position",
		encoderSlots: [null, null, null, null],
		familyEncoders: {
			componentSlot: (index: number) => index === 0 ? slot : null,
			display: () => familySlotDisplay(slot, { programmerValues: values, readouts: null }),
			pageCountFor: (family: ParameterFamily) => family === "Position" ? 2 : null,
			pointChoices: [],
		} as unknown as ParameterController["familyEncoders"],
	});
}

describe("compact hardware encoder assignments", () => {
	it("keeps real ordered semantic spreads and mixed requests without averaging", () => {
		const entry = (fixtureId: string, points: number[]): ProgrammerValueEntry => ({ fixtureId, attribute: "position", value: { kind: "position", value: { kind: "angles", pan_degrees: { kind: "spread", value: points }, tilt_degrees: { kind: "value", value: 0 } } } });
		const value = semantic(PAN, [entry(FIXTURE_A, [270, -270, 270]), entry(FIXTURE_B, [270, -270, 270])]);
		expect(compactEncoderReadout(value, 0)).toMatchObject({ text: "270° thru -270° thru 270°", detail: "Spread" });
		const mixed = semantic(PAN, [entry(FIXTURE_A, [270, -270]), entry(FIXTURE_B, [90, -90])]);
		expect(compactEncoderReadout(mixed, 0).text).toBe("Mixed");
	});

	it("preserves semantic provenance and unavailable/Point state", () => {
		const value = semantic(PAN, []);
		value.familyEncoders.display = () => ({ value: null, text: "-30°...60°", source: "resolved", provenance: "From Point" });
		expect(compactEncoderReadout(value, 0)).toMatchObject({ text: "-30°...60°", detail: "From Point" });
		expect(compactEncoderReadout(semantic(ZOOM_UNKNOWN_CONVENTION, []), 0)).toMatchObject({ text: "—", detail: "Unsupported" });
		expect(compactEncoderReadout(semantic(POINT, []), 0)).toMatchObject({ text: "No Points" });
	});

	it("retains native full-width integers and native wheel choices", () => {
		const emitter = nativeControl(3);
		const slot = nativeColorSlot(emitter, emitter.functions[0], [FIXTURE_A]);
		const value = semantic(slot, []);
		value.familyEncoders.display = () => ({ value: 4_294_967_295, text: nativeValueText(slot, 4_294_967_295), source: "requested" });
		expect(compactEncoderReadout(value, 0).text).toBe("4294967295");
		const wheel = wheelControl(0);
		const wheelSlot = nativeColorSlot(wheel, wheel.functions[1], [FIXTURE_A]);
		value.familyEncoders.componentSlot = () => wheelSlot;
		value.familyEncoders.display = () => ({ value: 15, text: nativeValueText(wheelSlot, 15), source: "requested" });
		expect(compactEncoderReadout(value, 0).text).toBe("Red");
	});

	it("uses live legacy selection ranges and reports its Push-turn binding", () => {
		const value = controller({
			encoderPushTurnSlots: ["strobe", null, null, null],
			attributeLabels: new Map([["strobe", "Strobe"]]),
			normalized: new Map([["intensity", 0.2], ["strobe", 0.75]]),
			normalizedByFixture: new Map([[FIXTURE_A, new Map([["intensity", 0.2], ["strobe", 0.75]])], [FIXTURE_B, new Map([["intensity", 0.8], ["strobe", 0.75]])]]),
		});
		expect(compactEncoderReadout(value, 0)).toMatchObject({ text: "20%...80%", pushTurn: { label: "Strobe", text: "75%" } });
		expect(compactEncoderReadout(controller(), 0).text).toBe("—");
		expect(compactEncoderReadout(controller(), 1)).toMatchObject({ label: "Unassigned", text: "—", assigned: false });
	});

	it("preserves group spread control points in order", () => {
		const value = controller({ selectedGroupId: "g1", groupProgrammerValues: [{ attribute: "intensity", value: { kind: "spread", value: [0.8, 0.2, 0.8] } }] as unknown as ParameterController["groupProgrammerValues"] });
		expect(compactEncoderReadout(value, 0)).toMatchObject({ text: "80% thru 20% thru 80%", detail: "Spread" });
	});

	it.each([4, 5, 6])("renders %s passive slots in a single grid row", (count) => {
		const value = controller({ visibleEncoderCount: count as ParameterController["visibleEncoderCount"] });
		const { container } = render(<CompactEncoderAssignments controller={value} />);
		const row = container.querySelector(".compact-encoder-readouts");
		expect(row).toHaveStyle({ gridTemplateColumns: `repeat(${count}, minmax(0, 1fr))` });
		expect(row?.children).toHaveLength(count);
		expect(row?.querySelector("button,input,[tabindex]")).toBeNull();
		fireEvent.click(row?.firstElementChild as Element);
		expect(value.selectEncoderGroup).not.toHaveBeenCalled();
	});

	it("uses semantic group pages and the shared navigation callback", () => {
		const value = semantic(PAN, []);
		const { rerender } = render(<CompactEncoderAssignments controller={value} />);
		fireEvent.click(screen.getByRole("button", { name: "Position (1/2)" }));
		expect(value.selectEncoderGroup).toHaveBeenLastCalledWith("Position", 2);
		value.encoderPage = 2;
		rerender(<CompactEncoderAssignments controller={value} />);
		fireEvent.click(screen.getByRole("button", { name: "Position (2/2)" }));
		expect(value.selectEncoderGroup).toHaveBeenLastCalledWith("Position", 1);
		fireEvent.click(screen.getByRole("button", { name: "Color" }));
		expect(value.selectEncoderGroup).toHaveBeenLastCalledWith("Color", 1);
	});

	it("keeps family controls available to leave shared Dynamics mode", () => {
		const value = controller({ dynamicsMode: true });
		const { container } = render(<CompactEncoderAssignments controller={value} />);
		expect(screen.getByRole("status")).toHaveTextContent("Dynamics — select a parameter group");
		expect(container.querySelector(".compact-encoder-readouts")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: "Color" }));
		expect(value.selectEncoderGroup).toHaveBeenCalledWith("Color", 1);
	});

	it.each(["stage", "timecode", "patch", "dynamic-editor"] as const)("shows the owner %s context instead of ordinary assignments", (encoderContext) => {
		const { container } = render(<CompactEncoderAssignments controller={controller({ encoderContext })} />);
		expect(screen.getByRole("status")).toBeInTheDocument();
		expect(container.querySelector(".compact-encoder-readouts")).toBeNull();
	});
});

it("keeps large live fixture spreads readable without losing their range or direction", () => {
	expect(compactEncoderValueText(Array(28).fill("1%").join(" thru "))).toBe("1%");
	expect(compactEncoderValueText("75% thru 50% thru 25% thru 0%" )).toBe("75…0%");
	expect(compactEncoderValueText("0° thru 30° thru -20° thru 0°")).toBe("-20…30° · Mixed");
	expect(compactEncoderValueText("25% thru 50% thru 75%")).toBe("25…75%");
});
