import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PrintFrame } from "./CadPrintFrame";
import {
	pageFrameDelta,
	pageFrameGeometry,
	withPageViewRotation,
} from "./printFrameGeometry";
import { type CadPrintPage, planeDelta, projectPoint } from "./types";

function page(change: Partial<CadPrintPage> = {}): CadPrintPage {
	return {
		kind: "plan",
		id: "page-5",
		tileId: "top-tile",
		name: "Page 5",
		view: "top_down",
		rotationQuarterTurns: 0,
		centreMillimetres: [-1000, 3500],
		widthMillimetres: 12000,
		included: true,
		orientation: "landscape",
		showFixtureIds: true,
		showDmxAddresses: true,
		showMountingHardware: false,
		hiddenUnderlayIds: ["roof"],
		...change,
	};
}

function expectPoint(actual: readonly number[], expected: readonly number[]) {
	expect(actual).toHaveLength(expected.length);
	for (const [index, value] of expected.entries()) {
		expect(actual[index]).toBeCloseTo(value, 8);
	}
}

function normaliseDegrees(angle: number) {
	return ((angle % 360) + 360) % 360;
}

describe("saved print frames in the current viewport", () => {
	for (const savedRotation of [0, 1, 2, 3]) {
		for (const viewportRotation of [0, 1, 2, 3]) {
			it(`keeps a world anchor aligned for page ${savedRotation} in viewport ${viewportRotation}`, () => {
				const worldCentre: [number, number, number] = [-1000, 3500, 0];
				const saved = page({
					rotationQuarterTurns: savedRotation,
					centreMillimetres: projectPoint(worldCentre, "top_down", savedRotation),
				});
				const frame = pageFrameGeometry(saved, viewportRotation);
				expectPoint(frame.centreMillimetres, projectPoint(worldCentre, "top_down", viewportRotation));
				expect(normaliseDegrees(frame.rotationDegrees)).toBe(normaliseDegrees((viewportRotation - savedRotation) * 90));

				// A fixture offset on the saved sheet must land on the same screen point as the rig.
				const worldFixture: [number, number, number] = [-300, 3900, 0];
				const savedFixture = projectPoint(worldFixture, "top_down", savedRotation);
				const offset = [savedFixture[0] - saved.centreMillimetres[0], savedFixture[1] - saved.centreMillimetres[1]];
				const radians = frame.rotationDegrees * Math.PI / 180;
				const screenX = offset[0] * Math.cos(radians) + offset[1] * Math.sin(radians);
				const screenYUp = -offset[0] * Math.sin(radians) + offset[1] * Math.cos(radians);
				expectPoint([frame.centreMillimetres[0] + screenX, frame.centreMillimetres[1] + screenYUp], projectPoint(worldFixture, "top_down", viewportRotation));
			});
		}
	}

	it("converts a rightward viewport drag into the saved page's positive Y axis", () => {
		expectPoint(pageFrameDelta(page(), 1, [250, 0]), [0, 250]);
		expectPoint(pageFrameDelta(page({ rotationQuarterTurns: 1 }), 0, [250, 0]), [0, -250]);
	});

	it("round-trips drag movement for every saved and current quarter turn", () => {
		for (const savedRotation of [0, 1, 2, 3]) {
			for (const viewportRotation of [0, 1, 2, 3]) {
				const saved = page({ rotationQuarterTurns: savedRotation });
				const delta = pageFrameDelta(saved, viewportRotation, [125, -75]);
				const moved = { ...saved, centreMillimetres: [saved.centreMillimetres[0] + delta[0], saved.centreMillimetres[1] + delta[1]] as [number, number] };
				const before = pageFrameGeometry(saved, viewportRotation).centreMillimetres;
				const after = pageFrameGeometry(moved, viewportRotation).centreMillimetres;
				expectPoint([after[0] - before[0], after[1] - before[1]], [125, -75]);
			}
		}
	});

	it("changes Page 5 from rotation 2 to Page 4's rotation 0 without moving its world centre", () => {
		const fifth = page({ rotationQuarterTurns: 2, orientation: "portrait" });
		const fourth = page({ id: "page-4", name: "Page 4", rotationQuarterTurns: 0 });
		const changed = withPageViewRotation(fifth, fourth.rotationQuarterTurns);
		expect(changed).toEqual({ ...fifth, rotationQuarterTurns: 0, centreMillimetres: [1000, -3500] });
		expect(fifth.centreMillimetres).toEqual([-1000, 3500]);
		expectPoint(planeDelta(changed.centreMillimetres, changed.view, changed.rotationQuarterTurns), planeDelta(fifth.centreMillimetres, fifth.view, fifth.rotationQuarterTurns));
		expect(withPageViewRotation(changed, 2)).toEqual(fifth);
	});

	it("retains the page centre when its saved rotation already matches", () => {
		const saved = page({ rotationQuarterTurns: 3 });
		expect(withPageViewRotation(saved, 3)).toEqual(saved);
	});
});


describe("rotated print frame controls", () => {
	afterEach(cleanup);

	it("draws and drags the entire sheet in its current viewport basis", () => {
		const onChange = vi.fn();
		const onCamera = vi.fn();
		const saved = page({ widthMillimetres: 5940 });
		render(createElement(PrintFrame, {
			page: saved, entities: [], underlays: [],
			camera: { pan: [100, -50], zoom: 0.1 }, viewportRotation: 1,
			selected: true, onSelect: vi.fn(), onChange, onCamera,
		}));
		const frame = screen.getByText("Page 5").closest(".cad-print-frame") as HTMLElement;
		expect(frame.style.transform).toBe("rotate(90deg)");
		expect(frame.style.transformOrigin).toBe("center");
		expect(frame.style.left).toBe("calc(50% + 63px)");
		expect(frame.style.top).toBe("calc(50% - 305px)");
		expect(frame.style.width).toBe("594px");
		expect(frame.style.height).toBe("420px");
		fireEvent.pointerDown(frame, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(frame, { pointerId: 1, clientX: 120, clientY: 110 });
		expect(onChange).toHaveBeenLastCalledWith({ centreMillimetres: [-900, 3700] });
		fireEvent.pointerUp(frame, { pointerId: 1 });
		onChange.mockClear();
		fireEvent.pointerDown(frame, { pointerId: 2, button: 1, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(frame, { pointerId: 2, buttons: 4, clientX: 120, clientY: 110 });
		expect(onCamera).toHaveBeenLastCalledWith({ pan: [300, -150], zoom: 0.1 });
		expect(onChange).not.toHaveBeenCalled();
	});

	it("locks the physical resize handle at a fixed scale while the sheet remains movable", () => {
		const onChange = vi.fn();
		render(createElement(PrintFrame, {
			page: page({ fixedScaleDenominator: 75 }), entities: [], underlays: [],
			camera: { pan: [0, 0], zoom: 0.1 }, selected: true,
			onSelect: vi.fn(), onChange, onCamera: vi.fn(),
		}));
		const handle = screen.getByRole("button", { name: "Scale locked at 1:75" });
		expect(handle).toHaveProperty("disabled", true);
		fireEvent.pointerDown(handle, { pointerId: 4, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(handle, { pointerId: 4, clientX: 150, clientY: 150 });
		expect(onChange).not.toHaveBeenCalled();
		const frame = screen.getByText("Page 5").closest(".cad-print-frame") as HTMLElement;
		fireEvent.pointerDown(frame, { pointerId: 5, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(frame, { pointerId: 5, clientX: 120, clientY: 110 });
		expect(onChange).toHaveBeenLastCalledWith({ centreMillimetres: [-800, 3400] });
	});

	it("resizes a portrait sheet through its rotated physical corner handle", () => {
		const onChange = vi.fn();
		render(createElement(PrintFrame, {
			page: page({ orientation: "portrait", widthMillimetres: 4200 }), entities: [], underlays: [],
			camera: { pan: [0, 0], zoom: 0.1 }, viewportRotation: 1,
			selected: true, onSelect: vi.fn(), onChange, onCamera: vi.fn(),
		}));
		const handle = screen.getByRole("button", { name: "Scale Page 5" });
		fireEvent.pointerDown(handle, { pointerId: 3, button: 0, clientX: 100, clientY: 100 });
		// The saved sheet's lower right moves down and left after the viewport's quarter turn.
		fireEvent.pointerMove(handle, { pointerId: 3, clientX: 80, clientY: 110 });
		expect(onChange).toHaveBeenLastCalledWith({ widthMillimetres: 4200 + 400 * 210 / 297 });
	});
});
