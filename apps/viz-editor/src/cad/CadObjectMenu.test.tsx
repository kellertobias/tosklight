import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CadObjectMenu, isMenuKey } from "./CadObjectMenu";
import { type CadDrawTool, CadToolContext, type CadTools } from "./cadTools";
import { CadViewport } from "./CadViewport";
import type { CadEntity } from "./types";

const lamp: CadEntity = {
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

/** A 1000 × 800 canvas at zoom 0.1: the lamp stands at the screen centre (500, 400). */
function setup({
	tool = "select",
	selectedIds = [],
	editEnabled = true,
}: {
	tool?: CadDrawTool;
	selectedIds?: string[];
	editEnabled?: boolean;
} = {}) {
	const tools: CadTools = {
		onAdd: vi.fn(),
		tool,
		setTool: vi.fn(),
		annotations: [],
		save: vi.fn().mockResolvedValue(undefined),
		remove: vi.fn().mockResolvedValue(undefined),
		error: null,
		clearError: vi.fn(),
		placed: null,
		announcePlaced: vi.fn(),
	};
	const onSelection = vi.fn();
	const onObjectMenu = vi.fn();
	const onFocusEntity = vi.fn();
	render(
		<CadToolContext.Provider value={tools}>
			<CadViewport
				entities={[lamp]}
				drawings={[]}
				selectedIds={selectedIds}
				view="top_down"
				rotationQuarterTurns={0}
				camera={{ pan: [0, 0], zoom: 0.1 }}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				editEnabled={editEnabled}
				onCamera={vi.fn()}
				onSelection={onSelection}
				onFocusEntity={onFocusEntity}
				onPreview={vi.fn()}
				onObjectMenu={onObjectMenu}
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
	const rightClick = (clientX: number, clientY: number, init: Partial<MouseEventInit> = {}) => {
		fireEvent.pointerDown(canvas, { pointerId: 1, button: 2, clientX, clientY, ...init });
		const shown = fireEvent.contextMenu(canvas, { button: 2, clientX, clientY, ...init });
		fireEvent.pointerUp(canvas, { pointerId: 1, button: 2, clientX, clientY, ...init });
		return shown;
	};
	return { tools, canvas, rightClick, onSelection, onObjectMenu, onFocusEntity };
}

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

afterEach(cleanup);

describe("right-clicking an element in a CAD viewport", () => {
	it("selects an unselected element and opens the menu there, stepping a copy by the element's own width", () => {
		const { rightClick, onSelection, onObjectMenu, onFocusEntity } = setup();
		expect(rightClick(505, 402)).toBe(false);
		expect(onFocusEntity).toHaveBeenCalledWith(lamp.id);
		expect(onSelection).toHaveBeenCalledTimes(1);
		expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: [lamp.id] });
		expect(onObjectMenu).toHaveBeenCalledWith({
			x: 505,
			y: 402,
			// The lamp is 400 mm across, so its copy stands exactly beside it rather than a fixed
			// 500 mm away, which would overlap a wider element and barely move a longer one.
			duplicateOffset: [400, 0, 0],
			// The menu carries what it acts on, so it paints without waiting for the selection.
			entityIds: [lamp.id],
		});
	});

	it("keeps a selection the element already belongs to", () => {
		const { rightClick, onSelection, onObjectMenu } = setup({ selectedIds: [lamp.id, "other"] });
		rightClick(500, 400);
		expect(onSelection).not.toHaveBeenCalled();
		expect(onObjectMenu).toHaveBeenCalledTimes(1);
	});

	it("opens nothing and clears nothing over empty plan", () => {
		const { rightClick, onSelection, onObjectMenu } = setup({ selectedIds: [lamp.id] });
		expect(rightClick(900, 100)).toBe(false);
		expect(onSelection).not.toHaveBeenCalled();
		expect(onObjectMenu).not.toHaveBeenCalled();
	});

	it("leaves a Control-click to selection and opens no menu", () => {
		const { canvas, onObjectMenu } = setup();
		fireEvent.contextMenu(canvas, { button: 0, ctrlKey: true, clientX: 500, clientY: 400 });
		expect(onObjectMenu).not.toHaveBeenCalled();
	});

	it("still finishes a line instead while a drawing tool is in hand", () => {
		const { rightClick, onObjectMenu, onSelection } = setup({ tool: "polyline" });
		rightClick(500, 400);
		expect(onObjectMenu).not.toHaveBeenCalled();
		expect(onSelection).not.toHaveBeenCalled();
	});

	it("offers nothing while the drawing cannot be edited", () => {
		const { rightClick, onObjectMenu } = setup({ editEnabled: false });
		rightClick(500, 400);
		expect(onObjectMenu).not.toHaveBeenCalled();
	});
});

describe("the CAD object menu", () => {
	function renderMenu(count = 1, grouping: { group?: boolean; ungroup?: boolean } = {}) {
		const actions = {
			onDuplicate: vi.fn(),
			onDelete: vi.fn(),
			onClose: vi.fn(),
			onGroup: vi.fn(),
			onUngroup: vi.fn(),
		};
		render(
			<CadObjectMenu
				request={{ x: 10, y: 20, duplicateOffset: [500, 0, 0], entityIds: ["a"] }}
				count={count}
				{...actions}
				onGroup={grouping.group ? actions.onGroup : null}
				onUngroup={grouping.ungroup ? actions.onUngroup : null}
			/>,
		);
		return actions;
	}

	it("offers exactly Duplicate and Delete when there is nothing to group or ungroup", () => {
		renderMenu();
		const menu = screen.getByRole("menu", { name: "Actions for the selected element" });
		expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
			"Duplicate",
			"Delete",
		]);
		expect(menu).toHaveStyle({ left: "10px", top: "20px" });
		expect(screen.getByRole("menuitem", { name: "Duplicate" })).toHaveFocus();
	});

	it("runs an action from the pointer and closes", () => {
		const actions = renderMenu(3);
		expect(screen.getByRole("menu", { name: "Actions for the selected 3 elements" })).toBeVisible();
		fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
		expect(actions.onClose).toHaveBeenCalled();
		expect(actions.onDelete).toHaveBeenCalledTimes(1);
		expect(actions.onDuplicate).not.toHaveBeenCalled();
	});

	it("is driven from the keyboard and closes on Escape or a press outside", () => {
		const actions = renderMenu();
		const menu = screen.getByRole("menu");
		fireEvent.keyDown(menu, { key: "ArrowDown" });
		expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveFocus();
		fireEvent.keyDown(menu, { key: "ArrowDown" });
		expect(screen.getByRole("menuitem", { name: "Duplicate" })).toHaveFocus();
		fireEvent.keyDown(menu, { key: "ArrowUp" });
		expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveFocus();
		fireEvent.keyDown(menu, { key: "Escape" });
		expect(actions.onClose).toHaveBeenCalledTimes(1);
		fireEvent.pointerDown(document.body);
		expect(actions.onClose).toHaveBeenCalledTimes(2);
	});

	it("offers Group above Duplicate when the selection can be grouped", () => {
		const actions = renderMenu(2, { group: true });
		expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
			"Group",
			"Duplicate",
			"Delete",
		]);
		fireEvent.click(screen.getByRole("menuitem", { name: "Group" }));
		expect(actions.onClose).toHaveBeenCalled();
		expect(actions.onGroup).toHaveBeenCalledTimes(1);
	});

	it("offers Ungroup for a grouped selection and keeps both entries when both apply", () => {
		const actions = renderMenu(3, { ungroup: true });
		expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
			"Ungroup",
			"Duplicate",
			"Delete",
		]);
		fireEvent.click(screen.getByRole("menuitem", { name: "Ungroup" }));
		expect(actions.onUngroup).toHaveBeenCalledTimes(1);
		cleanup();
		renderMenu(3, { group: true, ungroup: true });
		expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
			"Group",
			"Ungroup",
			"Duplicate",
			"Delete",
		]);
	});

	it("opens from the Menu key or Shift+F10 only", () => {
		const plain = { shiftKey: false, ctrlKey: false, metaKey: false, altKey: false };
		expect(isMenuKey({ ...plain, key: "ContextMenu" })).toBe(true);
		expect(isMenuKey({ ...plain, key: "F10", shiftKey: true })).toBe(true);
		expect(isMenuKey({ ...plain, key: "F10" })).toBe(false);
		expect(isMenuKey({ ...plain, key: "ContextMenu", metaKey: true })).toBe(false);
	});
});
