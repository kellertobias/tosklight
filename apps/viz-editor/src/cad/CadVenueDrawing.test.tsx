import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { CadViewport } from "./CadViewport";
import { type CadDrawTool, CadToolContext, type CadTools } from "./cadTools";
import type { CadEntity } from "./types";
import { pickWallOpening } from "./venueDrawingGeometry";

const persistence = vi.hoisted(() => ({
	wall: vi.fn(),
	opening: vi.fn(),
	object: vi.fn(),
}));
vi.mock("./venueDrawPersistence", () => ({
	placeWallSegment: persistence.wall,
	placeWallOpening: persistence.opening,
	placeDrawnObject: persistence.object,
}));
beforeEach(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
	vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
	vi.clearAllMocks();
	persistence.wall.mockResolvedValue(["new-wall"]);
	persistence.opening.mockResolvedValue("new-opening");
	persistence.object.mockResolvedValue(["new-object"]);
});
function wall(id = "wall"): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name: id,
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "—",
		kind: "venue",
		fixtureType: "venue",
		drawingId: id,
		layerId: "default",
		selectable: true,
		positionMillimetres: [0, 0, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [6000, 200, 4000],
		outputDirection: [0, 0, 0],
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: { kind: "wall", openings: [] },
		},
	};
}
function withOpening(entity = wall()): CadEntity {
	return {
		...entity,
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: {
				kind: "wall",
				openings: [
					{
						id: "existing-opening",
						offset_millimetres: 3500,
						sill_millimetres: 1000,
						width_millimetres: 1000,
						height_millimetres: 1000,
						opening: { kind: "window", shape: "rectangular" },
					},
				],
			},
		},
	};
}
function setup(tool: CadDrawTool, entities: CadEntity[] = [], snapping = true) {
	const tools: CadTools = {
		onAdd: vi.fn(),
		tool,
		setTool: vi.fn(),
		annotations: [],
		save: vi.fn(),
		remove: vi.fn(),
		change: vi.fn(),
		selectedTextId: null,
		selectText: vi.fn(),
		textPreview: null,
		setTextPreview: vi.fn(),
		error: null,
		clearError: vi.fn(),
		placed: null,
		announcePlaced: vi.fn(),
		placing: null,
		startPlacing: vi.fn(),
		stopPlacing: vi.fn(),
		placeAt: vi.fn(),
		selectOpening: vi.fn(),
		reportError: vi.fn(),
	};
	const onSelection = vi.fn();
	render(
		<CadToolContext.Provider value={tools}>
			<CadViewport
				entities={entities}
				drawings={[]}
				selectedIds={[]}
				view="top_down"
				rotationQuarterTurns={0}
				camera={{ pan: [0, 0], zoom: 0.1 }}
				preview={null}
				showFixtureIds={false}
				showDmxAddresses={false}
				snapping={snapping}
				onCamera={vi.fn()}
				onSelection={onSelection}
				onPreview={vi.fn()}
				onMove={vi.fn().mockResolvedValue(undefined)}
			/>
		</CadToolContext.Provider>,
	);
	const canvas = screen.getByLabelText("CAD top down viewport");
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
	Object.defineProperty(canvas, "setPointerCapture", { value: vi.fn() });
	Object.defineProperty(canvas, "releasePointerCapture", { value: vi.fn() });
	const hover = (x: number, y: number, shiftKey = false) =>
		fireEvent.pointerMove(canvas, {
			pointerId: 1,
			clientX: x,
			clientY: y,
			shiftKey,
		});
	const click = (x: number, y: number, shiftKey = false) => {
		hover(x, y, shiftKey);
		fireEvent.pointerDown(canvas, {
			pointerId: 1,
			button: 0,
			shiftKey,
			clientX: x,
			clientY: y,
		});
		fireEvent.pointerUp(canvas, {
			pointerId: 1,
			button: 0,
			shiftKey,
			clientX: x,
			clientY: y,
		});
	};
	return { canvas, tools, onSelection, click, hover };
}
it("draws connected wall segments, snapping the first endpoint and closing the polygon", async () => {
	const existing = {
		...wall("existing"),
		positionMillimetres: [-1000, 0, 0],
		sizeMillimetres: [2000, 200, 4000],
	} as CadEntity;
	const { click, tools } = setup("wall", [existing]);
	click(504, 400);
	click(700, 400);
	click(700, 200);
	click(500, 200);
	click(501, 400);
	await waitFor(() => expect(persistence.wall).toHaveBeenCalledTimes(4));
	expect(persistence.wall.mock.calls).toEqual([
		[
			[0, 0, 0],
			[2000, 0, 0],
		],
		[
			[2000, 0, 0],
			[2000, 2000, 0],
		],
		[
			[2000, 2000, 0],
			[0, 2000, 0],
		],
		[
			[0, 2000, 0],
			[0, 0, 0],
		],
	]);
	expect(tools.save).not.toHaveBeenCalled();
	expect(tools.announcePlaced).toHaveBeenCalledTimes(4);
	expect(tools.setTool).toHaveBeenCalledWith("select");
});
it("Escape finishes a chain without cancelling already queued wall segments", async () => {
	let resolveFirst: (ids: string[]) => void = () => {};
	persistence.wall.mockImplementationOnce(
		() =>
			new Promise<string[]>((resolve) => {
				resolveFirst = resolve;
			}),
	);
	const { click, tools } = setup("wall");
	click(500, 400);
	click(700, 400);
	click(700, 200);
	fireEvent.keyDown(window, { key: "Escape" });
	await waitFor(() => expect(persistence.wall).toHaveBeenCalledTimes(1));
	resolveFirst(["first"]);
	await waitFor(() => expect(persistence.wall).toHaveBeenCalledTimes(2));
	click(300, 400);
	expect(persistence.wall).toHaveBeenCalledTimes(2);
	expect(tools.remove).not.toHaveBeenCalled();
	expect(tools.setTool).toHaveBeenCalledWith("select");
});
it("shows a wall-snapped window preview before persisting the clicked opening", async () => {
	const { hover, click, tools } = setup("window", [wall()]);
	hover(600, 403);
	const preview = screen.getByLabelText("window placement preview");
	expect(preview).toHaveAttribute("data-snapped-wall", "wall");
	expect(preview).toHaveAttribute("data-valid", "true");
	expect(persistence.opening).not.toHaveBeenCalled();
	click(600, 403);
	await waitFor(() => expect(persistence.opening).toHaveBeenCalledOnce());
	expect(persistence.opening.mock.calls[0][0].opening).toMatchObject({
		offset_millimetres: 3500,
		width_millimetres: 1000,
		opening: { kind: "window" },
	});
	expect(tools.selectOpening).toHaveBeenCalledWith({
		fixtureId: "wall",
		openingId: "new-opening",
	});
});
it("rejects an overlapping opening visibly without writing the wall", () => {
	const { hover, click } = setup("window", [withOpening()]);
	hover(600, 400);
	expect(screen.getByLabelText("window placement preview")).toHaveAttribute(
		"data-valid",
		"false",
	);
	click(600, 400);
	expect(screen.getByRole("alert")).toHaveTextContent("overlaps");
	expect(persistence.opening).not.toHaveBeenCalled();
});
it("Select picks an existing opening through its aperture and focuses the owning wall", () => {
	const { click, tools, onSelection } = setup("select", [withOpening()]);
	click(600, 400);
	expect(tools.selectOpening).toHaveBeenCalledWith({
		fixtureId: "wall",
		openingId: "existing-opening",
	});
	expect(onSelection).toHaveBeenCalledWith({ type: "replace", ids: ["wall"] });
});
it("chooses the near opening in opposing elevations and never selects a far opening through a solid near wall", () => {
	const near = withOpening({
		...wall("near"),
		positionMillimetres: [0, -2000, 0],
	});
	const far = withOpening({
		...wall("far"),
		positionMillimetres: [0, 2000, 0],
	});
	expect(
		pickWallOpening([near, far], [1000, 1500], "front_to_back", 0, 50)?.wall.id,
	).toBe("near");
	expect(
		pickWallOpening([far, near], [-1000, 1500], "back_to_front", 0, 50)?.wall
			.id,
	).toBe("far");
	expect(
		pickWallOpening(
			[
				{ ...wall("solid"), positionMillimetres: [0, -2000, 0] },
				{ ...far, sizeMillimetres: [6000, 200, 10000] },
			],
			[1000, 1500],
			"front_to_back",
			0,
			50,
		),
	).toBeNull();
});
it("places a door at its wall base", async () => {
	const { click } = setup("door", [wall()]);
	click(400, 400);
	await waitFor(() => expect(persistence.opening).toHaveBeenCalledOnce());
	expect(persistence.opening.mock.calls[0][0].opening).toMatchObject({
		sill_millimetres: 0,
		height_millimetres: 2100,
		opening: { kind: "door", leaves: "single", swing: "inward", hinge: "left" },
	});
});

it("selects a far opening through an outlined foreground wall", () => {
	const near = {
		...wall("near"),
		positionMillimetres: [0, -2000, 0],
	} as CadEntity;
	const far = withOpening({
		...wall("far"),
		positionMillimetres: [0, 2000, 0],
	});
	expect(
		pickWallOpening([near, far], [1000, 1500], "front_to_back", 0, 50)?.wall.id,
	).toBe("far");
});

for (const kind of ["pillar", "stairs"] as const) {
	it(`previews and places ${kind} at the clicked floor position`, async () => {
		const { hover, click } = setup(kind);
		hover(600, 300);
		expect(
			screen.getByLabelText(`${kind} placement preview`),
		).toBeInTheDocument();
		expect(persistence.object).not.toHaveBeenCalled();
		click(600, 300);
		await waitFor(() =>
			expect(persistence.object).toHaveBeenCalledWith(kind, [1000, 1000, 0]),
		);
	});
}

it("starts a T junction on a translated rotated wall segment, with corners taking priority", async () => {
	const existing = {
		...wall(),
		positionMillimetres: [1000, 2000, 0],
		rotationDegrees: [0, 0, 90],
		sizeMillimetres: [4000, 200, 4000],
	} as CadEntity;
	const { click } = setup("wall", [existing]);
	click(605, 200);
	click(800, 190);
	await waitFor(() =>
		expect(persistence.wall).toHaveBeenCalledWith(
			[1000, 2000, 0],
			[3000, 2000, 0],
		),
	);
	fireEvent.keyDown(window, { key: "Escape" });
	click(596, 396);
	click(800, 396);
	await waitFor(() =>
		expect(persistence.wall).toHaveBeenLastCalledWith(
			[1000, 0, 0],
			[3000, 0, 0],
		),
	);
});
it("ends a T junction on a rotated wall while preserving the horizontal constraint", async () => {
	const existing = {
		...wall(),
		positionMillimetres: [3000, 1000, 0],
		rotationDegrees: [0, 0, 90],
		sizeMillimetres: [4000, 200, 4000],
	} as CadEntity;
	const { click } = setup("wall", [existing]);
	click(500, 400);
	click(805, 390);
	await waitFor(() =>
		expect(persistence.wall).toHaveBeenCalledWith([0, 0, 0], [3000, 0, 0]),
	);
});
it("quantizes new wall directions to 45 and 90 degrees even with general snapping off", async () => {
	const { click } = setup("wall", [], false);
	click(500, 400);
	click(600, 290);
	click(615, 95);
	await waitFor(() => expect(persistence.wall).toHaveBeenCalledTimes(2));
	expect(persistence.wall.mock.calls).toEqual([
		[
			[0, 0, 0],
			[1050, 1050, 0],
		],
		[
			[1050, 1050, 0],
			[1050, 3050, 0],
		],
	]);
});
it("Shift bypasses corners, segments, angle constraints and automatic loop closure", async () => {
	const existing = {
		...wall(),
		positionMillimetres: [0, 2000, 0],
	} as CadEntity;
	const { click, tools } = setup("wall", [existing]);
	click(504, 204, true);
	click(700, 200, true);
	click(505, 203, true);
	await waitFor(() => expect(persistence.wall).toHaveBeenCalledTimes(2));
	expect(persistence.wall.mock.calls).toEqual([
		[
			[40, 1960, 0],
			[2000, 2000, 0],
		],
		[
			[2000, 2000, 0],
			[50, 1970, 0],
		],
	]);
	expect(tools.setTool).not.toHaveBeenCalledWith("select");
});
it("does not jump to a distant wall intersection when constraining the direction", async () => {
	const existing = {
		...wall(),
		positionMillimetres: [3000, 2000, 0],
		rotationDegrees: [0, 0, 90],
		sizeMillimetres: [5000, 200, 4000],
	} as CadEntity;
	const { click } = setup("wall", [existing]);
	click(500, 400);
	click(805, 250);
	await waitFor(() =>
		expect(persistence.wall).toHaveBeenCalledWith([0, 0, 0], [2275, 2275, 0]),
	);
});
