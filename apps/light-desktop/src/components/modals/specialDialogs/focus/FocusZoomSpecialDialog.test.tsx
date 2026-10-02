import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	componentSlot,
	FIXTURE_A,
	FIXTURE_B,
	fakeWriter,
} from "../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import type { ProgrammerValueEntry } from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import {
	FocusZoomSpecialDialog,
	type FocusZoomSpecialDialogProps,
} from "./FocusZoomSpecialDialog";
import { ZOOM_REQUESTED_STATUS, ZOOM_UNSUPPORTED_STATUS } from "./focusZoomDialogModel";

// Default diagram geometry (640 × 180) mapped 1:1 to client pixels, as in FocusZoomDialog.test.
const WIDTH = 640, HEIGHT = 180, LEFT = 32, RIGHT = WIDTH - 32, LENGTH = RIGHT - LEFT, CENTER = HEIGHT / 2;
const MAXIMUM_OPENING = CENTER - 28;
const ZOOM_MAX = 50;
const rad = (degrees: number) => (degrees * Math.PI) / 180;
const halfOpening = (zoom: number) => (MAXIMUM_OPENING * Math.tan(rad(zoom / 2))) / Math.tan(rad(ZOOM_MAX / 2));
const zoomFromOpening = (opening: number) =>
	Math.round(((2 * Math.atan((opening / MAXIMUM_OPENING) * Math.tan(rad(ZOOM_MAX / 2))) * 180) / Math.PI) * 10) / 10;

const ZOOM = componentSlot("zoom", { kind: "zoom" }, {
	limits: { min: 10, max: ZOOM_MAX },
	limits_source: "selection",
	convention: "beam",
});
const FOCUS = componentSlot("focus", { kind: "focus" }, { limits_source: "descriptor" });

function zoomValue(fixtureId: string, degrees: number): ProgrammerValueEntry {
	return {
		fixtureId,
		attribute: "zoom",
		value: { kind: "zoom", value: { opening_degrees: { kind: "value", value: degrees }, convention: "beam" } },
	};
}
function focusValue(fixtureId: string, value: number): ProgrammerValueEntry {
	return { fixtureId, attribute: "focus", value: { kind: "normalized", value } };
}

type Environment = FocusZoomSpecialDialogProps["environment"];

let ids = 0;
beforeEach(() => {
	ids = 0;
	vi.stubGlobal("crypto", { ...crypto, randomUUID: () => `uuid-${++ids}` });
	vi.stubGlobal("ResizeObserver", class {
		observe() {}
		disconnect() {}
		unobserve() {}
	});
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

function setup(overrides: Partial<Environment> = {}) {
	const normal = fakeWriter();
	const preload = fakeWriter();
	const environment: Environment = {
		focusSlot: FOCUS,
		zoomSlot: ZOOM,
		lane: "normal",
		ready: true,
		groupId: null,
		timing: { fade: false, fadeMillis: null, delayMillis: null },
		programmerValues: [zoomValue(FIXTURE_A, 20), zoomValue(FIXTURE_B, 20), focusValue(FIXTURE_A, 0.5), focusValue(FIXTURE_B, 0.5)],
		writerFor: (lane) => (lane === "preload" ? preload : normal),
		...overrides,
	};
	const close = vi.fn();
	const view = render(<FocusZoomSpecialDialog environment={environment} close={close} />);
	const diagram = screen.getByRole("group", { name: "Beam angle and focus diagram" });
	vi.spyOn(diagram, "getBoundingClientRect").mockReturnValue({
		x: 0, y: 0, left: 0, top: 0, right: WIDTH, bottom: HEIGHT, width: WIDTH, height: HEIGHT, toJSON: () => ({}),
	});
	return { normal, preload, close, view, environment };
}

const pointer = (pointerId: number, clientX: number, clientY: number) => ({ pointerId, clientX, clientY, button: 0, pointerType: "touch" });
const upperHandle = () => screen.getByTestId("beam-angle-handle-upper").querySelector(".focus-zoom-angle-hit") as Element;
const focusSlider = () => screen.getByRole("slider", { name: "Focus position" });
const focusHandle = () => focusSlider().querySelector(".focus-zoom-focus-handle") as Element;
const cy = (element: Element) => Number(element.getAttribute("cy"));
type Writer = ReturnType<typeof fakeWriter>;
const intents = (writer: Writer) => writer.applyIntent.mock.calls.map(([input]) => input as Record<string, unknown>);
const finishes = (writer: Writer) => writer.finishGesture.mock.calls.map(([input]) => input as Record<string, unknown>);
const scalar = (kind: "zoom" | "focus", value: number) => ({
	type: "component_edits",
	edits: [{ kind: "scalar", component: { kind }, operation: { kind: "set", value: { kind: "value", value } } }],
});

function dragZoom(pointerId: number, offset: number, deltas: number[]) {
	const handleY = cy(upperHandle());
	fireEvent.pointerDown(upperHandle(), pointer(pointerId, RIGHT + 4, handleY + offset));
	for (const delta of deltas) fireEvent.pointerMove(upperHandle(), pointer(pointerId, RIGHT + 4, handleY + offset - delta));
	return () => fireEvent.pointerUp(upperHandle(), pointer(pointerId, RIGHT + 4, handleY + offset - (deltas.at(-1) ?? 0)));
}

describe("Focus Special Dialog (production)", () => {
	it("opens directly in the standard modal from the published descriptors and sends nothing on open or close", () => {
		const { normal, close } = setup();
		const dialog = screen.getByRole("dialog", { name: "Focus Special Dialog" });
		expect(screen.getByRole("slider", { name: "Beam opening angle" })).toHaveAttribute("aria-valuemin", "10");
		expect(screen.getByRole("slider", { name: "Beam opening angle" })).toHaveAttribute("aria-valuemax", "50");
		expect(screen.getByRole("slider", { name: "Beam opening angle" })).toHaveAttribute("aria-valuenow", "20");
		expect(focusSlider()).toHaveAttribute("aria-valuetext", "50%");
		expect(dialog.querySelector("img, canvas, [data-testid='semantic-special-dialog-placeholder']")).toBeNull();
		expect(screen.getByTestId("focus-zoom-zoom-status")).toHaveTextContent(ZOOM_REQUESTED_STATUS);
		fireEvent.click(screen.getByRole("button", { name: "Close Focus Special Dialog" }));
		expect(close).toHaveBeenCalledTimes(1);
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
	});

	it("turns a beam-edge drag into Zoom degree edits from the grab offset, then one Finish", async () => {
		const { normal } = setup();
		const release = dragZoom(3, 9, [0, 10, 20]);
		// The zero-movement move after an off-centre grab sends nothing (no jump to the pointer).
		expect(intents(normal)).toHaveLength(2);
		expect(intents(normal)[0]).toMatchObject({
			attribute: "zoom",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
			groupId: null,
			operation: scalar("zoom", zoomFromOpening(halfOpening(20) + 10)),
		});
		expect(intents(normal)[1]?.operation).toEqual(scalar("zoom", zoomFromOpening(halfOpening(20) + 20)));
		release();
		expect(finishes(normal)).toEqual([expect.objectContaining({ attribute: "zoom", undoGroup: intents(normal)[0]?.undoGroup })]);
		await act(async () => undefined);
		expect(screen.getByTestId("focus-zoom-zoom-status")).toHaveTextContent(ZOOM_REQUESTED_STATUS);
	});

	it("keeps Focus and Zoom as separate owners with separate Undo groups and Finishes", () => {
		const { normal } = setup();
		dragZoom(1, 0, [12])();
		const planeX = LEFT + 0.5 * LENGTH;
		fireEvent.pointerDown(focusHandle(), pointer(2, planeX + 12, CENTER + 8));
		fireEvent.pointerMove(focusHandle(), pointer(2, planeX + 12 + LENGTH * 0.1, CENTER + 30));
		fireEvent.pointerUp(focusHandle(), pointer(2, 0, 0));
		const [zoom, focus] = intents(normal);
		expect(zoom?.attribute).toBe("zoom");
		expect(focus).toMatchObject({ attribute: "focus", operation: scalar("focus", 0.6) });
		expect(zoom?.undoGroup).not.toBe(focus?.undoGroup);
		expect(finishes(normal).map((finish) => [finish.attribute, finish.undoGroup])).toEqual([
			["zoom", zoom?.undoGroup],
			["focus", focus?.undoGroup],
		]);
	});

	it("keeps both angle handles draggable while Focus sits at its far end", () => {
		const { normal } = setup({
			programmerValues: [zoomValue(FIXTURE_A, 10), zoomValue(FIXTURE_B, 10), focusValue(FIXTURE_A, 1), focusValue(FIXTURE_B, 1)],
		});
		const hit = focusSlider().querySelector(".focus-zoom-focus-hit") as Element;
		const hitTop = Number(hit.getAttribute("y"));
		expect(Number(hit.getAttribute("x")) + Number(hit.getAttribute("width"))).toBeGreaterThanOrEqual(RIGHT);
		// The upper handle's 44 px circle stays clear of the Focus target at the far end.
		expect(cy(upperHandle()) + 22).toBeLessThanOrEqual(Math.max(hitTop, CENTER - 22) + 0.001);
		dragZoom(4, 0, [15])();
		expect(intents(normal)).toEqual([expect.objectContaining({ attribute: "zoom" })]);
		expect((intents(normal)[0]?.operation as ReturnType<typeof scalar>).edits[0]?.operation.value.value).toBeGreaterThan(10);
	});

	it.each([
		["window blur", "blur", () => window.dispatchEvent(new Event("blur"))],
		["hidden document", "hidden", () => {
			Object.defineProperty(document, "hidden", { configurable: true, get: () => true });
			document.dispatchEvent(new Event("visibilitychange"));
		}],
	] as const)("ends an open drag once on %s", (_name, _reason, stop) => {
		const { normal } = setup();
		const planeX = LEFT + 0.5 * LENGTH;
		fireEvent.pointerDown(focusHandle(), pointer(5, planeX, CENTER));
		fireEvent.pointerMove(focusHandle(), pointer(5, planeX + LENGTH * 0.1, CENTER));
		act(() => stop());
		expect(normal.cancelGesture).toHaveBeenCalledTimes(1);
		expect(finishes(normal)).toEqual([expect.objectContaining({ attribute: "focus" })]);
		fireEvent.pointerMove(focusHandle(), pointer(5, planeX + LENGTH * 0.3, CENTER));
		fireEvent.pointerUp(focusHandle(), pointer(5, planeX + LENGTH * 0.3, CENTER));
		expect(intents(normal)).toHaveLength(1);
		expect(normal.finishGesture).toHaveBeenCalledTimes(1);
		Reflect.deleteProperty(document, "hidden");
	});

	it("keeps an unknown-convention Zoom request locally with a quiet unsupported state and sends nothing", () => {
		const { normal } = setup({ zoomSlot: { ...ZOOM, convention: null } });
		expect(screen.getByRole("slider", { name: "Zoom opening angle" })).toBeInTheDocument();
		expect(screen.getByTestId("focus-zoom-zoom-status")).toHaveTextContent(ZOOM_UNSUPPORTED_STATUS);
		dragZoom(6, 0, [10])();
		expect(normal.applyIntent).not.toHaveBeenCalled();
		expect(normal.finishGesture).not.toHaveBeenCalled();
		expect(screen.getByRole("slider", { name: "Zoom opening angle" })).toHaveAttribute(
			"aria-valuenow",
			String(zoomFromOpening(halfOpening(20) + 10)),
		);
		expect(screen.queryByRole("alert")).toBeNull();
	});

	it("keeps the requested Zoom and shows unsupported quietly when the server refuses the first edit", async () => {
		const { normal } = setup();
		normal.applyIntent.mockImplementation(async () => null as never);
		dragZoom(7, 0, [10])();
		await act(async () => undefined);
		expect(screen.getByTestId("focus-zoom-zoom-status")).toHaveTextContent(ZOOM_UNSUPPORTED_STATUS);
		expect(screen.getByRole("slider", { name: "Beam opening angle" })).toHaveAttribute(
			"aria-valuenow",
			String(zoomFromOpening(halfOpening(20) + 10)),
		);
		expect(screen.queryByRole("alert")).toBeNull();
		// Focus is an independent owner and keeps working.
		fireEvent.keyDown(focusSlider(), { key: "ArrowRight" });
		expect(intents(normal).at(-1)).toMatchObject({ attribute: "focus", operation: scalar("focus", 0.51) });
	});

	it.each(["normal", "preload"] as const)("writes on the %s lane only, pinned for the gesture, naming its displayed source", (lane) => {
		const timing = lane === "preload" ? { fade: true, fadeMillis: 2_000, delayMillis: null } : { fade: false, fadeMillis: null, delayMillis: null };
		const { normal, preload } = setup({
			lane,
			timing,
			displayedSource: (target) => ({ lane: target, lease: target === "preload" ? 41 : 7 }),
		});
		const [used, other] = lane === "preload" ? [preload, normal] : [normal, preload];
		fireEvent.keyDown(screen.getByRole("slider", { name: "Beam opening angle" }), { key: "ArrowUp" });
		expect(intents(used)).toEqual([
			expect.objectContaining({
				attribute: "zoom",
				timing,
				operation: scalar("zoom", 21),
				displayedSource: { lane, lease: lane === "preload" ? 41 : 7 },
			}),
		]);
		expect(finishes(used)).toEqual([expect.objectContaining({ attribute: "zoom" })]);
		expect(other.applyIntent).not.toHaveBeenCalled();
		expect(other.finishGesture).not.toHaveBeenCalled();
	});

	it("marks both controls disabled until the lane can take edits, so no key is silently lost", () => {
		const notReady = setup({ ready: false });
		for (const name of ["Beam opening angle", "Focus position"])
			expect(screen.getByRole("slider", { name })).toHaveAttribute("aria-disabled", "true");
		fireEvent.keyDown(focusSlider(), { key: "End" });
		expect(notReady.normal.applyIntent).not.toHaveBeenCalled();
		notReady.view.unmount();
		setup();
		for (const name of ["Beam opening angle", "Focus position"])
			expect(screen.getByRole("slider", { name })).not.toHaveAttribute("aria-disabled");
	});

	it("targets a selected group by id and refuses quietly until the lane is ready", () => {
		const grouped = setup({ groupId: "group-1" });
		fireEvent.keyDown(focusSlider(), { key: "End" });
		expect(intents(grouped.normal)[0]).toMatchObject({ fixtureIds: [], groupId: "group-1", operation: scalar("focus", 1) });
		grouped.view.unmount();
		const notReady = setup({ ready: false });
		fireEvent.keyDown(focusSlider(), { key: "End" });
		expect(notReady.normal.applyIntent).not.toHaveBeenCalled();
	});
});
