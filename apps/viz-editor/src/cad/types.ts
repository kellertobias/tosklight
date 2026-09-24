import type { CutPlanes } from "./cutPlanes";
/** Which sides of a flight of stairs a handrail runs up, as seen climbing it. */
export type CadStairHandrails = "none" | "left" | "right" | "both";

export type CadViewDirection =
	| "top_down"
	| "left_to_right"
	| "right_to_left"
	| "front_to_back"
	| "back_to_front";

export interface CadEntity {
	id: string;
	logicalFixtureId: string;
	name: string;
	fixtureNumber: number | null;
	fixtureDisplayId: string;
	dmxAddress: string;
	fixtureProfile?: string;
	mode?: string;
	note?: string;
	kind: string;
	fixtureType: string;
	drawingId: string;
	layerId: string;
	selectable: boolean;
	positionMillimetres: [number, number, number];
	rotationDegrees: [number, number, number];
	sizeMillimetres: [number, number, number];
	/**
	 * Where the lamp points, as a unit vector in plan axes (x across, y deep, z up): its emitter's
	 * axis turned by the bracket angle and then the placement's rotation, as the Visualizer aims it.
	 */
	outputDirection: [number, number, number];
	/**
	 * Where the light leaves the lamp, in millimetres from its position in plan axes, after scale,
	 * bracket and rotation; absent when neither the fixture nor a shipped model says, and the
	 * direction indicator then starts at the position.
	 */
	emitterOffsetMillimetres?: [number, number, number];
	/** Degrees this placement's mounting bracket is set to, positive nose-down; absent reads as 0. */
	bracketAngle?: number;
	/** How a generated Venue object is built; absent for fixtures and modelled objects. */
	scenery?: CadScenery;
	/** The clip this fixture hangs by, at the size it is drawn; absent when it has no box. */
	mounting?: CadMounting;
	/** A 3D model imported into this show (manufacturer "Imported models"), not a shipped Venue object. */
	importedModel?: boolean;
}

export interface CadScenery {
	/** The generated kind: `truss`, `curtain`, `chain`, `riser` and so on. */
	kind: string;
	/** Chords in a truss section: 1 a pipe, 2 a ladder, 3 a triangle, 4 a box. */
	chords: number;
	/** A truss's bracing. */
	pattern: "standard" | "deco" | string;
	/** What a stage element stands on; only a riser carries it, and absent reads as a scissor lift. */
	feet?: CadRiserFeet;
	/**
	 * Which sides of a flight of stairs carry a handrail, as seen climbing it: the placement's
	 * choice, else what the profile was made with. Absent reads as none.
	 */
	handrails?: CadStairHandrails;
	/** How a chain is rigged; only a chain carries it, and absent reads as a hoist at the top. */
	chain?: CadChainMode;
	/** What a rigged chain's end away from its hoist is fixed with; absent reads as a steelflex. */
	anchor?: CadChainAnchor;
}

/** A deck raised on crossed scissor arms, or standing on one fixed leg under each corner. */
export type CadRiserFeet = "scissor" | "fixed";

/**
 * Where a fixture is held: its mounting clip, as the desk resolved it for this placement.
 *
 * Every measurement is in the entity's own millimetres from the middle of its box — `x` across,
 * `y` deep, `z` up — before the placement's rotation. The desk carries the profile's declared clip
 * over to the size the entity is drawn at, so nothing here has to be scaled again.
 */
export interface CadMounting {
	/** What the fixture hangs by; only a clamp is caught by a pipe. */
	hardware: CadMountingHardware | string;
	/** The middle of the clip. */
	centre: [number, number, number];
	/** Half the clip's reach across, deep and up. */
	halfExtent: [number, number, number];
	/** Where a pipe's axis lies once the fixture hangs from the clip. */
	pipe: [number, number, number];
}

/** A hook clamp over a pipe, a yoke bolted to a surface, or nothing to hang the fixture by. */
export type CadMountingHardware = "clamp" | "yoke" | "none";

/** A steelflex round a three- or four-point truss, a flange on a pipe, or a shackle to the steel. */
export type CadChainAnchor = "steelflex" | "flange" | "shackle";

/** A chain hanging free, from a hoist at its top, or pulled down by a hoist at its bottom. */
export type CadChainMode = "plain" | "motor_top" | "motor_bottom";

export interface CadTransformPreview {
	entityIds: readonly string[];
	deltaMillimetres: [number, number, number];
	spread: boolean;
	/** A turn in flight: where each turned fixture stands and how it is turned, drawn in its place. */
	placements?: ReadonlyArray<{
		id: string;
		positionMillimetres: [number, number, number];
		rotationDegrees: [number, number, number];
	}>;
}

export function previewDeltaForEntity(
	preview: CadTransformPreview | null,
	entityId: string,
): [number, number, number] {
	if (!preview) return [0, 0, 0];
	const index = preview.entityIds.indexOf(entityId);
	if (index < 0) return [0, 0, 0];
	const factor =
		preview.spread && preview.entityIds.length > 1
			? index / (preview.entityIds.length - 1)
			: 1;
	return preview.deltaMillimetres.map((value) => {
		const next = value * factor;
		return Object.is(next, -0) ? 0 : next;
	}) as [number, number, number];
}

export type CadProjectionView = "top" | "left" | "right" | "front" | "back";

export interface CadProjection {
	view: CadProjectionView;
	svg: string;
	viewBoxMillimetres: [number, number, number, number];
	originMillimetres: [number, number];
}

export interface CadDrawing {
	id: string;
	projections: CadProjection[];
	liveMeshes?: CadLiveMesh[];
	/** The shipped model's line drawings, for a profile that carries no model of its own. */
	modelDrawing?: CadModelDrawing;
}

export interface CadModelDrawing {
	/** The shipped model's id in `assets/models`. */
	model: string;
	/** From the drawing's millimetres to the fixture's physical size. */
	scale: number;
	views: CadModelDrawingView[];
}

export interface CadModelDrawingView {
	view: "top" | "front" | "side";
	svg: string;
	/** The same view without the clamp or other mounting hardware, when the model has one. */
	noClampSvg?: string;
}

export interface CadLiveMesh {
	pose: "top" | "elevation";
	triangles: CadLiveTriangle[];
}

export interface CadLiveTriangle {
	pointsMillimetres: [
		[number, number, number],
		[number, number, number],
		[number, number, number],
	];
	colour: [number, number, number];
}

export interface EntityTransform {
	id: string;
	positionMillimetres: [number, number, number];
	rotationDegrees: [number, number, number];
}

export interface RigAttachment {
	fixtureId: string;
	trussMemberId: string;
	mountingPointId: string;
	localTransform: EntityTransform;
}

export interface CadSceneSnapshot {
	showId: string;
	sceneRevision: number;
	selectionRevision: number;
	entities: CadEntity[];
	drawings: CadDrawing[];
	selectedIds: string[];
	attachments: RigAttachment[];
}

export interface CadSceneDelta {
	sceneRevision: number;
	upserted: CadEntity[];
	drawings: CadDrawing[];
	removedIds: string[];
	attachments: RigAttachment[];
}

export interface SelectionDelta {
	revision: number;
	selectedIds: string[];
}

export type SelectionChange =
	| { type: "replace"; ids: readonly string[] }
	| { type: "add"; ids: readonly string[] }
	| { type: "toggle"; ids: readonly string[] };

export interface CadTransformOutcome {
	sceneRevision: number;
	transforms: EntityTransform[];
	attachments: RigAttachment[];
}

export interface TileCamera {
	pan: [number, number];
	zoom: number;
}

export interface ViewportTile {
	type: "tile";
	id: string;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	/// The slice of depth this tile shows. Absent means the whole drawing, which is what every
	/// tile saved before cut planes existed means too.
	cutPlanes?: CutPlanes;
}

export interface CadPrintPage {
	kind?: "plan" | "fixture_list";
	id: string;
	tileId: string;
	name: string;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	centreMillimetres: [number, number];
	widthMillimetres: number;
	included: boolean;
	orientation: "landscape" | "portrait";
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
	/// Whether fixtures print with their clamps and other mounting hardware. Pages saved before
	/// the switch existed print them.
	showMountingHardware: boolean;
	/// The slice of depth this page prints. Absent means the whole drawing.
	cutPlanes?: CutPlanes;
	/// Placed drawings this page leaves off. Absent means it prints every drawing on its axis,
	/// which is what every page saved before drawings could be placed means too.
	hiddenUnderlayIds?: readonly string[];
}

export function printPaperSize(page: Pick<CadPrintPage, "orientation">) {
	return page.orientation === "portrait"
		? { width: 210, height: 297 }
		: { width: 297, height: 210 };
}

export function printPageHeight(
	page: Pick<CadPrintPage, "widthMillimetres" | "orientation">,
) {
	const paper = printPaperSize(page);
	return page.widthMillimetres * (paper.height / paper.width);
}

export interface SplitTile {
	type: "split";
	id: string;
	direction: "horizontal" | "vertical";
	ratio: number;
	first: TileNode;
	second: TileNode;
}

export type TileNode = ViewportTile | SplitTile;

export type TileEdge = "left" | "right" | "top" | "bottom";

export const CAD_VIEW_LABELS: Record<CadViewDirection, string> = {
	top_down: "Top down",
	left_to_right: "Left to right",
	right_to_left: "Right to left",
	front_to_back: "Front to back",
	back_to_front: "Back to front",
};

export type WorldAxis = "x" | "y" | "z";

export interface ViewAxes {
	horizontal: { axis: WorldAxis; sign: 1 | -1 };
	vertical: { axis: WorldAxis; sign: 1 | -1 };
}

export function viewAxes(
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): ViewAxes {
	const base = baseViewAxes(view);
	if (view !== "top_down") return base;
	switch (normaliseQuarterTurns(rotationQuarterTurns)) {
		case 0:
			return base;
		case 1:
			return {
				horizontal: base.vertical,
				vertical: negateAxis(base.horizontal),
			};
		case 2:
			return {
				horizontal: negateAxis(base.horizontal),
				vertical: negateAxis(base.vertical),
			};
		case 3:
			return {
				horizontal: negateAxis(base.vertical),
				vertical: base.horizontal,
			};
	}
}

function baseViewAxes(view: CadViewDirection): ViewAxes {
	switch (view) {
		// A plan reads like a map: +X to the right and +Y up.
		case "top_down":
			return {
				horizontal: { axis: "x", sign: 1 },
				vertical: { axis: "y", sign: 1 },
			};
		case "left_to_right":
			return {
				horizontal: { axis: "y", sign: -1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "right_to_left":
			return {
				horizontal: { axis: "y", sign: 1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "front_to_back":
			return {
				horizontal: { axis: "x", sign: 1 },
				vertical: { axis: "z", sign: 1 },
			};
		case "back_to_front":
			return {
				horizontal: { axis: "x", sign: -1 },
				vertical: { axis: "z", sign: 1 },
			};
	}
}

function negateAxis(value: ViewAxes["horizontal"]): ViewAxes["horizontal"] {
	return { ...value, sign: value.sign === 1 ? -1 : 1 };
}

export function applySelectionChange(
	selectedIds: readonly string[],
	change: SelectionChange,
): string[] {
	if (change.type === "replace") return [...new Set(change.ids)];
	const next = new Set(selectedIds);
	for (const id of change.ids) {
		if (change.type === "toggle" && next.has(id)) next.delete(id);
		else next.add(id);
	}
	return [...next];
}

export function projectPoint(
	point: readonly [number, number, number],
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): [number, number] {
	const projected = projectPointUnrotated(point, view);
	if (view !== "top_down") return projected;
	return rotatePlane(projected, rotationQuarterTurns);
}

/** How long a lamp's direction indicator is drawn, in millimetres. */
export const DIRECTION_INDICATOR_MILLIMETRES = 420;

/**
 * A lamp's direction indicator on the page, from `centre` (its projected position): it starts at
 * the lamp's emitter, where that is known, and runs along where the lamp points. The screen and
 * printed pages both draw it from here.
 */
export function directionIndicator(
	entity: Pick<CadEntity, "outputDirection" | "emitterOffsetMillimetres">,
	centre: readonly [number, number],
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): [[number, number], [number, number]] {
	const offset = entity.emitterOffsetMillimetres
		? projectPoint(entity.emitterOffsetMillimetres, view, rotationQuarterTurns)
		: [0, 0];
	const start: [number, number] = [centre[0] + offset[0], centre[1] + offset[1]];
	const direction = projectPoint(
		entity.outputDirection.map(
			(value) => value * DIRECTION_INDICATOR_MILLIMETRES,
		) as [number, number, number],
		view,
		rotationQuarterTurns,
	);
	return [start, [start[0] + direction[0], start[1] + direction[1]]];
}

function projectPointUnrotated(
	point: readonly [number, number, number],
	view: CadViewDirection,
): [number, number] {
	switch (view) {
		case "top_down":
			return [point[0], point[1]];
		// The side views are the Visualizer's: from house left (left to right) downstage is on the
		// right, from house right it is on the left. Desk y runs upstage.
		case "left_to_right":
			return [-point[1], point[2]];
		case "right_to_left":
			return [point[1], point[2]];
		case "front_to_back":
			return [point[0], point[2]];
		case "back_to_front":
			return [-point[0], point[2]];
	}
}

export function planeDelta(
	delta: readonly [number, number],
	view: CadViewDirection,
	rotationQuarterTurns = 0,
): [number, number, number] {
	const resolved =
		view === "top_down"
			? rotatePlane(delta, -normaliseQuarterTurns(rotationQuarterTurns))
			: delta;
	switch (view) {
		case "top_down":
			return [resolved[0], resolved[1], 0];
		case "left_to_right":
			return [0, negate(resolved[0]), resolved[1]];
		case "right_to_left":
			return [0, resolved[0], resolved[1]];
		case "front_to_back":
			return [resolved[0], 0, resolved[1]];
		case "back_to_front":
			return [negate(resolved[0]), 0, resolved[1]];
	}
}

function rotatePlane(
	point: readonly [number, number],
	rotationQuarterTurns: number,
): [number, number] {
	switch (normaliseQuarterTurns(rotationQuarterTurns)) {
		case 0:
			return [point[0], point[1]];
		case 1:
			return [point[1], -point[0]];
		case 2:
			return [-point[0], -point[1]];
		case 3:
			return [-point[1], point[0]];
	}
}

/**
 * A plan point stored before the top-down plan showed +Y up, moved into the plan the same tile or
 * page uses now. The old plan was this one mirrored about its horizontal axis before rotation.
 */
export function legacyTopDownPlanPoint(
	point: readonly [number, number],
	rotationQuarterTurns: number,
): [number, number] {
	return normaliseQuarterTurns(rotationQuarterTurns) % 2 === 0
		? [point[0], negate(point[1])]
		: [negate(point[0]), point[1]];
}

export function normaliseQuarterTurns(value: number): 0 | 1 | 2 | 3 {
	return (((Math.round(value) % 4) + 4) % 4) as 0 | 1 | 2 | 3;
}

function negate(value: number): number {
	return value === 0 ? 0 : -value;
}

export function newTile(view: CadViewDirection = "top_down"): ViewportTile {
	return {
		type: "tile",
		id: crypto.randomUUID(),
		view,
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 0.08 },
	};
}

export function mapTile(
	node: TileNode,
	id: string,
	change: (tile: ViewportTile) => TileNode,
): TileNode {
	if (node.type === "tile") return node.id === id ? change(node) : node;
	return {
		...node,
		first: mapTile(node.first, id, change),
		second: mapTile(node.second, id, change),
	};
}

export function splitTile(
	node: TileNode,
	id: string,
	direction: "horizontal" | "vertical",
): TileNode {
	return splitTileAtEdge(
		node,
		id,
		direction === "horizontal" ? "right" : "bottom",
	);
}

export function splitTileAtEdge(
	node: TileNode,
	id: string,
	edge: TileEdge,
): TileNode {
	return mapTile(node, id, (tile) => {
		const adjacent = newTile(tile.view);
		const adjacentFirst = edge === "left" || edge === "top";
		return {
			type: "split",
			id: crypto.randomUUID(),
			direction:
				edge === "left" || edge === "right" ? "horizontal" : "vertical",
			ratio: 0.5,
			first: adjacentFirst ? adjacent : tile,
			second: adjacentFirst ? tile : adjacent,
		};
	});
}

export function removeSplitSide(
	node: TileNode,
	id: string,
	remove: "first" | "second",
): TileNode {
	if (node.type === "tile") return node;
	if (node.id === id) return remove === "first" ? node.second : node.first;
	return {
		...node,
		first: removeSplitSide(node.first, id, remove),
		second: removeSplitSide(node.second, id, remove),
	};
}

export function setSplitRatio(
	node: TileNode,
	id: string,
	ratio: number,
): TileNode {
	if (node.type === "tile") return node;
	if (node.id === id)
		return { ...node, ratio: Math.max(0.15, Math.min(0.85, ratio)) };
	return {
		...node,
		first: setSplitRatio(node.first, id, ratio),
		second: setSplitRatio(node.second, id, ratio),
	};
}
