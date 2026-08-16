import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CadViewport } from "./CadViewport";
import type { CadEntity } from "./types";

const fixture: CadEntity = {
	id: "11111111-1111-4111-8111-111111111111",
	name: "Profile Stage 1",
	fixtureNumber: 101,
	fixtureDisplayId: "101",
	dmxAddress: "1.1",
	kind: "profile",
	fixtureType: "moving_head_profile",
	drawingId: "profile:1",
	layerId: "default",
	selectable: true,
	positionMillimetres: [0, 0, 4000],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [400, 500, 700],
	outputDirection: [0, 1, 0],
};

const camera = { pan: [0, 0] as [number, number], zoom: 0.1 };

beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});

function setup(
	selectedIds: readonly string[] = [],
	entity: CadEntity = fixture,
	labels = { fixtureIds: false, dmxAddresses: false },
) {
	const onSelection = vi.fn();
	const onPreview = vi.fn();
	const onMove = vi.fn().mockResolvedValue(undefined);
	render(
		<CadViewport
			entities={[entity]}
			drawings={[]}
			selectedIds={selectedIds}
			view="top_down"
			rotationQuarterTurns={0}
			camera={camera}
			preview={null}
			showFixtureIds={labels.fixtureIds}
			showDmxAddresses={labels.dmxAddresses}
			onCamera={() => undefined}
			onSelection={onSelection}
			onPreview={onPreview}
			onMove={onMove}
		/>,
	);
	const canvas = screen.getByLabelText(
		"CAD top down viewport",
	) as HTMLCanvasElement;
	Object.defineProperty(canvas, "getBoundingClientRect", {
		value: () => ({
			left: 0,
			top: 0,
			width: 1000,
			height: 800,
			right: 1000,
			bottom: 800,
		}),
	});
	Object.defineProperty(canvas, "clientWidth", { value: 1000 });
	Object.defineProperty(canvas, "clientHeight", { value: 800 });
	Object.defineProperty(canvas, "setPointerCapture", { value: vi.fn() });
	Object.defineProperty(canvas, "releasePointerCapture", { value: vi.fn() });
	return { canvas, onSelection, onPreview, onMove };
}

describe("CAD fixture interaction", () => {
	it("selects fixture geometry without starting a transform drag", () => {
		const { canvas, onSelection, onMove } = setup();
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 560,
			clientY: 400,
		});
		expect(onSelection).toHaveBeenCalledWith({
			type: "replace",
			ids: [fixture.id],
		});
		expect(onMove).not.toHaveBeenCalled();
	});

	it("supports Shift toggling and drag-box selection", () => {
		const { canvas, onSelection } = setup([fixture.id]);
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			shiftKey: true,
			clientX: 500,
			clientY: 400,
		});
		expect(onSelection).toHaveBeenCalledWith({
			type: "toggle",
			ids: [fixture.id],
		});
		fireEvent.pointerDown(canvas, {
			pointerId: 2,
			button: 0,
			clientX: 450,
			clientY: 450,
		});
		fireEvent.pointerMove(canvas, { pointerId: 2, clientX: 550, clientY: 350 });
		fireEvent.pointerUp(canvas, {
			pointerId: 2,
			button: 0,
			clientX: 550,
			clientY: 350,
		});
		expect(onSelection).toHaveBeenLastCalledWith({
			type: "replace",
			ids: [fixture.id],
		});
	});

	it("moves only from the gizmo and constrains an axis-arrow drag", async () => {
		const { canvas, onMove, onPreview } = setup([fixture.id]);
		// The right arrow starts at the gizmo origin, 36 screen pixels right and above the fixture.
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 558,
			clientY: 364,
		});
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 598, clientY: 390 });
		expect(onPreview).toHaveBeenCalledWith({
			entityIds: [fixture.id],
			deltaMillimetres: [400, 0, 0],
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 598,
			clientY: 390,
		});
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([400, 0, 0], [fixture.id]),
		);
	});

	it("does not select locked entities and renders optional operator labels", () => {
		const locked = { ...fixture, selectable: false };
		const { canvas, onSelection } = setup([], locked, {
			fixtureIds: true,
			dmxAddresses: true,
		});
		expect(screen.getByText("ID 101 · DMX 1.1")).toBeInTheDocument();
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 500,
			clientY: 400,
		});
		expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: [] });
	});
});
