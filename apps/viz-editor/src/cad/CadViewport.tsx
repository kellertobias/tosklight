import { type WheelEvent, useEffect, useMemo, useRef } from "react";
import { PrintFrame } from "./CadPrintFrame";
import { CadEntityLabels, CadScaleBar } from "./CadViewportOverlays";
import { LineRenderer, observeViewportResize } from "./lineRenderer";
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
import { useCadViewportInteraction } from "./useCadViewportInteraction";

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
	printMode?: boolean;
	/** Venue drawings placed on this view, drawn under the rig. */
	underlays?: readonly CadUnderlay[];
	printPages?: readonly CadPrintPage[];
	selectedPrintPageId?: string | null;
	onSelectPrintPage?(id: string): void;
	onChangePrintPage?(id: string, change: Partial<CadPrintPage>): void;
	documentInfo?: CadPrintDocumentInfo;
	onCamera(camera: TileCamera): void;
	onSelection(change: SelectionChange): void;
	onPreview(preview: CadTransformPreview | null): void;
	onMove(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
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


/** Zooms from a wheel gesture, mounted on the viewport rather than the canvas: print page frames
 * are siblings of the canvas, so a wheel over one never reached it and print mode did nothing. */
function zoomFromWheel(camera: TileCamera, onCamera: (c: TileCamera) => void) {
	return (event: WheelEvent<HTMLDivElement>) => {
		event.preventDefault();
		const zoom = camera.zoom * Math.exp(-event.deltaY * 0.0015);
		onCamera({ ...camera, zoom: Math.min(2.5, Math.max(0.004, zoom)) });
	};
}

export function CadViewport({
	entities,
	drawings,
	selectedIds,
	view,
	rotationQuarterTurns,
	camera,
	preview,
	showFixtureIds,
	showDmxAddresses,
	showCoordinateOrigins = false,
	editEnabled = true,
	printMode = false,
	underlays = [],
	printPages = [],
	selectedPrintPageId = null,
	onSelectPrintPage,
	onChangePrintPage,
	documentInfo,
	onCamera,
	onSelection,
	onPreview,
	onMove,
}: CadViewportProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const renderer = useRef<LineRenderer | null>(null);
	const redraw = useRef<() => void>(() => undefined);
	const selected = useMemo(() => new Set(selectedIds), [selectedIds]);
	const drawingById = useMemo(
		() => new Map(drawings.map((drawing) => [drawing.id, drawing])),
		[drawings],
	);

	const { guide, selectionBox, pointerDown, pointerMove, pointerUp, cancel } =
		useCadViewportInteraction({
			canvas,
			entities,
			drawingById,
			selected,
			selectedIds,
			view,
			rotationQuarterTurns,
			camera,
			editEnabled,
			onCamera,
			onSelection,
			onPreview,
			onMove,
		});

	useEffect(() => {
		if (!canvas.current) return;
		renderer.current ??= LineRenderer.create(canvas.current);
		return observeViewportResize(canvas.current, () => redraw.current());
	}, []);

	useEffect(() => {
		redraw.current = () =>
			renderer.current?.draw({
				entities,
				drawings: drawingById,
				selected,
				view,
				rotationQuarterTurns,
				camera,
				preview,
				editEnabled,
				guide,
				selectionBox,
				showCoordinateOrigins,
				underlays,
			});
		redraw.current();
	}, [
		entities,
		drawingById,
		selected,
		view,
		rotationQuarterTurns,
		camera,
		preview,
		editEnabled,
		guide,
		selectionBox,
		showCoordinateOrigins,
		underlays,
	]);

	const scale = cadScaleForZoom(camera.zoom);

	return (
		<div className="cad-viewport" onWheel={zoomFromWheel(camera, onCamera)}>
			<canvas
				ref={canvas}
				className="cad-canvas"
				aria-label={`CAD ${view.replaceAll("_", " ")} viewport`}
				data-floor-datum={view === "top_down" ? "hidden" : "visible"}
				data-coordinate-origins={showCoordinateOrigins ? "visible" : "hidden"}
				onPointerDown={pointerDown}
				onPointerMove={pointerMove}
				onPointerUp={pointerUp}
				onPointerCancel={cancel}
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
							onCamera={onCamera}
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


