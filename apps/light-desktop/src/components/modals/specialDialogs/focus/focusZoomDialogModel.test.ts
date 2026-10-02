import { describe, expect, it } from "vitest";
import {
	componentSlot,
	FIXTURE_A,
	FIXTURE_B,
	pagesSnapshot,
} from "../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import {
	focusDialogValue,
	focusZoomSlots,
	NOT_AVAILABLE_STATUS,
	NOT_PROGRAMMED_STATUS,
	requestedControlValue,
	ZOOM_UNSUPPORTED_STATUS,
	zoomDialogValue,
	zoomLimits,
} from "./focusZoomDialogModel";

const ZOOM = componentSlot("zoom", { kind: "zoom" }, { convention: "beam" });
const zoomEntry = (fixtureId: string, value: number) => ({
	fixtureId,
	attribute: "zoom",
	value: { kind: "zoom", value: { opening_degrees: { kind: "value", value }, convention: "beam" } } as const,
});

describe("Focus Special Dialog model", () => {
	it("takes the Focus and Zoom slots only from a semantic snapshot", () => {
		expect(focusZoomSlots(pagesSnapshot(false))).toEqual({ focus: null, zoom: null });
		const slots = focusZoomSlots(pagesSnapshot(true));
		expect(slots.focus?.id).toBe("focus");
		expect(slots.zoom?.id).toBe("zoom");
	});

	it("uses published selection limits, else the descriptor domain, never a demo range", () => {
		expect(zoomLimits({ ...ZOOM, limits: { min: 7, max: 42 } })).toEqual({
			minimum: 7, maximum: 42, step: 0.1, keyStep: 1, largeKeyStep: 10,
		});
		expect(zoomLimits(ZOOM)).toMatchObject({ minimum: 0, maximum: 180 });
	});

	it("shows Mixed or Not programmed instead of inventing a shared requested value", () => {
		const mixed = requestedControlValue(ZOOM, [zoomEntry(FIXTURE_A, 10), zoomEntry(FIXTURE_B, 30)]);
		expect(mixed.value).toBeNull();
		expect(zoomDialogValue({ slot: ZOOM, requested: mixed, local: undefined, refused: false }).status).toBe("Mixed");
		const none = requestedControlValue(ZOOM, []);
		expect(zoomDialogValue({ slot: ZOOM, requested: none, local: undefined, refused: false }).status).toBe(NOT_PROGRAMMED_STATUS);
		expect(focusDialogValue({ slot: null, requested: none, local: undefined }).status).toBe(NOT_AVAILABLE_STATUS);
	});

	it("reports an unknown convention or a refusal as unsupported and keeps the requested value", () => {
		const requested = requestedControlValue(ZOOM, [zoomEntry(FIXTURE_A, 25), zoomEntry(FIXTURE_B, 25)]);
		const unknown = zoomDialogValue({ slot: { ...ZOOM, convention: null }, requested, local: 31, refused: false });
		expect(unknown).toMatchObject({ status: ZOOM_UNSUPPORTED_STATUS, zoom: { value: 31, convention: null } });
		expect(zoomDialogValue({ slot: ZOOM, requested, local: undefined, refused: true })).toMatchObject({
			status: ZOOM_UNSUPPORTED_STATUS,
			zoom: { value: 25, convention: "beam" },
		});
	});
});
