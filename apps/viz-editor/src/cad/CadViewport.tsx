import { type WheelEvent, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { flushSync } from "react-dom";
import { annotationsForView } from "./annotationGeometry";
import { CadGrid, type CadGridSettings, DEFAULT_GRID } from "./cadGrid";
import { CadAnnotationLayer } from "./CadAnnotationLayer";
import type { CadObjectMenuRequest } from "./CadObjectMenu";
import { clampZoom } from "./cadShortcuts";
import { useCadTools } from "./cadTools";
import { PrintFrame } from "./CadPrintFrame";
import {
	CadEntityLabels,
	CadMoveReadout,
	CadPlacingBanner,
	CadScaleBar,
} from "./CadViewportOverlays";
import {
	type CadFrame,
	LineRenderer,
	observeViewportResize,
} from "./lineRenderer";
import {
	fitCadOverview,
	OVERVIEW_ROTATION_QUARTER_TURNS,
	OVERVIEW_VIEW,
} from "./planGeometry";
import type { CadPrintDocumentInfo } from "./print";
import type {
	CadDrawing,
	CadEntity,
	CadPrintPage,
	CadTransformPreview,
	CadViewDirection,
	SelectionChange,
	TileCamera,
} from "./types";
import type { CadUnderlay } from "./underlays";
import { useCadDrawingTool } from "./useCadDrawingTool";
import { useCadPlacementTool } from "./useCadPlacementTool";
import { groupSelectedIds } from "./venueGroups";
import {
	type CadViewportContext,
	useCadViewportInteraction,
} from "./useCadViewportInteraction";
import { useAnchoredCamera } from "./useTopLeftAnchor";

/** Stable empty defaults, so a frame without them does not count as a new picture every render. */
const NO_UNDERLAYS: readonly CadUnderlay[] = [];
const NO_PRINT_PAGES: readonly CadPrintPage[] = [];

interface CadViewportProps {
	entities: readonly CadEntity[];
	drawings: readonly CadDrawing[];
	selectedIds: readonly string[];
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	preview: CadTransformPreview | null;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
	showCoordinateOrigins?: boolean;
	editEnabled?: boolean;
	/** Whether moves and measurements snap onto a fit; Shift turns it off while held. */
	snapping?: boolean;
	printMode?: boolean;
	/** Venue drawings placed on this view, drawn under the rig. */
	underlays?: readonly CadUnderlay[];
	/** The grid over the plan, as Settings chose it. */
	grid?: CadGridSettings;
	printPages?: readonly CadPrintPage[];
	selectedPrintPageId?: string | null;
	onSelectPrintPage?(id: string): void;
	onChangePrintPage?(id: string, change: Partial<CadPrintPage>): void;
	documentInfo?: CadPrintDocumentInfo;
	onCamera(camera: TileCamera): void;
	onSelection(change: SelectionChange): void;
	/**
	 * Widens a plain click's or marquee's pick to whole Venue element groups; Shift skips it so one
	 * element is picked alone. Absent picks exactly what was hit.
	 */
	expandSelection?(ids: readonly string[]): string[];
	/** Which placement a click picked, so a multi-patch copy can be edited on its own. */
	onFocusEntity?(entityId: string | null): void;
	onPreview(preview: CadTransformPreview | null): void;
	/** Opens the Duplicate / Delete menu for the selection a right-click picked. */
	onObjectMenu?(request: CadObjectMenuRequest): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
		/** False while Shift is held, so nothing snapped. */
		snap: boolean,
	): Promise<void>;
}

interface CadRigOverviewProps {
	entities: readonly CadEntity[];
	drawings: readonly CadDrawing[];
	showName: string;
}

/** A fixed, read-only Show-screen plan rendered by the canonical CAD drawing pipeline. */
export function CadRigOverview({
	entities,
	drawings,
	showName,
}: CadRigOverviewProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const redraw = useRef<() => void>(() => undefined);
	const drawingById = useMemo(
		() => new Map(drawings.map((drawing) => [drawing.id, drawing])),
		[drawings],
	);

	useEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		return observeViewportResize(canvas.current, () => redraw.current());
	}, []);

	useEffect(() => {
		redraw.current = () => {
			const target = canvas.current;
			if (!target) return;
			const camera = fitCadOverview(
				entities,
				drawingById,
				target.clientWidth,
				target.clientHeight,
			);
			renderer.current?.draw({
				entities,
				drawings: drawingById,
				selected: new Set(),
				view: OVERVIEW_VIEW,
				rotationQuarterTurns: OVERVIEW_ROTATION_QUARTER_TURNS,
				camera,
				preview: null,
				editEnabled: false,
				guide: null,
				selectionBox: null,
			});
		};
		redraw.current();
	}, [entities, drawingById]);

	return (
		<canvas
			ref={canvas}
			className="cad-canvas viz-show-rig-canvas"
			role="img"
			aria-label={`Read-only rig overview for ${showName}`}
			data-view={OVERVIEW_VIEW}
			data-rotation-quarter-turns={OVERVIEW_ROTATION_QUARTER_TURNS}
			data-entity-count={entities.length}
		/>
	);
}


/**
 * Draws a frame now and whenever the canvas changes size, through one renderer per canvas.
 *
 * The frame is drawn in a layout effect, before the browser paints, so the canvas lands in the same
 * frame as the grid, labels and scale bar React has just written; drawn after paint, the linework
 * trailed the grid by a frame while panning.
 */
function useViewportRedraw(
	canvas: React.RefObject<HTMLCanvasElement | null>,
	frame: CadFrame,
) {
	const renderer = useRef<LineRenderer | null>(null);
	const redraw = useRef<() => void>(() => undefined);
	useLayoutEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		return observeViewportResize(canvas.current, () => redraw.current());
	}, [canvas]);
	useLayoutEffect(() => {
		redraw.current = () => renderer.current?.draw(frame);
		redraw.current();
		// A frame is a fixed set of fields; any one of them changing is a new picture.
	}, Object.values(frame));
}

/** Zooms from a wheel gesture, mounted on the viewport rather than the canvas: print page frames
 * are siblings of the canvas, so a wheel over one never reached it and print mode did nothing. */
function zoomFromWheel(latest: () => TileCamera, settle: (c: TileCamera) => void) {
	return (event: WheelEvent<HTMLDivElement>) => {
		event.preventDefault();
		const camera = latest();
		const zoom = camera.zoom * Math.exp(-event.deltaY * 0.0015);
		// Rendered before the event returns, so the zoom shows in the very next frame.
		flushSync(() => settle({ ...camera, zoom: clampZoom(zoom) }));
	};
}

/** The drawing tool and the pointer gestures of one viewport, and the snapped fits both mark. */
function useViewportGestures(context: CadViewportContext) {
	const tools = useCadTools();
	const { canvas, view, rotationQuarterTurns, camera, entities, snapping } = context;
	const drawing = useCadDrawingTool({
		canvas,
		view,
		rotationQuarterTurns,
		camera,
		enabled: context.editEnabled,
		entities,
		snapping,
	});
	const annotations = useMemo(() => {
		const onView = annotationsForView(tools.annotations, view);
		return drawing.draft ? [...onView, drawing.draft] : onView;
	}, [tools.annotations, view, drawing.draft]);
	const interaction = useCadViewportInteraction(context);
	const placement = useCadPlacementTool({
		canvas,
		view,
		rotationQuarterTurns,
		camera,
		enabled: context.editEnabled,
	});
	const moveMarkers = interaction.snapMarkers;
	const snapMarkers = useMemo(
		() => (drawing.snapMarker ? [...moveMarkers, drawing.snapMarker] : moveMarkers),
		[moveMarkers, drawing.snapMarker],
	);
	// The drawing tool is asked first; what it leaves goes to selection, moves and panning.
	const canvasHandlers = {
		onPointerDown: (event: React.PointerEvent<HTMLCanvasElement>) =>
			placement.pointerDown(event) ||
			drawing.pointerDown(event) ||
			interaction.pointerDown(event),
		onPointerMove: (event: React.PointerEvent<HTMLCanvasElement>) =>
			// A pointer move is not urgent to React and would render after the next frame;
			// rendering it inside the event puts the pan or drag on screen in that frame.
			flushSync(() => {
				drawing.pointerMove(event);
				interaction.pointerMove(event);
			}),
		onPointerUp: (event: React.PointerEvent<HTMLCanvasElement>) => {
			if (!drawing.pointerUp(event)) void interaction.pointerUp(event);
		},
		onPointerCancel: interaction.cancel,
		onDoubleClick: drawing.doubleClick,
		// With a drawing tool in hand a right-click finishes the line; with Select it opens the menu.
		onContextMenu: (event: React.MouseEvent<HTMLCanvasElement>) =>
			drawing.active ? drawing.contextMenu(event) : interaction.contextMenu(event),
	};
	return { drawing, annotations, interaction, placement, snapMarkers, canvasHandlers };
}

export function CadViewport({
	entities,
	drawings,
	selectedIds,
	view,
	rotationQuarterTurns,
	camera: committedCamera,
	preview,
	showFixtureIds,
	showDmxAddresses,
	showCoordinateOrigins = false,
	editEnabled = true,
	snapping = false,
	printMode = false,
	underlays = NO_UNDERLAYS,
	grid = DEFAULT_GRID,
	printPages = NO_PRINT_PAGES,
	selectedPrintPageId = null,
	onSelectPrintPage,
	onChangePrintPage,
	documentInfo,
	onCamera,
	onSelection,
	expandSelection,
	onFocusEntity,
	onPreview,
	onObjectMenu,
	onMove,
}: CadViewportProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	// Everything drawn below — canvas, grid, labels, scale bar, print frames — reads this one camera,
	// which is the pan or zoom in flight, so they all move together.
	const liveCamera = useAnchoredCamera(canvas, committedCamera, onCamera);
	const { camera } = liveCamera;
	const selected = useMemo(() => new Set(selectedIds), [selectedIds]);
	const groupSelected = useMemo(
		() => groupSelectedIds(selectedIds, expandSelection),
		[selectedIds, expandSelection],
	);
	const drawingById = useMemo(
		() => new Map(drawings.map((drawing) => [drawing.id, drawing])),
		[drawings],
	);
	const { drawing, annotations, interaction, placement, snapMarkers, canvasHandlers } =
		useViewportGestures({
		canvas,
		entities,
		drawingById,
		selected,
		selectedIds,
		view,
		rotationQuarterTurns,
		camera,
		editEnabled,
		snapping,
		onCamera: liveCamera.show,
		onCameraEnd: liveCamera.commit,
		onSelection,
		expandSelection,
		onFocusEntity,
		onPreview,
		onObjectMenu,
		onMove,
	});
	const { guide, selectionBox } = interaction;

	useViewportRedraw(canvas, {
		entities,
		drawings: drawingById,
		selected,
		groupSelected,
		view,
		rotationQuarterTurns,
		camera,
		preview,
		editEnabled,
		guide,
		selectionBox,
		showCoordinateOrigins,
		underlays,
		annotations,
		snapMarkers,
	});

	const scale = cadScaleForZoom(camera.zoom);

	return (
		<div
			className={`cad-viewport ${drawing.active ? "is-drawing" : ""}`.trim()}
			onWheel={zoomFromWheel(liveCamera.latest, liveCamera.settle)}
		>
			<canvas
				ref={canvas}
				className="cad-canvas"
				aria-label={`CAD ${view.replaceAll("_", " ")} viewport`}
				data-floor-datum={view === "top_down" ? "hidden" : "visible"}
				data-coordinate-origins={showCoordinateOrigins ? "visible" : "hidden"}
				{...canvasHandlers}
			/>
			<CadGrid
				camera={camera}
				settings={grid}
				stepMillimetres={grid.spacingMillimetres ?? scale.distanceMillimetres}
			/>
			<CadScaleBar scale={scale} printMode={printMode} />
			<CadEntityLabels
				{...{
					entities,
					preview,
					view,
					rotationQuarterTurns,
					camera,
					showFixtureIds,
					showDmxAddresses,
				}}
			/>
			<CadMoveReadout readout={interaction.readout} camera={camera} />
			{placement.active && placement.placing ? (
				<CadPlacingBanner name={placement.placing.name} onDone={placement.stop} />
			) : null}
			<CadAnnotationLayer
				annotations={annotations}
				rotationQuarterTurns={rotationQuarterTurns}
				camera={camera}
				pendingText={drawing.pendingText}
				onCommitText={drawing.commitText}
				onCancelText={drawing.cancelText}
			/>
			{printMode && printPages.length ? (
				<div className="cad-print-frames">
					{printPages.map((page) => (
						<PrintFrame
							key={page.id}
							underlays={underlays}
							page={page}
							entities={entities}
							camera={camera}
							selected={page.id === selectedPrintPageId}
							onSelect={() => onSelectPrintPage?.(page.id)}
							onChange={(change) => onChangePrintPage?.(page.id, change)}
							onCamera={(next) => flushSync(() => liveCamera.settle(next))}
							documentInfo={documentInfo}
						/>
					))}
				</div>
			) : null}
		</div>
	);
}

export interface CadViewportScale {
	distanceMillimetres: number;
	pixelWidth: number;
	label: string;
}

/** Select the nearest useful real-world length from the repeating metric 1-2-5 sequence. */
export function cadScaleForZoom(
	zoom: number,
	targetPixelWidth = 120,
): CadViewportScale {
	const safeZoom = Number.isFinite(zoom) && zoom > 0 ? zoom : 0.1;
	const desiredMillimetres = targetPixelWidth / safeZoom;
	const exponent = Math.floor(Math.log10(desiredMillimetres));
	const candidates = [exponent - 1, exponent, exponent + 1].flatMap((power) =>
		[1, 2, 5].map((step) => step * 10 ** power),
	);
	const distanceMillimetres = candidates.reduce((best, candidate) =>
		Math.abs(Math.log(candidate / desiredMillimetres)) <
		Math.abs(Math.log(best / desiredMillimetres))
			? candidate
			: best,
	);
	return {
		distanceMillimetres,
		pixelWidth: distanceMillimetres * safeZoom,
		label: formatCadScale(distanceMillimetres),
	};
}

export function formatCadScale(distanceMillimetres: number) {
	if (distanceMillimetres >= 1000)
		return `${formatMetricNumber(distanceMillimetres / 1000)} m`;
	return `${formatMetricNumber(distanceMillimetres / 10)} cm`;
}

function formatMetricNumber(value: number) {
	return Number.isInteger(value)
		? String(value)
		: value.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}


