import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	CadRigOverview,
	CadViewport,
	fitCadOverview,
	observeViewportResize,
	renderDepthMaskedLinework,
} from "./CadViewport";
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
	entity: CadEntity | readonly CadEntity[] = fixture,
	labels = { fixtureIds: false, dmxAddresses: false },
) {
	const onSelection = vi.fn();
	const onPreview = vi.fn();
	const onMove = vi.fn().mockResolvedValue(undefined);
	const onCamera = vi.fn();
	render(
		<CadViewport
			entities={Array.isArray(entity) ? entity : [entity]}
			drawings={[]}
			selectedIds={selectedIds}
			view="top_down"
			rotationQuarterTurns={0}
			camera={camera}
			preview={null}
			showFixtureIds={labels.fixtureIds}
			showDmxAddresses={labels.dmxAddresses}
			onCamera={onCamera}
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
	return { canvas, onCamera, onSelection, onPreview, onMove };
}

describe("CAD fixture interaction", () => {
	it("shows the floor datum only in elevation views and exposes the origin setting", () => {
		const { rerender } = render(
			<CadViewport
				entities={[fixture]}
				drawings={[]}
				selectedIds={[]}
				view="left_to_right"
				rotationQuarterTurns={0}
				camera={camera}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				showCoordinateOrigins
				onCamera={vi.fn()}
				onSelection={vi.fn()}
				onPreview={vi.fn()}
				onMove={vi.fn()}
			/>,
		);
		let canvas = screen.getByLabelText("CAD left to right viewport");
		expect(canvas).toHaveAttribute("data-floor-datum", "visible");
		expect(canvas).toHaveAttribute("data-coordinate-origins", "visible");

		rerender(
			<CadViewport
				entities={[fixture]}
				drawings={[]}
				selectedIds={[]}
				view="top_down"
				rotationQuarterTurns={0}
				camera={camera}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				showCoordinateOrigins={false}
				onCamera={vi.fn()}
				onSelection={vi.fn()}
				onPreview={vi.fn()}
				onMove={vi.fn()}
			/>,
		);
		canvas = screen.getByLabelText("CAD top down viewport");
		expect(canvas).toHaveAttribute("data-floor-datum", "hidden");
		expect(canvas).toHaveAttribute("data-coordinate-origins", "hidden");
	});

	it("renders the Show overview in one fixed non-interactive orientation", () => {
		render(
			<CadRigOverview
				entities={[fixture]}
				drawings={[]}
				showName="Summer Tour"
			/>,
		);
		const overview = screen.getByRole("img", {
			name: "Read-only rig overview for Summer Tour",
		});
		expect(overview).toHaveAttribute("data-view", "top_down");
		expect(overview).toHaveAttribute("data-rotation-quarter-turns", "-1");
		expect(overview).toHaveAttribute("data-entity-count", "1");
		expect(overview).not.toHaveAttribute("tabindex");
	});

	it("refits the fixed overview camera to the rendered aspect ratio", () => {
		const large = fitCadOverview([fixture], new Map(), 1200, 600);
		const small = fitCadOverview([fixture], new Map(), 600, 300);
		expect(large.pan).toEqual(small.pan);
		expect(large.zoom).toBeGreaterThan(small.zoom);
	});

	it("redraws the viewport whenever its rendered size changes", () => {
		let notifyResize: ResizeObserverCallback | undefined;
		const disconnect = vi.fn();
		vi.stubGlobal(
			"ResizeObserver",
			class {
				constructor(callback: ResizeObserverCallback) {
					notifyResize = callback;
				}
				observe() {}
				disconnect() {
					disconnect();
				}
			},
		);
		const redraw = vi.fn();
		const stop = observeViewportResize(
			document.createElement("canvas"),
			redraw,
		);

		expect(redraw).toHaveBeenCalledTimes(1);
		notifyResize?.([], {} as ResizeObserver);
		expect(redraw).toHaveBeenCalledTimes(2);

		stop();
		expect(disconnect).toHaveBeenCalledTimes(1);
	});

	it("offsets invisible depth masks before drawing coplanar technical outlines", () => {
		const calls: string[] = [];
		const gl = {
			DEPTH_TEST: 1,
			LEQUAL: 2,
			POLYGON_OFFSET_FILL: 3,
			enable: (value: number) => calls.push(`enable:${value}`),
			disable: (value: number) => calls.push(`disable:${value}`),
			depthFunc: (value: number) => calls.push(`depth:${value}`),
			polygonOffset: (factor: number, units: number) =>
				calls.push(`offset:${factor}:${units}`),
		};

		renderDepthMaskedLinework(
			gl as unknown as WebGL2RenderingContext,
			() => calls.push("masks"),
			() => calls.push("lines"),
		);

		expect(calls).toEqual([
			"enable:1",
			"depth:2",
			"enable:3",
			"offset:1:1",
			"masks",
			"disable:3",
			"lines",
			"disable:1",
		]);
	});

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
		fireEvent.pointerUp(canvas, {
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

	it("turns a drag begun inside fixture geometry into marquee selection", () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([], [fixture, second]);

		fireEvent.pointerDown(canvas, {
			pointerId: 7,
			button: 0,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 7,
			buttons: 1,
			clientX: 570,
			clientY: 350,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 7,
			button: 0,
			clientX: 570,
			clientY: 350,
		});

		expect(onSelection).toHaveBeenCalledTimes(1);
		expect(onSelection).toHaveBeenCalledWith({
			type: "replace",
			ids: [fixture.id, second.id],
		});
	});

	it("adds a marquee begun inside a fixture while Shift is held", () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([fixture.id], [fixture, second]);

		fireEvent.pointerDown(canvas, {
			pointerId: 8,
			button: 0,
			shiftKey: true,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 8,
			buttons: 1,
			shiftKey: true,
			clientX: 570,
			clientY: 350,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 8,
			button: 0,
			shiftKey: true,
			clientX: 570,
			clientY: 350,
		});

		expect(onSelection).toHaveBeenCalledWith({
			type: "add",
			ids: [fixture.id, second.id],
		});
	});

	it("always pans the whole view with a middle-button drag", () => {
		const { canvas, onCamera, onSelection } = setup();
		fireEvent.pointerDown(canvas, {
			pointerId: 4,
			button: 1,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 4,
			buttons: 4,
			clientX: 530,
			clientY: 420,
		});
		expect(onCamera).toHaveBeenLastCalledWith({
			pan: [300, -200],
			zoom: 0.1,
		});
		expect(onSelection).not.toHaveBeenCalled();
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
			spread: false,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 598,
			clientY: 390,
		});
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([400, 0, 0], [fixture.id], false),
		);
	});

	it("spreads an axis drag in selection order while Shift is held", async () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			name: "Profile Stage 2",
		};
		const third = {
			...fixture,
			id: "33333333-3333-4333-8333-333333333333",
			name: "Profile Stage 3",
		};
		const orderedSelection = [third.id, fixture.id, second.id];
		const { canvas, onMove, onPreview } = setup(orderedSelection, [
			fixture,
			second,
			third,
		]);

		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 558,
			clientY: 364,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 1,
			clientX: 598,
			clientY: 390,
			shiftKey: true,
		});
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: orderedSelection,
			deltaMillimetres: [400, 0, 0],
			spread: true,
		});

		fireEvent.keyUp(window, { key: "Shift" });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: orderedSelection,
			deltaMillimetres: [400, 0, 0],
			spread: false,
		});
		fireEvent.keyDown(window, { key: "Shift" });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: orderedSelection,
			deltaMillimetres: [400, 0, 0],
			spread: true,
		});

		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 598,
			clientY: 390,
			shiftKey: true,
		});
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([400, 0, 0], orderedSelection, true),
		);
		expect(onMove).toHaveBeenCalledTimes(1);
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

	it("moves and uniformly scales print frames while rig editing is disabled", () => {
		const onSelection = vi.fn();
		const onChangePrintPage = vi.fn();
		const onCamera = vi.fn();
		render(
			<CadViewport
				entities={[fixture]}
				drawings={[]}
				selectedIds={[fixture.id]}
				view="top_down"
				rotationQuarterTurns={0}
				camera={camera}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				editEnabled={false}
				printMode
				printPages={[
					{
						id: "page-1",
						tileId: "tile",
						name: "Page 1",
						view: "top_down",
						rotationQuarterTurns: 0,
						centreMillimetres: [0, 0],
						widthMillimetres: 5000,
						included: true,
					},
				]}
				selectedPrintPageId="page-1"
				onCamera={onCamera}
				onSelection={onSelection}
				onPreview={() => undefined}
				onMove={vi.fn()}
				onSelectPrintPage={() => undefined}
				onChangePrintPage={onChangePrintPage}
			/>,
		);
		const frame = screen.getByText("Page 1").parentElement as HTMLElement;
		fireEvent.pointerDown(frame, { pointerId: 1, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(frame, { pointerId: 1, clientX: 120, clientY: 110 });
		expect(onChangePrintPage).toHaveBeenCalledWith("page-1", {
			centreMillimetres: [200, -100],
		});

		const scale = screen.getByRole("button", { name: "Scale Page 1" });
		fireEvent.pointerDown(scale, { pointerId: 2, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(scale, { pointerId: 2, clientX: 110, clientY: 110 });
		expect(onChangePrintPage).toHaveBeenLastCalledWith("page-1", {
			widthMillimetres: expect.any(Number),
		});
		const width = onChangePrintPage.mock.calls.at(-1)?.[1].widthMillimetres;
		expect(width).toBeGreaterThan(5000);

		onChangePrintPage.mockClear();
		fireEvent.pointerDown(frame, {
			pointerId: 3,
			button: 1,
			clientX: 100,
			clientY: 100,
		});
		fireEvent.pointerMove(frame, {
			pointerId: 3,
			buttons: 4,
			clientX: 120,
			clientY: 110,
		});
		expect(onCamera).toHaveBeenLastCalledWith({ pan: [200, -100], zoom: 0.1 });
		expect(onChangePrintPage).not.toHaveBeenCalled();
	});

	it("does not render persisted print pages outside Print mode", () => {
		render(
			<CadViewport
				entities={[fixture]}
				drawings={[]}
				selectedIds={[]}
				view="top_down"
				rotationQuarterTurns={0}
				camera={camera}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				printMode={false}
				printPages={[
					{
						id: "page-hidden",
						tileId: "tile",
						name: "Hidden Page",
						view: "top_down",
						rotationQuarterTurns: 0,
						centreMillimetres: [0, 0],
						widthMillimetres: 5000,
						included: true,
					},
				]}
				onCamera={() => undefined}
				onSelection={() => undefined}
				onPreview={() => undefined}
				onMove={vi.fn()}
			/>,
		);
		expect(screen.queryByText("Hidden Page")).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Scale Hidden Page" }),
		).not.toBeInTheDocument();
	});
});
