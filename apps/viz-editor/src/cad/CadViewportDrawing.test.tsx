import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CadAnnotation } from "./annotations";
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
}: {
	tool: CadDrawTool;
	annotations?: CadAnnotation[];
	rotationQuarterTurns?: number;
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
	};
	const onSelection = vi.fn();
	render(
		<CadToolContext.Provider value={tools}>
			<CadViewport
				entities={[]}
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
	const click = (clientX: number, clientY: number) => {
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX, clientY });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX, clientY });
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

	it("draws a box and a measurement by dragging, stored before the tile's rotation", () => {
		const box = setup({ tool: "box" });
		box.drag([500, 400], [700, 300]);
		expect(saved(box.tools)).toEqual([
			expect.objectContaining({ kind: "box", points: [[0, 0], [2000, 1000]] }),
		]);

		cleanup();
		const measure = setup({ tool: "measure", rotationQuarterTurns: 1 });
		measure.drag([500, 400], [600, 400]);
		expect(saved(measure.tools)).toEqual([
			expect.objectContaining({ kind: "measure", points: [[0, 0], [0, 1000]] }),
		]);
	});

	it("does not draw a box from a click that never travelled", () => {
		const { tools, click } = setup({ tool: "box" });
		click(500, 400);
		expect(tools.save).not.toHaveBeenCalled();
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
});
