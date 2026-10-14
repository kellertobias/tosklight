// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import { EncoderSurfaces } from "./EncoderSurfaces";
import {
	discreteParameterDisplay,
	normalizedParameterTarget,
} from "./parameterProgrammerState";
import type { ParameterController } from "./useParameterController";

vi.mock("../../../features/dynamics/DynamicEditorSessionContext", () => ({
	useDynamicEditorSession: () => ({ session: null }),
}));
vi.mock(
	"../../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingDeleteCommandActive: () => false,
		useProgrammingCommandLineActions: () => null,
	}),
);
afterEach(cleanup);
function fixture(id: string, raw = 180, resolution = "u8"): PatchedFixture {
	return {
		fixture_id: id,
		logical_heads: [],
		definition: {
			mode_id: "mode",
			profile_snapshot: {
				revision: 1,
				modes: [
					{
						id: "mode",
						heads: [{ id: "head", master_shared: true }],
						control_actions: [],
						channels: [
							{
								attribute: "iris",
								head_id: "head",
								resolution,
								functions: [
									{
										id: "closed-fn",
										attribute: "iris",
										dmx_from: raw,
										dmx_to: raw,
										name: "Closed",
										behavior: {
											type: "fixed",
											semantic_id: "closed",
											label: "Closed",
											raw_value: raw,
										},
									},
									{
										id: "open-fn",
										attribute: "iris",
										dmx_from: 0,
										dmx_to: 0,
										name: "Open",
										behavior: {
											type: "fixed",
											semantic_id: "open",
											label: "Open",
											raw_value: 0,
										},
									},
								],
							},
						],
					},
				],
			},
		},
	} as unknown as PatchedFixture;
}
function controller(
	values: Array<string | number | undefined>,
	hardware = true,
	fixtures = values.map((_, i) => fixture(`f${i}`)),
) {
	const c = {
		selectedFixtures: fixtures,
		selectedFixtureIds: fixtures.map((f) => f.fixture_id),
		selectedGroupId: null,
		programmerValues: values.flatMap((value, i) =>
			value === undefined
				? []
				: [
						{
							fixtureId: `f${i}`,
							attribute: "iris",
							value: {
								kind: typeof value === "string" ? "discrete" : "normalized",
								value,
							},
						},
					],
		),
		groupProgrammerValues: [],
		dynamicProgrammerValues: [],
		discreteByFixture: new Map(),
		normalizedByFixture: new Map(),
		normalized: new Map(),
		encoderSlots: ["iris"],
		encoderPushTurnSlots: [],
		attributeUnits: new Map(),
		attributeLabels: new Map(),
		hardwareConnected: hardware,
		canWriteValues: true,
		dynamicsMode: false,
		familyEncoders: { componentSlot: () => null },
		encoderSemanticDisplay: () => undefined,
		encoderNormalizedDisplay: () => undefined,
		hasProgrammerValue: () => true,
		applyParameter: vi.fn(),
		applyParameterRange: vi.fn(),
		applyIndexedPreset: vi.fn(),
		releaseParameter: vi.fn(),
		stepParameter: vi.fn(),
	} as unknown as ParameterController;
	c.programmerTarget = (a) => normalizedParameterTarget(c, a);
	c.encoderDiscreteDisplay = (a) => discreteParameterDisplay(c, a);
	return c;
}
function open(hardware = true) {
	fireEvent.click(
		screen.getByRole("button", {
			name: hardware ? /Encoder 1: Iris/ : /Set Enc 1/,
		}),
	);
	if (screen.queryByRole("tab", { name: "Indexed Presets" }))
		fireEvent.click(screen.getByRole("tab", { name: "Indexed Presets" }));
}
it.each([
	true,
	false,
])("reopens the real %s encoder with authoritative Closed selected and updates to Open", (hardware) => {
	const c = controller(["closed"], hardware);
	const view = render(<EncoderSurfaces controller={c} />, {
		wrapper: ModalProvider,
	});
	open(hardware);
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "true");
	fireEvent.click(screen.getByRole("button", { name: /^Close / }));
	open(hardware);
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "true");
	view.rerender(
		<EncoderSurfaces controller={controller(["open"], hardware)} />,
	);
	expect(
		screen.getByRole("button", { name: /Open.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "true");
	expect(c.applyParameter).not.toHaveBeenCalled();
	expect(c.applyIndexedPreset).not.toHaveBeenCalled();
});
it.each([
	[["closed", "open"]],
	[["closed", undefined]],
	[[undefined]],
	[[0.5]],
])("has no false selection for %j", (values) => {
	render(
		<EncoderSurfaces
			controller={controller(values as Array<string | number | undefined>)}
		/>,
		{ wrapper: ModalProvider },
	);
	open();
	expect(
		screen
			.getAllByRole("button", {
				name: /(?:Closed|Open).*All selected fixtures/,
			})
			.every((b) => b.getAttribute("aria-pressed") === "false"),
	).toBe(true);
});
it("seeds Direct input with Closed's actual authored raw position, not a fabricated zero", () => {
	const c = controller(["closed"]);
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	fireEvent.click(screen.getByRole("tab", { name: "Direct input" }));
	expect(screen.getByRole("textbox", { name: "Iris value" })).toHaveValue(
		"70.6",
	);
	expect(c.applyParameter).not.toHaveBeenCalled();
});

it("keeps common semantic selection when different profile raw positions cannot be represented by one number", () => {
	const c = controller(["closed", "closed"], true, [
		fixture("f0", 180),
		fixture("f1", 93),
	]);
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "true");
	expect(screen.queryByRole("tab", { name: "Direct input" })).toBeNull();
	fireEvent.click(
		screen.getByRole("button", { name: /Open.*All selected fixtures/ }),
	);
	expect(c.applyIndexedPreset).toHaveBeenCalledWith("iris", "open", [
		expect.objectContaining({
			fixtureId: "f0",
			functionId: "open-fn",
			profileRevision: 1,
		}),
		expect.objectContaining({
			fixtureId: "f1",
			functionId: "open-fn",
			profileRevision: 1,
		}),
	]);
});
it("uses the full u16 raw position for direct entry", () => {
	const c = controller(["closed"], true, [fixture("f0", 46_260, "u16")]);
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	fireEvent.click(screen.getByRole("tab", { name: "Direct input" }));
	expect(screen.getByRole("textbox", { name: "Iris value" })).toHaveValue(
		"70.6",
	);
});
it("does not mark same-label incompatible semantics selected", () => {
	const f = fixture("f1");
	const behavior =
		f.definition.profile_snapshot?.modes[0].channels[0].functions[0].behavior;
	if (behavior?.type === "fixed") behavior.semantic_id = "other-closed";
	const c = controller(["closed", "closed"], true, [fixture("f0"), f]);
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	expect(
		screen
			.getAllByRole("button", { name: /Closed/ })
			.every((b) => b.getAttribute("aria-pressed") === "false"),
	).toBe(true);
});
it("clears a prior discrete selection when authoritative normalized programming takes over", () => {
	const c = controller([0.5]);
	c.discreteByFixture.set("f0", new Map([["iris", "closed"]]));
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "false");
});

it("keeps normalized direct entry available after discrete programming is replaced", () => {
	const c = controller([0.5]);
	c.discreteByFixture.set("f0", new Map([["iris", "closed"]]));
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	fireEvent.click(screen.getByRole("tab", { name: "Direct input" }));
	expect(screen.getByRole("textbox", { name: "Iris value" })).toHaveValue("50");
});
it("does not borrow fixture resolved state for an unset portable group", () => {
	const c = controller([undefined]);
	c.selectedGroupId = "group";
	c.discreteByFixture.set("f0", new Map([["iris", "closed"]]));
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "false");
});

it.each([
	46_260, 46_270,
])("selects a normalized u16 fixed band at native boundary %s", (raw) => {
	const f = fixture("f0", 46_260, "u16");
	const fn = f.definition.profile_snapshot?.modes[0].channels[0].functions[0];
	if (!fn) throw new Error("Missing authored fixture function");
	fn.dmx_to = 46_270;
	const c = controller([raw / 65_535], true, [f]);
	render(<EncoderSurfaces controller={c} />, { wrapper: ModalProvider });
	open();
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "true");
	expect(c.applyParameter).not.toHaveBeenCalled();
	expect(c.applyIndexedPreset).not.toHaveBeenCalled();
});
it.each([
	46_259, 46_271,
])("does not select the normalized u16 band outside native boundary %s", (raw) => {
	const f = fixture("f0", 46_260, "u16");
	const fn = f.definition.profile_snapshot?.modes[0].channels[0].functions[0];
	if (!fn) throw new Error("Missing authored fixture function");
	fn.dmx_to = 46_270;
	render(
		<EncoderSurfaces controller={controller([raw / 65_535], true, [f])} />,
		{ wrapper: ModalProvider },
	);
	open();
	expect(
		screen.getByRole("button", { name: /Closed.*All selected fixtures/ }),
	).toHaveAttribute("aria-pressed", "false");
});
