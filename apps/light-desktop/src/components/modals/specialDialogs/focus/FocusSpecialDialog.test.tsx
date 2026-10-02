import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FamilyEncoderPagesSnapshot } from "../../../../api/familyEncoderModels";
import { FamilyEncodersContextProvider } from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { DisplayedSourceReadouts } from "../../../../features/programmerValues/displayedSource";
import {
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
	pagesSnapshot,
} from "../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import { resolveSpecialDialog } from "../registry/specialDialogRegistry";
import { FocusSpecialDialog } from "./FocusSpecialDialog";

const desk = vi.hoisted(() => ({
	capture: null as unknown,
	normal: null as unknown,
	preload: null as unknown,
	enabled: { normal: false, preload: false },
}));
vi.mock("../../../../features/programmerCaptureMode/ProgrammerCaptureModeView", () => ({
	useProgrammerCaptureModeView: () => desk.capture,
}));
vi.mock("../../../../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: () => ({ selected: ["a", "b"], expression: null }),
}));
vi.mock("../../../../features/configuration/ConfigurationState", () => ({
	useProgrammerFadeMillis: () => 2_000,
	useColorPresentation: () => "easy_rgbw",
}));
vi.mock("../../../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => desk.normal,
}));
vi.mock("../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView", () => ({
	useProgrammerPreloadValuesActions: () => desk.preload,
}));
const view = (value: number) => ({
	ready: true,
	fixtureValues: [FIXTURE_A, FIXTURE_B].map((fixtureId) => ({
		fixtureId,
		attribute: "zoom",
		value: { kind: "zoom", value: { opening_degrees: { kind: "value", value }, convention: "beam" } },
	})),
	groupValues: [],
	dynamicValues: [],
});
vi.mock("../../../control/parameterControls/useParameterProgrammerValues", () => ({
	useParameterProgrammerValues: (_ids: unknown, _group: unknown, enabled: boolean) => {
		desk.enabled.normal = enabled;
		return enabled ? view(20) : null;
	},
}));
vi.mock("../../../control/parameterControls/useParameterPreloadValues", () => ({
	useParameterPreloadValues: (_ids: unknown, _group: unknown, enabled: boolean) => {
		desk.enabled.preload = enabled;
		return enabled ? view(30) : null;
	},
}));

function snapshot(semantic: boolean): FamilyEncoderPagesSnapshot {
	const pages = pagesSnapshot(semantic);
	for (const group of pages.families)
		for (const page of group.pages)
			for (const slot of page.slots)
				if (slot?.kind === "component" && slot.component.kind === "zoom") {
					slot.convention = "field";
					slot.limits = { min: 5, max: 60 };
				}
	return pages;
}

function mount(
	request = vi.fn(async (_path: string): Promise<unknown> => ({ lane: "normal", scope: {}, revision: 1, owners: [] })),
) {
	const readouts = new DisplayedSourceReadouts({ request: request as never });
	render(
		<FamilyEncodersContextProvider
			value={{
				loadPages: async () => snapshot(true),
				readouts,
				session: null,
			}}
		>
			<FocusSpecialDialog family="Focus" selectedFixtureIds={[FIXTURE_A, FIXTURE_B]} close={() => undefined} />
		</FamilyEncodersContextProvider>,
	);
	return { request, readouts };
}

const leased = (lease: number, fixtureIds: readonly string[]) => ({
	lane: "normal",
	scope: {},
	revision: 1,
	lease,
	owners: fixtureIds.map((fixture_id) => ({ fixture_id, position: { available: false, commands: [] } })),
});

beforeEach(() => {
	vi.stubGlobal("ResizeObserver", class {
		observe() {}
		disconnect() {}
		unobserve() {}
	});
	desk.normal = fakeWriter();
	desk.preload = fakeWriter();
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

describe("Focus Special Dialog registration and lanes", () => {
	it("is the semantic Focus entry, and contract 0 keeps no Focus dialog", () => {
		expect(resolveSpecialDialog("Focus", true)).toMatchObject({ mode: "semantic", Component: FocusSpecialDialog });
		expect(resolveSpecialDialog("Focus", false)).toBeNull();
	});

	it.each([
		["Normal", { blind: false, preloadCaptureProgrammer: false }, "normal", 20, { fade: false, fadeMillis: null, delayMillis: null }],
		["Preload", { blind: true, preloadCaptureProgrammer: true }, "preload", 30, { fade: true, fadeMillis: 2_000, delayMillis: null }],
	] as const)("reads and writes the %s lane with the published convention and limits", async (_name, capture, lane, requested, timing) => {
		desk.capture = capture;
		mount();
		const slider = await screen.findByRole("slider", { name: "Field opening angle" });
		await waitFor(() => expect(slider).toHaveAttribute("aria-valuenow", String(requested)));
		expect(slider).toHaveAttribute("aria-valuemin", "5");
		expect(slider).toHaveAttribute("aria-valuemax", "60");
		expect(desk.enabled).toEqual({ normal: lane === "normal", preload: lane === "preload" });
		fireEvent.keyDown(slider, { key: "ArrowDown" });
		const used = (lane === "preload" ? desk.preload : desk.normal) as ReturnType<typeof fakeWriter>;
		const other = (lane === "preload" ? desk.normal : desk.preload) as ReturnType<typeof fakeWriter>;
		expect(used.applyIntent).toHaveBeenCalledWith(
			expect.objectContaining({
				attribute: "zoom",
				timing,
				operation: {
					type: "component_edits",
					edits: [{ kind: "scalar", component: { kind: "zoom" }, operation: { kind: "set", value: { kind: "value", value: requested - 1 } } }],
				},
			}),
		);
		expect(used.finishGesture).toHaveBeenCalledWith(expect.objectContaining({ attribute: "zoom" }));
		expect(other.applyIntent).not.toHaveBeenCalled();
	});

	it("names the lease that delivered the dialog's Focus/Zoom fixtures, not another consumer's newer one", async () => {
		desk.capture = { blind: false, preloadCaptureProgrammer: false };
		const { readouts } = mount(vi.fn(async () => leased(3, [FIXTURE_A, FIXTURE_B])));
		const slider = await screen.findByRole("slider", { name: "Field opening angle" });
		await waitFor(() => expect(readouts.displayedSource("normal")).toEqual({ lane: "normal", lease: 3 }));
		// Another surface then reads a different fixture and receives a newer lease.
		readouts.observe(leased(4, ["33333333-3333-4333-8333-333333333333"]) as never);
		fireEvent.keyDown(slider, { key: "ArrowDown" });
		expect((desk.normal as ReturnType<typeof fakeWriter>).applyIntent).toHaveBeenCalledWith(
			expect.objectContaining({ attribute: "zoom", displayedSource: { lane: "normal", lease: 3 } }),
		);
	});
});
