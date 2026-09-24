import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	CadRigOverview,
	CadViewport,
	cadScaleForZoom,
	formatCadScale,
} from "./CadViewport";
import {
	cadEntityOutlineColor,
	observeViewportResize,
	renderDepthMaskedLinework,
} from "./lineRenderer";
import { fitCadOverview, viewportGuideRange } from "./planGeometry";
import type { CadEntity, CadViewDirection } from "./types";

const fixture: CadEntity = {
	id: "11111111-1111-4111-8111-111111111111",
	logicalFixtureId: "11111111-1111-4111-8111-111111111111",
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

describe("CAD viewport scale", () => {
	it("selects and formats the nearest distance from the metric 1-2-5 sequence", () => {
		expect(cadScaleForZoom(0.5)).toEqual({
			distanceMillimetres: 200,
			pixelWidth: 100,
			label: "20 cm",
		});
		expect(cadScaleForZoom(0.1)).toEqual({
			distanceMillimetres: 1000,
			pixelWidth: 100,
			label: "1 m",
		});
		expect(cadScaleForZoom(0.02)).toEqual({
			distanceMillimetres: 5000,
			pixelWidth: 100,
			label: "5 m",
		});
		expect(formatCadScale(10)).toBe("1 cm");
		expect(formatCadScale(500)).toBe("50 cm");
		expect(formatCadScale(10_000)).toBe("10 m");
	});

	it("rerenders its label and five graduated ticks with the viewport zoom", () => {
		const props = {
			entities: [fixture],
			drawings: [],
			selectedIds: [],
			view: "top_down" as const,
			rotationQuarterTurns: 0,
			preview: null,
			showFixtureIds: false,
			showDmxAddresses: false,
			onCamera: vi.fn(),
			onSelection: vi.fn(),
			onPreview: vi.fn(),
			onMove: vi.fn().mockResolvedValue(undefined),
		};
		const { container, rerender } = render(
			<CadViewport {...props} camera={{ pan: [0, 0], zoom: 0.12 }} />,
		);
		const scale = screen.getByLabelText("Scale 1 m");
		expect(scale).toHaveStyle({ width: "120px" });
		expect(container.querySelectorAll(".cad-viewport-scale-tick")).toHaveLength(
			5,
		);
		expect(container.querySelector(".is-start")).toBeInTheDocument();
		expect(container.querySelector(".is-quarter")).toBeInTheDocument();
		expect(container.querySelector(".is-middle")).toBeInTheDocument();
		expect(container.querySelector(".is-three-quarter")).toBeInTheDocument();
		expect(container.querySelector(".is-end")).toBeInTheDocument();

		rerender(<CadViewport {...props} camera={{ pan: [0, 0], zoom: 0.08 }} />);
		expect(screen.getByLabelText("Scale 2 m")).toHaveStyle({ width: "160px" });
	});
});

it("draws locked CAD entities darker while retaining cyan selection", () => {
	expect(
		cadEntityOutlineColor({ kind: "venue", selectable: false }, false),
	).toEqual([0.24, 0.27, 0.3]);
	expect(
		cadEntityOutlineColor({ kind: "venue", selectable: false }, true),
	).toEqual([0.02, 0.82, 0.98]);
});

it("marks a member of a whole selected group in its own violet, a lone pick in cyan", () => {
	const venue = { kind: "venue", selectable: true };
	expect(cadEntityOutlineColor(venue, true, true)).toEqual([0.74, 0.47, 1.0]);
	expect(cadEntityOutlineColor(venue, true, false)).toEqual([0.02, 0.82, 0.98]);
	// Grouping never colours an unselected element, and a locked one keeps its own grey.
	expect(cadEntityOutlineColor(venue, false, true)).toEqual([0.56, 0.62, 0.68]);
	expect(
		cadEntityOutlineColor({ kind: "venue", selectable: false }, false, true),
	).toEqual([0.24, 0.27, 0.3]);
});

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
	options: {
		snapping?: boolean;
		expandSelection?: (ids: readonly string[]) => string[];
		view?: CadViewDirection;
	} = {},
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
			{...options}
		/>,
	);
	const canvas = screen.getByLabelText(
		`CAD ${(options.view ?? "top_down").replaceAll("_", " ")} viewport`,
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

	it("keeps dotted guide bounds beyond both viewport edges while panned and zoomed", () => {
		const panned = { pan: [4200, -1700] as [number, number], zoom: 0.5 };
		const horizontal = viewportGuideRange(true, panned, 1000, 800);
		const visibleHorizontal = [
			-panned.pan[0] - 1000 / (2 * panned.zoom),
			-panned.pan[0] + 1000 / (2 * panned.zoom),
		];
		expect(horizontal[0]).toBeLessThan(visibleHorizontal[0]);
		expect(horizontal[1]).toBeGreaterThan(visibleHorizontal[1]);

		const vertical = viewportGuideRange(false, panned, 1000, 800);
		const visibleVertical = [
			-panned.pan[1] - 800 / (2 * panned.zoom),
			-panned.pan[1] + 800 / (2 * panned.zoom),
		];
		expect(vertical[0]).toBeLessThan(visibleVertical[0]);
		expect(vertical[1]).toBeGreaterThan(visibleVertical[1]);
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
			logicalFixtureId: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([], [fixture, second]);

		// Swept back to the left, so it takes everything the rectangle touches.
		fireEvent.pointerDown(canvas, {
			pointerId: 7,
			button: 0,
			clientX: 570,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 7,
			buttons: 1,
			clientX: 500,
			clientY: 350,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 7,
			button: 0,
			clientX: 500,
			clientY: 350,
		});

		expect(onSelection).toHaveBeenCalledTimes(1);
		expect(onSelection).toHaveBeenCalledWith({
			type: "replace",
			ids: [fixture.id, second.id],
		});
	});

	it("takes only what fits inside a rectangle drawn left to right", () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			logicalFixtureId: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([], [fixture, second]);

		// The same rectangle as the sweep above, drawn the other way: neither fixture fits
		// entirely inside it, so it catches nothing.
		fireEvent.pointerDown(canvas, {
			pointerId: 9,
			button: 0,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 9,
			buttons: 1,
			clientX: 570,
			clientY: 350,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 9,
			button: 0,
			clientX: 570,
			clientY: 350,
		});

		expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: [] });
	});

	it("selects a multi-patch placement through its shared logical fixture", () => {
		const copy = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			logicalFixtureId: fixture.id,
			name: "Profile Stage 1 copy",
			positionMillimetres: [1_000, 0, 4_000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([], [fixture, copy]);

		fireEvent.pointerDown(canvas, {
			pointerId: 17,
			button: 0,
			clientX: 600,
			clientY: 400,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 17,
			button: 0,
			clientX: 600,
			clientY: 400,
		});

		expect(onSelection).toHaveBeenCalledWith({
			type: "replace",
			ids: [fixture.id],
		});
	});

	it("adds a marquee begun inside a fixture while Shift is held", () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			logicalFixtureId: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const { canvas, onSelection } = setup([fixture.id], [fixture, second]);

		fireEvent.pointerDown(canvas, {
			pointerId: 8,
			button: 0,
			shiftKey: true,
			clientX: 570,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 8,
			buttons: 1,
			shiftKey: true,
			clientX: 500,
			clientY: 350,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 8,
			button: 0,
			shiftKey: true,
			clientX: 500,
			clientY: 350,
		});

		expect(onSelection).toHaveBeenCalledWith({
			type: "add",
			ids: [fixture.id, second.id],
		});
	});

	it("always pans the whole view with a middle-button drag", () => {
		const { canvas, onCamera, onSelection } = setup([], fixture, {
			fixtureIds: true,
			dmxAddresses: false,
		});
		const label = screen.getByText("ID 101");
		expect(label.style.left).toBe("calc(50% + 0px)");
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
		// The pan in flight is drawn at once by the viewport — labels with it — without handing every
		// move to the layout, which hears of the camera once the drag ends.
		expect(label.style.left).toBe("calc(50% + 30px)");
		expect(label.style.top).toBe("calc(50% + 20px)");
		expect(onCamera).not.toHaveBeenCalled();
		fireEvent.pointerUp(canvas, { pointerId: 4, button: 1, clientX: 530, clientY: 420 });
		expect(onCamera).toHaveBeenCalledOnce();
		expect(onCamera).toHaveBeenLastCalledWith({
			pan: [300, -200],
			zoom: 0.1,
		});
		expect(onSelection).not.toHaveBeenCalled();
	});

	it("moves only from the gizmo and constrains an axis-arrow drag", async () => {
		const { canvas, onMove, onPreview } = setup([fixture.id]);
		// The gizmo stands on the fixture's origin; the press is on its right arrow, clear of the square.
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 537,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 577, clientY: 426 });
		expect(onPreview).toHaveBeenCalledWith({
			entityIds: [fixture.id],
			deltaMillimetres: [400, 0, 0],
			spread: false,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			clientX: 577,
			clientY: 426,
		});
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([400, 0, 0], [fixture.id], false, true),
		);
	});

	it("shows the live position beside the gizmo while a move is in flight", () => {
		const { canvas } = setup([fixture.id]);
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 537, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 577, clientY: 426 });
		const readout = screen.getByRole("status", { name: "Move position" });
		expect(readout).toHaveTextContent("X 0.400 m");
		expect(readout).toHaveTextContent("Y 0.000 m");
		expect(readout.style.left).toBe("calc(50% + 40px)");
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: 577, clientY: 426 });
		expect(screen.queryByRole("status", { name: "Move position" })).toBeNull();
	});

	it("sets a typed coordinate on the dragged axis and moves by a signed one", async () => {
		const { canvas, onMove } = setup([fixture.id]);
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 537, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 577, clientY: 400 });
		for (const key of ["2", ",", "5"]) fireEvent.keyDown(window, { key });
		expect(screen.getByRole("status", { name: "Move position" })).toHaveTextContent(
			"X 2.500 m",
		);
		fireEvent.keyDown(window, { key: "Enter" });
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([2500, 0, 0], [fixture.id], false, false),
		);
		// The release that follows commits nothing more.
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: 577, clientY: 400 });
		expect(onMove).toHaveBeenCalledOnce();

		fireEvent.pointerDown(canvas, { pointerId: 2, button: 0, clientX: 537, clientY: 400 });
		for (const key of ["-", "1", ".", "5", "Enter"]) fireEvent.keyDown(window, { key });
		await waitFor(() =>
			expect(onMove).toHaveBeenLastCalledWith([-1500, 0, 0], [fixture.id], false, false),
		);
	});

	it("moves nothing on half-typed input and lets Escape clear it, then abandon the move", async () => {
		const { canvas, onMove, onPreview } = setup([fixture.id]);
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 537, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 577, clientY: 400 });
		for (const key of ["+", "Enter"]) fireEvent.keyDown(window, { key });
		const readout = screen.getByRole("status", { name: "Move position" });
		expect(readout).toHaveTextContent("not a number yet");
		expect(onMove).not.toHaveBeenCalled();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(readout).not.toHaveTextContent("not a number yet");
		fireEvent.keyDown(window, { key: "Escape" });
		expect(screen.queryByRole("status", { name: "Move position" })).toBeNull();
		expect(onPreview).toHaveBeenLastCalledWith(null);

		// Letting go mid-entry never commits the half-typed value.
		fireEvent.pointerDown(canvas, { pointerId: 2, button: 0, clientX: 537, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 2, clientX: 577, clientY: 400 });
		for (const key of ["1", ".", "."]) fireEvent.keyDown(window, { key });
		fireEvent.pointerUp(canvas, { pointerId: 2, button: 0, clientX: 577, clientY: 400 });
		await Promise.resolve();
		expect(onMove).not.toHaveBeenCalled();
	});

	it("turns the selection with the gizmo's rotate arc, in 15° steps unless Shift is held", async () => {
		const onTransforms = vi.fn().mockResolvedValue(undefined);
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
				onCamera={vi.fn()}
				onSelection={vi.fn()}
				onPreview={vi.fn()}
				onMove={vi.fn()}
				onTransforms={onTransforms}
			/>,
		);
		const canvas = screen.getByLabelText("CAD top down viewport") as HTMLCanvasElement;
		Object.defineProperty(canvas, "getBoundingClientRect", {
			value: () => ({ left: 0, top: 0, width: 1000, height: 800, right: 1000, bottom: 800 }),
		});
		Object.defineProperty(canvas, "setPointerCapture", { value: vi.fn() });
		Object.defineProperty(canvas, "releasePointerCapture", { value: vi.fn() });
		// The gizmo stands on the fixture at the centre; its arc is 0.62 of the 48 px arrows out.
		const radius = 48 * 0.62;
		const at = (degrees: number) => ({
			clientX: 500 + radius * Math.cos((degrees * Math.PI) / 180),
			clientY: 400 - radius * Math.sin((degrees * Math.PI) / 180),
		});
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, ...at(45) });
		fireEvent.pointerMove(canvas, { pointerId: 1, ...at(128) });
		const readout = screen.getByRole("status", { name: "Rotation" });
		expect(readout).toHaveTextContent(/Rotation Z [+−]90°/u);
		// Shift turns freely: 83° rather than the 90° the 15° steps give.
		fireEvent.pointerMove(canvas, { pointerId: 1, ...at(128), shiftKey: true });
		expect(readout).toHaveTextContent(/83°/u);
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, ...at(128) });
		await waitFor(() => expect(onTransforms).toHaveBeenCalledTimes(1));
		const [[placement]] = onTransforms.mock.calls[0];
		// One fixture turns about itself: it stays where it stands and only its Z turns, 90° either
		// way as the page is drawn.
		expect(placement.id).toBe(fixture.id);
		expect(placement.positionMillimetres).toEqual(fixture.positionMillimetres);
		expect(Math.abs(placement.rotationDegrees[2])).toBe(90);
		expect(placement.rotationDegrees.slice(0, 2)).toEqual([0, 0]);
		expect(screen.queryByRole("status", { name: "Rotation" })).toBeNull();
	});

	it("keeps Backspace inside a move even with nothing typed, so it never deletes what moves", () => {
		const { canvas } = setup([fixture.id]);
		const heard = vi.fn();
		window.addEventListener("keydown", heard);
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 537, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 577, clientY: 400 });
		expect(fireEvent.keyDown(window, { key: "Backspace" })).toBe(false);
		expect(heard).not.toHaveBeenCalled();
		window.removeEventListener("keydown", heard);
	});

	it("types onto the world axis an elevation view shows upward, and Tab picks the axis of a free drag", async () => {
		const { canvas, onMove } = setup([fixture.id], fixture, undefined, {
			view: "front_to_back",
		});
		// The fixture hangs at 4 m, the top edge of this tile; its upward arrow is above it.
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 500, clientY: -37 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 500, clientY: -47 });
		expect(screen.getByRole("status", { name: "Move position" })).toHaveTextContent(
			"Z 4.100 m",
		);
		for (const key of ["6", "Enter"]) fireEvent.keyDown(window, { key });
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([0, 0, 2000], [fixture.id], false, false),
		);

		fireEvent.pointerDown(canvas, { pointerId: 2, button: 0, clientX: 500, clientY: 0 });
		fireEvent.pointerMove(canvas, { pointerId: 2, clientX: 520, clientY: 0 });
		for (const key of ["Tab", "3", "Enter"]) fireEvent.keyDown(window, { key });
		await waitFor(() =>
			expect(onMove).toHaveBeenLastCalledWith([200, 0, -1000], [fixture.id], false, false),
		);
	});

	it("selects the element under the gizmo square when a press there never moves", async () => {
		const { canvas, onMove, onSelection } = setup([fixture.id]);
		fireEvent.pointerDown(canvas, { pointerId: 2, button: 0, clientX: 500, clientY: 400 });
		fireEvent.pointerUp(canvas, { pointerId: 2, button: 0, clientX: 500, clientY: 400 });
		await waitFor(() =>
			expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: [fixture.id] }),
		);
		expect(onMove).not.toHaveBeenCalled();
	});

	it("spreads an axis drag in selection order while Shift is held", async () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			logicalFixtureId: "22222222-2222-4222-8222-222222222222",
			name: "Profile Stage 2",
		};
		const third = {
			...fixture,
			id: "33333333-3333-4333-8333-333333333333",
			logicalFixtureId: "33333333-3333-4333-8333-333333333333",
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
			clientX: 537,
			clientY: 400,
		});
		fireEvent.pointerMove(canvas, {
			pointerId: 1,
			clientX: 577,
			clientY: 426,
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
			clientX: 577,
			clientY: 426,
			shiftKey: true,
		});
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([400, 0, 0], orderedSelection, true, false),
		);
		expect(onMove).toHaveBeenCalledTimes(1);
	});

	it("selects a grouped element's whole group, and the element alone with Shift", () => {
		const second = {
			...fixture,
			id: "22222222-2222-4222-8222-222222222222",
			logicalFixtureId: "22222222-2222-4222-8222-222222222222",
			positionMillimetres: [600, 0, 4000] as [number, number, number],
		};
		const group = [fixture.id, second.id];
		const { canvas, onSelection } = setup([], [fixture, second], undefined, {
			expandSelection: (ids) => (ids.some((id) => group.includes(id)) ? group : [...ids]),
		});
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 500, clientY: 400 });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: 500, clientY: 400 });
		expect(onSelection).toHaveBeenLastCalledWith({ type: "replace", ids: group });
		fireEvent.pointerDown(canvas, {
			pointerId: 2,
			button: 0,
			shiftKey: true,
			clientX: 500,
			clientY: 400,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 2,
			button: 0,
			shiftKey: true,
			clientX: 500,
			clientY: 400,
		});
		expect(onSelection).toHaveBeenLastCalledWith({ type: "toggle", ids: [fixture.id] });
	});

	it("snaps a dragged truss onto the next one's connector unless Shift is held", async () => {
		const truss = (id: string, x: number): CadEntity => ({
			...fixture,
			id,
			logicalFixtureId: id,
			kind: "venue",
			fixtureType: "rigging",
			positionMillimetres: [x, 0, 0],
			sizeMillimetres: [4000, 340, 340],
			scenery: { kind: "truss", chords: 4, pattern: "standard" },
		});
		const still = truss("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", 0);
		const moving = truss("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", 4100);
		const { canvas, onPreview, onMove } = setup([moving.id], [still, moving], undefined, {
			snapping: true,
		});
		// The gizmo square stands on the moving truss's origin, 410 px right of the centre.
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 910, clientY: 400 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 905, clientY: 402 });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: [moving.id],
			deltaMillimetres: [-100, 0, 0],
			spread: false,
		});
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 905, clientY: 402, shiftKey: true });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: [moving.id],
			deltaMillimetres: [-50, -20, 0],
			spread: false,
		});
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: 905, clientY: 402 });
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([-100, 0, 0], [moving.id], false, true),
		);
	});

	it("butts a dragged stage deck against its neighbour's side and draws the joined side", async () => {
		const deck = (id: string, position: [number, number, number], width: number): CadEntity => ({
			...fixture,
			id,
			logicalFixtureId: id,
			kind: "venue",
			fixtureType: "venue",
			fixtureProfile: "Venue Stage Deck",
			positionMillimetres: position,
			sizeMillimetres: [width, 1000, 400],
			scenery: { kind: "riser", chords: 0, pattern: "standard" },
		});
		const still = deck("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", [0, 0, 0], 2000);
		const moving = deck("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", [300, 1040, 0], 1000);
		const { canvas, onPreview, onMove } = setup([moving.id], [still, moving], undefined, {
			snapping: true,
		});
		// An ordinary drag of the gizmo, which stands on the deck's position (530, 296 on screen).
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 530, clientY: 296 });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 531, clientY: 297 });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: [moving.id],
			deltaMillimetres: [10, -40, 0],
			spread: false,
		});
		expect(canvas).toHaveAttribute("data-snap-guides", "1");
		expect(canvas).toHaveAttribute("data-snap-markers", "1");
		// Shift places freely: no snap and nothing marked.
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 531, clientY: 297, shiftKey: true });
		expect(onPreview).toHaveBeenLastCalledWith({
			entityIds: [moving.id],
			deltaMillimetres: [10, -10, 0],
			spread: false,
		});
		expect(canvas).toHaveAttribute("data-snap-guides", "0");
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: 531, clientY: 297 });
		await waitFor(() =>
			expect(onMove).toHaveBeenCalledWith([10, -40, 0], [moving.id], false, true),
		);
		expect(canvas).toHaveAttribute("data-snap-guides", "0");
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

	it("zooms when the wheel turns over a print page frame", async () => {
		const onCamera = vi.fn();
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
						orientation: "landscape",
						showFixtureIds: false,
						showDmxAddresses: false,
						showMountingHardware: true,
					},
				]}
				selectedPrintPageId="page-1"
				onCamera={onCamera}
				onSelection={vi.fn()}
				onPreview={() => undefined}
				onMove={vi.fn()}
				onSelectPrintPage={() => undefined}
				onChangePrintPage={vi.fn()}
			/>,
		);
		// A page frame is a sibling of the canvas, so a wheel over it only reaches a handler
		// that sits on the viewport they share.
		const frame = screen.getByText("Page 1").parentElement as HTMLElement;
		fireEvent.wheel(frame, { deltaY: -120 });
		// The zoom shows at once; the layout is told once the wheel has come to rest.
		expect(screen.getByLabelText("Scale 1 m").style.width).not.toBe("100px");
		expect(onCamera).not.toHaveBeenCalled();
		await waitFor(() => expect(onCamera).toHaveBeenCalledTimes(1));
		expect(onCamera.mock.calls[0][0].zoom).toBeGreaterThan(camera.zoom);
	});

	it("moves and uniformly scales print frames while rig editing is disabled", async () => {
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
						orientation: "landscape",
						showFixtureIds: false,
						showDmxAddresses: false,
						showMountingHardware: true,
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
		await waitFor(() =>
			expect(onCamera).toHaveBeenLastCalledWith({ pan: [200, -100], zoom: 0.1 }),
		);
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
						orientation: "landscape",
						showFixtureIds: false,
						showDmxAddresses: false,
						showMountingHardware: true,
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
