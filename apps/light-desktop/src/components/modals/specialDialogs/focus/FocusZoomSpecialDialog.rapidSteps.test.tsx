import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { inFlightLaneWriter } from "../../../../features/programmerValues/realWriterTestSupport";
import type { ProgrammerValueEntry } from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import {
	componentSlot,
	FIXTURE_A,
	FIXTURE_B,
} from "../../../control/parameterControls/familyEncoders/familyEncoderTestSupport";
import { FocusZoomSpecialDialog, type FocusZoomSpecialDialogProps } from "./FocusZoomSpecialDialog";
import { ZOOM_UNSUPPORTED_STATUS } from "./focusZoomDialogModel";

/**
 * TL-637 follow-up: rapid keyboard steps against the mounted lane writer. Every key press is one
 * complete gesture; a press while the previous step (or its Finish) is still in flight queues
 * behind it, in order, without loss and without a false "Requested · unsupported".
 */

const ZOOM = componentSlot("zoom", { kind: "zoom" }, {
	limits: { min: 10, max: 50 },
	limits_source: "selection",
	convention: "beam",
});
const FOCUS = componentSlot("focus", { kind: "focus" }, { limits_source: "descriptor" });

const zoomValue = (fixtureId: string, degrees: number): ProgrammerValueEntry => ({
	fixtureId,
	attribute: "zoom",
	value: { kind: "zoom", value: { opening_degrees: { kind: "value", value: degrees }, convention: "beam" } },
});

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

function setup(lane: "normal" | "preload") {
	const server = inFlightLaneWriter(lane);
	const environment: FocusZoomSpecialDialogProps["environment"] = {
		focusSlot: FOCUS,
		zoomSlot: ZOOM,
		lane,
		ready: true,
		groupId: null,
		timing: { fade: false, fadeMillis: null, delayMillis: null },
		programmerValues: [zoomValue(FIXTURE_A, 20), zoomValue(FIXTURE_B, 20)],
		writerFor: () => server.writer,
	};
	const view = render(<FocusZoomSpecialDialog environment={environment} close={vi.fn()} />);
	/** The store reflects an accepted step (a WebSocket projection) while others are in flight. */
	const reflect = (degrees: number) =>
		view.rerender(
			<FocusZoomSpecialDialog
				environment={{ ...environment, programmerValues: [zoomValue(FIXTURE_A, degrees), zoomValue(FIXTURE_B, degrees)] }}
				close={vi.fn()}
			/>,
		);
	return Object.assign(server, { reflect });
}

const setValue = (value: number) => ({
	type: "component_edits",
	edits: [{ kind: "scalar", component: { kind: "zoom" }, operation: { kind: "set", value: { kind: "value", value } } }],
});

describe("Focus Special Dialog: rapid keyboard steps", () => {
	for (const lane of ["normal", "preload"] as const) {
		it(`${lane}: three fast Zoom steps are all sent in order, each with its own Finish`, async () => {
			const server = setup(lane);
			const zoom = screen.getByRole("slider", { name: "Beam opening angle" });
			// The first step is in flight; the next two are pressed before anything settles.
			fireEvent.keyDown(zoom, { key: "ArrowUp" });
			fireEvent.keyDown(zoom, { key: "ArrowUp" });
			fireEvent.keyDown(zoom, { key: "ArrowUp" });
			expect(zoom).toHaveAttribute("aria-valuenow", "23");
			await act(server.drain);

			const sent = server.sent();
			expect(sent.map((action) => action.action)).toEqual([
				"apply_intent",
				"finish_gesture",
				"apply_intent",
				"finish_gesture",
				"apply_intent",
				"finish_gesture",
			]);
			expect(sent.filter((action) => action.action === "apply_intent").map((action) => action.operation)).toEqual([
				setValue(21),
				setValue(22),
				setValue(23),
			]);
			// Each step is its own gesture: its own Undo group, finished exactly once.
			const groups = sent.map((action) => action.undoGroup);
			expect(new Set(groups).size).toBe(3);
			expect(groups[0]).toBe(groups[1]);
			expect(groups[2]).toBe(groups[3]);
			expect(groups[4]).toBe(groups[5]);
			expect(screen.getByTestId("focus-zoom-zoom-status")).not.toHaveTextContent(ZOOM_UNSUPPORTED_STATUS);
			expect(server.onError.mock.calls.filter(([error]) => error instanceof Error)).toEqual([]);
		});
	}

	it("a reflection of an older step mid-burst never makes the next key repeat a step", async () => {
		const server = setup("normal");
		const zoom = screen.getByRole("slider", { name: "Beam opening angle" });
		fireEvent.keyDown(zoom, { key: "ArrowUp" });
		fireEvent.keyDown(zoom, { key: "ArrowUp" });
		// The first step (21) is reflected while the second (22) is still unanswered.
		server.reflect(21);
		expect(zoom).toHaveAttribute("aria-valuenow", "22");
		fireEvent.keyDown(zoom, { key: "ArrowUp" });
		await act(server.drain);
		expect(
			server.sent().filter((action) => action.action === "apply_intent").map((action) => action.operation),
		).toEqual([setValue(21), setValue(22), setValue(23)]);
		// Once every step is answered, the dialog shows the store's value again.
		server.reflect(23);
		expect(zoom).toHaveAttribute("aria-valuenow", "23");
	});
});

