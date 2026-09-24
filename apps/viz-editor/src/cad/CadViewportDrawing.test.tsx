import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CadAnnotation } from "./annotations";
import type { CadEntity } from "./types";
import { type CadDrawTool, CadToolContext, type CadTools } from "./cadTools";
import { CadViewport } from "./CadViewport";

/**
 * A 1000 × 800 canvas at zoom 0.1 with no pan: the screen centre (500, 400) is the plan origin and
 * every 100 screen pixels are a metre.
 */
function setup({
	tool,
	annotations = [],
	rotationQuarterTurns = 0,
	entities = [],
	snapping = false,
	selectedTextId = null,
	placing = null,
	textPreview = null,
}: {
	placing?: CadTools["placing"];
	tool: CadDrawTool;
	annotations?: CadAnnotation[];
	rotationQuarterTurns?: number;
	entities?: CadEntity[];
	snapping?: boolean;
	selectedTextId?: string | null;
	textPreview?: CadTools["textPreview"];
}) {
	const tools: CadTools = {
		onAdd: vi.fn(),
		tool,
		setTool: vi.fn(),
		annotations,
		save: vi.fn().mockResolvedValue(undefined),
		remove: vi.fn().mockResolvedValue(undefined),
		error: null,
		clearError: vi.fn(),
		placed: null,
		announcePlaced: vi.fn(),
		placing,
		startPlacing: vi.fn(),
		stopPlacing: vi.fn(),
		placeAt: vi.fn(),
		change: vi.fn().mockResolvedValue(undefined),
		selectedTextId: selectedTextId,
		selectText: vi.fn(),
		textPreview,
		setTextPreview: vi.fn(),
	};
	const onSelection = vi.fn();
	render(
		<CadToolContext.Provider value={tools}>
			<CadViewport
				entities={entities}
				snapping={snapping}
				drawings={[]}
				selectedIds={[]}
				view="top_down"
				rotationQuarterTurns={rotationQuarterTurns}
				camera={{ pan: [0, 0], zoom: 0.1 }}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				onCamera={vi.fn()}
				onSelection={onSelection}
				onPreview={vi.fn()}
				onMove={vi.fn().mockResolvedValue(undefined)}
			/>
		</CadToolContext.Provider>,
	);
	const canvas = screen.getByLabelText("CAD top down viewport");
	Object.defineProperty(canvas, "getBoundingClientRect", {
		value: () => ({ left: 0, top: 0, width: 1000, height: 800, right: 1000, bottom: 800 }),
	});
	Object.defineProperty(canvas, "setPointerCapture", { value: vi.fn() });
	Object.defineProperty(canvas, "releasePointerCapture", { value: vi.fn() });
	const click = (clientX: number, clientY: number, shiftKey = false) => {
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX, clientY, shiftKey });
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX, clientY, shiftKey });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX, clientY, shiftKey });
	};
	const drag = (from: [number, number], to: [number, number]) => {
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: from[0], clientY: from[1] });
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: to[0], clientY: to[1] });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX: to[0], clientY: to[1] });
	};
	return { tools, canvas, click, drag, onSelection };
}

beforeEach(() => {
	// jsdom has neither a resize observer nor WebGL; the drawing is checked through what is saved.
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});

function saved(tools: CadTools) {
	return vi.mocked(tools.save).mock.calls.map(([annotation]) => annotation);
}

describe("drawing on a CAD viewport", () => {
	it("draws a line point by point and finishes it with a double click", () => {
		const { tools, canvas, click, onSelection } = setup({ tool: "polyline" });
		click(500, 400);
		click(600, 400);
		click(600, 300);
		expect(tools.save).not.toHaveBeenCalled();
		fireEvent.doubleClick(canvas);
		expect(saved(tools)).toEqual([
			expect.objectContaining({
				id: "",
				view: "top_down",
				kind: "polyline",
				closed: false,
				points: [
					[0, 0],
					[1000, 0],
					[1000, 1000],
				],
			}),
		]);
		// Drawing is not selecting.
		expect(onSelection).not.toHaveBeenCalled();
	});

	it("closes a line when the last click lands on its first point, and Enter finishes an open one", () => {
		const closing = setup({ tool: "polyline" });
		closing.click(500, 400);
		closing.click(600, 400);
		closing.click(600, 300);
		closing.click(503, 402);
		expect(saved(closing.tools)).toEqual([
			expect.objectContaining({ closed: true, points: [[0, 0], [1000, 0], [1000, 1000]] }),
		]);
	});

	it("draws a box from a click on one corner and a click on the opposite one", () => {
		const { tools, canvas, click } = setup({ tool: "box" });
		click(500, 400);
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 650, clientY: 350 });
		expect(tools.save).not.toHaveBeenCalled();
		click(700, 300);
		expect(saved(tools)).toEqual([
			expect.objectContaining({ kind: "box", points: [[0, 0], [2000, 1000]] }),
		]);
		// The next click starts a new box rather than finishing the last one again.
		click(500, 400);
		expect(tools.save).toHaveBeenCalledTimes(1);
	});

	it("draws a measurement by dragging, stored before the tile's rotation", () => {
		const measure = setup({ tool: "measure", rotationQuarterTurns: 1 });
		measure.drag([500, 400], [600, 400]);
		expect(saved(measure.tools)).toEqual([
			expect.objectContaining({ kind: "measure", points: [[0, 0], [0, 1000]] }),
		]);
	});

	it("does not draw a box from a drag, nor from a second click on its first corner", () => {
		const { tools, drag, click } = setup({ tool: "box" });
		drag([500, 400], [700, 300]);
		expect(tools.save).not.toHaveBeenCalled();
		// The drag's press put the first corner down; its release did not finish the box.
		click(500, 400);
		expect(tools.save).not.toHaveBeenCalled();
		click(600, 300);
		expect(saved(tools)).toEqual([
			expect.objectContaining({ kind: "box", points: [[0, 0], [1000, 1000]] }),
		]);
	});

	it("places typed text where the operator clicked, at a size that reads at that zoom", () => {
		const { tools, click } = setup({ tool: "text" });
		click(500, 400);
		const field = screen.getByRole("textbox", { name: "Text to place" });
		fireEvent.change(field, { target: { value: "  FOH  " } });
		fireEvent.keyDown(field, { key: "Enter" });
		expect(saved(tools)).toEqual([
			expect.objectContaining({
				kind: "text",
				points: [[0, 0]],
				text: "FOH",
				textHeightMillimetres: 160,
			}),
		]);
		expect(screen.queryByRole("textbox", { name: "Text to place" })).toBeNull();
	});

	it("erases the item under the pointer and shows the words of the rest", () => {
		const { tools, click } = setup({
			tool: "erase",
			annotations: [
				{ id: "edge", view: "top_down", kind: "box", points: [[0, 0], [1000, 1000]], closed: false, text: "", textHeightMillimetres: 250 },
				{ id: "span", view: "top_down", kind: "measure", points: [[0, -2000], [3250, -2000]], closed: false, text: "", textHeightMillimetres: 250 },
				{ id: "note", view: "top_down", kind: "text", points: [[-3000, 0]], closed: false, text: "Stage left", textHeightMillimetres: 250 },
			],
		});
		expect(screen.getByText("3.25 m")).toBeInTheDocument();
		expect(screen.getByText("Stage left")).toBeInTheDocument();
		click(550, 400);
		expect(tools.remove).toHaveBeenCalledWith("edge");
	});

	it("drops a line in progress on Escape, and puts the tool down on a second Escape", () => {
		const { tools, canvas, click } = setup({ tool: "polyline" });
		click(500, 400);
		click(600, 400);
		fireEvent.keyDown(window, { key: "Escape" });
		fireEvent.doubleClick(canvas);
		expect(tools.save).not.toHaveBeenCalled();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(tools.setTool).toHaveBeenCalledWith("select");
	});

	it("shows the length of the segment being drawn before the line is finished", () => {
		const { tools, canvas, click } = setup({ tool: "polyline" });
		click(500, 400);
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 650, clientY: 400 });
		expect(screen.getByText("1.50 m")).toBeInTheDocument();
		click(650, 400);
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 650, clientY: 395 });
		expect(screen.getByText("50 mm")).toBeInTheDocument();
		expect(screen.queryByText("1.50 m")).toBeNull();
		expect(tools.save).not.toHaveBeenCalled();
	});

	it("finishes a line on a right-click without adding the point under the pointer", () => {
		const { tools, canvas, click } = setup({ tool: "polyline" });
		click(500, 400);
		click(600, 400);
		fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 700, clientY: 200 });
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 2, clientX: 700, clientY: 200 });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 2, clientX: 700, clientY: 200 });
		const menuShown = fireEvent.contextMenu(canvas, { clientX: 700, clientY: 200 });
		expect(menuShown).toBe(false);
		expect(saved(tools)).toEqual([
			expect.objectContaining({ kind: "polyline", closed: false, points: [[0, 0], [1000, 0]] }),
		]);
		expect(screen.queryByText("2.24 m")).toBeNull();
	});
});

const venue = (id: string, extra: Partial<CadEntity>): CadEntity => ({
	id,
	logicalFixtureId: id,
	name: id,
	fixtureNumber: null,
	fixtureDisplayId: "0.1",
	dmxAddress: "Visual only",
	kind: "venue",
	fixtureType: "venue",
	drawingId: id,
	layerId: "default",
	selectable: true,
	positionMillimetres: [0, 0, 0],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [1000, 1000, 1000],
	outputDirection: [0, 1, 0],
	...extra,
});

/** A 4 m truss at 5 m, its box reaching x ±2000 and y ±145, and a 2 × 1 m riser left of it. */
const RIG = [
	venue("truss", {
		positionMillimetres: [0, 0, 5000],
		sizeMillimetres: [4000, 290, 290],
		scenery: { kind: "truss", chords: 4, pattern: "standard" },
	}),
	venue("riser", {
		positionMillimetres: [-3000, 0, 0],
		sizeMillimetres: [2000, 1000, 400],
		scenery: { kind: "riser", chords: 0, pattern: "standard" },
	}),
];

describe("snapping while drawing", () => {
	it("snaps a line's points onto truss and stage-element corners", () => {
		const { tools, click } = setup({ tool: "polyline", entities: RIG, snapping: true });
		// 10 mm and 15 mm off the truss's back-right corner.
		click(701, 384);
		// 20 mm and 10 mm off the riser's back-right corner.
		click(302, 351);
		// On the truss's connector at its left end, reached past the grid.
		click(301, 401);
		fireEvent.keyDown(window, { key: "Enter" });
		expect(saved(tools)).toEqual([
			expect.objectContaining({
				points: [
					[2000, 145],
					[-2000, 500],
					[-2000, 0],
				],
			}),
		]);
	});

	it("keeps a line level or plumb with its last point, and every free coordinate on 10 cm", () => {
		const { tools, click } = setup({ tool: "polyline", entities: RIG, snapping: true });
		// (330, 330) lands on the grid.
		click(533, 367);
		// (2400, 360) is 60 mm off level with the last point, so it levels.
		click(740, 364);
		// (2430, 2500) is 30 mm off plumb over it, so it stands straight above.
		click(743, 150);
		// (1260, 2640) is near neither, so both coordinates land on the grid.
		click(626, 136);
		fireEvent.keyDown(window, { key: "Enter" });
		expect(saved(tools)).toEqual([
			expect.objectContaining({
				points: [
					[300, 300],
					[2400, 300],
					[2400, 2500],
					[1300, 2600],
				],
			}),
		]);
	});

	it("snaps both corners of a box", () => {
		const { tools, click } = setup({ tool: "box", entities: RIG, snapping: true });
		click(302, 351);
		click(533, 367);
		expect(saved(tools)).toEqual([
			expect.objectContaining({ kind: "box", points: [[-2000, 500], [300, 300]] }),
		]);
	});

	it("draws exactly where the pointer is while Shift is held", () => {
		const { tools, click } = setup({ tool: "polyline", entities: RIG, snapping: true });
		click(701, 384, true);
		click(740, 364, true);
		// Released, Shift gives snapping back: (330, 330) levels with the unsnapped point before it.
		click(533, 367);
		fireEvent.keyDown(window, { key: "Enter" });
		expect(saved(tools)).toEqual([
			expect.objectContaining({
				points: [
					[2010, 160],
					[2400, 360],
					[300, 360],
				],
			}),
		]);

		cleanup();
		const box = setup({ tool: "box", entities: RIG, snapping: true });
		box.click(302, 351, true);
		box.click(533, 367, true);
		expect(saved(box.tools)).toEqual([
			expect.objectContaining({ kind: "box", points: [[-1980, 490], [330, 330]] }),
		]);
	});

	it("draws exactly where the pointer is with snapping switched off", () => {
		const { tools, click } = setup({ tool: "polyline", entities: RIG });
		click(701, 384);
		click(533, 367);
		fireEvent.keyDown(window, { key: "Enter" });
		expect(saved(tools)).toEqual([
			expect.objectContaining({ points: [[2010, 160], [330, 330]] }),
		]);
	});
});

describe("Add Several on a CAD viewport", () => {
	it("places a copy of the held element at every press and stops on Escape or Done", () => {
		const { tools, click, onSelection } = setup({
			tool: "select",
			placing: { profileId: "railing", name: "Stage Railing 2 m" },
		});
		const banner = screen.getByRole("status", { name: "Adding several" });
		expect(banner).toHaveTextContent("Adding several Stage Railing 2 m");
		// A metre right and two up of the origin, then the origin itself: one copy each press.
		click(600, 200);
		click(500, 400);
		expect(tools.placeAt).toHaveBeenNthCalledWith(1, { x: 1, y: 2, z: 0 });
		expect(tools.placeAt).toHaveBeenNthCalledWith(2, { x: 0, y: 0, z: 0 });
		expect(onSelection).not.toHaveBeenCalled();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(tools.stopPlacing).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "Done" }));
		expect(tools.stopPlacing).toHaveBeenCalledTimes(2);
	});
});

describe("picking and moving placed text", () => {
	// At plan (-3000, 0): screen (200, 400), its words reaching right and up from there.
	const note: CadAnnotation = {
		id: "note",
		view: "top_down",
		kind: "text",
		points: [[-3000, 0]],
		closed: false,
		text: "Stage left",
		textHeightMillimetres: 250,
	};
	// A lamp standing right under the words, which picking the text must leave alone.
	const lamp = {
		id: "lamp",
		logicalFixtureId: "lamp",
		name: "Lamp",
		kind: "profile",
		fixtureType: "moving_head_profile",
		drawingId: "lamp",
		layerId: "default",
		selectable: true,
		positionMillimetres: [-2800, 50, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [400, 400, 400],
		outputDirection: [0, 1, 0],
	} as unknown as CadEntity;

	it("picks text alone and moves it by dragging, as one change", () => {
		const { tools, drag, onSelection } = setup({ tool: "select", annotations: [note], entities: [lamp] });
		drag([220, 395], [320, 395]);
		expect(tools.selectText).toHaveBeenCalledWith("note");
		// The rig's selection is cleared, never set to the lamp under the words.
		expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: [] });
		expect(onSelection).not.toHaveBeenCalledWith(expect.objectContaining({ ids: ["lamp"] }));
		expect(tools.setTextPreview).toHaveBeenCalledWith({ id: "note", points: [[-2000, 0]] });
		expect(tools.change).toHaveBeenCalledWith({ ...note, points: [[-2000, 0]] });
	});

	it("moves picked text along one axis from its gizmo's arrow", () => {
		const { tools, drag } = setup({ tool: "select", annotations: [note], selectedTextId: "note" });
		// The horizontal arrow runs right from the anchor; the drag's rise is ignored.
		drag([230, 400], [330, 350]);
		expect(tools.change).toHaveBeenCalledWith({ ...note, points: [[-2000, 0]] });
	});

	it("keeps the gizmo on the words while a move is on its way to the show, and moves on from there", () => {
		// Dragged a metre right to screen (300, 400); the show has not answered yet.
		const { tools, drag } = setup({
			tool: "select",
			annotations: [note],
			selectedTextId: "note",
			textPreview: { id: "note", points: [[-2000, 0]] },
		});
		// The old anchor's arrow at (230, 400) is not there any more.
		drag([230, 400], [230, 300]);
		expect(tools.change).not.toHaveBeenCalled();
		// The arrow stands on the moved anchor, and the next move starts where the words are.
		drag([330, 400], [430, 350]);
		expect(tools.change).toHaveBeenCalledWith({ ...note, points: [[-1000, 0]] });
	});

	it("puts picked text down on a press elsewhere, and a click on it moves nothing", () => {
		const { tools, click } = setup({ tool: "select", annotations: [note], selectedTextId: "note" });
		click(700, 600);
		expect(tools.selectText).toHaveBeenCalledWith(null);
		click(220, 395);
		expect(tools.change).not.toHaveBeenCalled();
		expect(tools.setTextPreview).toHaveBeenLastCalledWith(null);
	});

	it("leaves text to the drawing tools while one is in hand", () => {
		const { tools, click } = setup({ tool: "polyline", annotations: [note] });
		click(220, 395);
		expect(tools.selectText).not.toHaveBeenCalled();
	});
});
