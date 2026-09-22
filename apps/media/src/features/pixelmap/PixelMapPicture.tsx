// The output picture with the display regions and pixel zones drawn over it.
//
// The live frame sits underneath so an operator places a zone against what is actually on the
// wall. The open tab's shapes are the ones that answer a press; the other tab's are drawn faintly
// for reference, so a region can be placed around the zones it has to leave alone.
//
// The selected shape can be dragged to move it, or pulled by one of its corner handles to resize
// it, with a finger as well as a mouse. The arrow keys do the same from the keyboard.

import {
	type KeyboardEvent,
	type PointerEvent,
	type RefObject,
	useEffect,
	useRef,
	useState,
} from "react";
import { api } from "../../shared/api/client";
import type {
	OutputConfigurationView,
	PixelMapView,
} from "../../shared/api/generated/media-wire";
import type { PixelMapTab } from "./PixelMapPage";
import {
	type CanvasShape,
	dragShape,
	keyDrag,
	RESIZE_HANDLES,
	type ShapeHandle,
	sameArea,
} from "./pixelMapGeometry";

const PREVIEW_REFRESH_MS = 1_000;
/** How far a press may wander, in screen pixels, before it becomes a drag rather than a tap. */
const DRAG_THRESHOLD_PX = 4;

/** A percentage for CSS, from a canvas fraction. */
function percent(value: number): string {
	return `${Math.max(0, Math.min(1, value)) * 100}%`;
}

function box(shape: {
	start: { x: number; y: number };
	end: { x: number; y: number };
}) {
	return {
		left: percent(Math.min(shape.start.x, shape.end.x)),
		top: percent(Math.min(shape.start.y, shape.end.y)),
		width: percent(Math.abs(shape.end.x - shape.start.x)),
		height: percent(Math.abs(shape.end.y - shape.start.y)),
	};
}

/** The share of the canvas a shape covers, rounded so float noise from a move never reorders. */
function area(shape: Parameters<typeof box>[0]): number {
	return Math.round(
		Math.abs(shape.end.x - shape.start.x) *
			Math.abs(shape.end.y - shape.start.y) *
			1e6,
	);
}

/** Larger shapes first, so a smaller one drawn inside them can always be pressed. */
function largestFirst<T extends Parameters<typeof box>[0]>(shapes: T[]): T[] {
	return [...shapes].sort((left, right) => area(right) - area(left));
}

/**
 * The Program picture behind the map, refreshed from the running Media Server.
 *
 * `pictureSrc` replaces it with a fixed picture and stops the refresh, for a surface rendered
 * away from a server — an isolated story has none to ask, and reaching for one there produces a
 * canvas that never paints.
 */
function useLivePreview(
	output: OutputConfigurationView,
	pictureSrc?: string,
): string {
	const [revision, setRevision] = useState(0);
	const live = pictureSrc === undefined;
	useEffect(() => {
		if (!live) return;
		const timer = window.setInterval(
			() => setRevision((current) => current + 1),
			PREVIEW_REFRESH_MS,
		);
		return () => window.clearInterval(timer);
	}, [live]);
	if (pictureSrc !== undefined) return pictureSrc;
	return api.outputPreviewUrl(output.id, revision, {
		width: Math.max(output.width, 1),
		height: Math.max(output.height, 1),
	});
}

type Drag = {
	pointerId: number;
	handle: ShapeHandle;
	x: number;
	y: number;
	original: CanvasShape;
	moved: boolean;
};

/**
 * Pointer and key handlers that turn a press on a shape or one of its handles into edits.
 *
 * The drag is measured from where it started against the shape as it was then, so a slow drag
 * never accumulates rounding and a drag back to the start leaves the shape exactly as it was.
 */
function useShapeDrag<T extends CanvasShape>(
	canvas: RefObject<HTMLDivElement | null>,
	shape: T,
	onChange: (shape: T) => void,
	onSelect: () => void,
) {
	// The drag listens on the window, so it follows the pointer even when a re-render or the
	// selection moving to the tables takes the pointer capture away from the shape.
	const latest = useRef({ shape, onChange, onSelect });
	latest.current = { shape, onChange, onSelect };
	const stop = useRef<(() => void) | null>(null);
	useEffect(() => () => stop.current?.(), []);

	const begin = (handle: ShapeHandle) => (event: PointerEvent<HTMLElement>) => {
		if (event.button !== 0) return;
		event.stopPropagation();
		stop.current?.();
		event.currentTarget.setPointerCapture?.(event.pointerId);
		const drag: Drag = {
			pointerId: event.pointerId,
			handle,
			x: event.clientX,
			y: event.clientY,
			original: { start: shape.start, end: shape.end },
			// A corner handle only exists on the selected shape, so it drags at once.
			moved: handle !== "move",
		};
		const move = (moved: globalThis.PointerEvent) => {
			const rect = canvas.current?.getBoundingClientRect();
			if (moved.pointerId !== drag.pointerId || !rect) return;
			const pixelsX = moved.clientX - drag.x;
			const pixelsY = moved.clientY - drag.y;
			if (!drag.moved && Math.hypot(pixelsX, pixelsY) < DRAG_THRESHOLD_PX)
				return;
			const current = latest.current;
			if (!drag.moved) current.onSelect();
			drag.moved = true;
			moved.preventDefault();
			const next = dragShape(
				{ ...current.shape, ...drag.original },
				drag.handle,
				pixelsX / Math.max(rect.width, 1),
				pixelsY / Math.max(rect.height, 1),
			);
			if (!sameArea(next, current.shape)) current.onChange(next);
		};
		const end = (ended: globalThis.PointerEvent) => {
			if (ended.pointerId === drag.pointerId) stop.current?.();
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", end);
		window.addEventListener("pointercancel", end);
		stop.current = () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", end);
			window.removeEventListener("pointercancel", end);
			stop.current = null;
		};
	};
	const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		const step = keyDrag(event);
		if (!step) return;
		event.preventDefault();
		const next = dragShape(shape, step.handle, step.dx, step.dy);
		if (!sameArea(next, shape)) onChange(next);
	};
	const handlers = (handle: ShapeHandle) => ({
		onPointerDown: begin(handle),
	});
	return { handlers, onKeyDown };
}

type PictureShape = CanvasShape & {
	id: string;
	name: string;
	enabled: boolean;
};

function Shape<T extends PictureShape>({
	canvas,
	shape,
	kind,
	active,
	selected,
	title,
	onSelect,
	onChange,
}: {
	canvas: RefObject<HTMLDivElement | null>;
	shape: T;
	kind: "region" | "zone";
	active: boolean;
	selected: boolean;
	title?: string;
	onSelect: (id: string) => void;
	onChange: (shape: T) => void;
}) {
	const { handlers, onKeyDown } = useShapeDrag(canvas, shape, onChange, () =>
		onSelect(shape.id),
	);
	const noun = kind === "region" ? "display region" : "pixel zone";
	return (
		<button
			type="button"
			className={`media-pixel-${kind}`}
			aria-label={`${shape.name} ${noun}`}
			aria-pressed={selected}
			aria-keyshortcuts={
				active && selected
					? "ArrowLeft ArrowRight ArrowUp ArrowDown"
					: undefined
			}
			data-enabled={shape.enabled ? "true" : "false"}
			title={title}
			disabled={!active}
			tabIndex={active ? 0 : -1}
			onClick={() => onSelect(shape.id)}
			onKeyDown={selected ? onKeyDown : undefined}
			style={box(shape)}
			{...(active ? handlers("move") : {})}
		>
			{active && <span>{shape.name}</span>}
		</button>
	);
}

/** The corner handles of the selected shape, drawn above every shape so none can cover them. */
function ResizeHandles<T extends PictureShape>({
	canvas,
	shape,
	kind,
	onSelect,
	onChange,
}: {
	canvas: RefObject<HTMLDivElement | null>;
	shape: T;
	kind: "region" | "zone";
	onSelect: (id: string) => void;
	onChange: (shape: T) => void;
}) {
	const { handlers } = useShapeDrag(canvas, shape, onChange, () =>
		onSelect(shape.id),
	);
	const left = Math.min(shape.start.x, shape.end.x);
	const right = Math.max(shape.start.x, shape.end.x);
	const top = Math.min(shape.start.y, shape.end.y);
	const bottom = Math.max(shape.start.y, shape.end.y);
	return (
		<>
			{RESIZE_HANDLES.map(({ handle, label }) => {
				const x = handle.endsWith("left") ? left : right;
				const y = handle.startsWith("top") ? top : bottom;
				return (
					<span
						key={handle}
						className="media-pixel-handle"
						data-kind={kind}
						data-handle={handle}
						// The table cells and the arrow keys are the accessible way to resize.
						aria-hidden="true"
						title={`Drag to resize ${shape.name} from its ${label}`}
						style={{
							left: `clamp(0px, calc(${percent(x)} - var(--handle-size) / 2), calc(100% - var(--handle-size)))`,
							top: `clamp(0px, calc(${percent(y)} - var(--handle-size) / 2), calc(100% - var(--handle-size)))`,
						}}
						{...handlers(handle)}
					/>
				);
			})}
		</>
	);
}

export function PixelMapPicture({
	output,
	map,
	tab,
	selectedRegionId,
	selectedZoneId,
	onSelectRegion,
	onSelectZone,
	onChangeRegion,
	onChangeZone,
	pictureSrc,
}: {
	output: OutputConfigurationView;
	map: PixelMapView;
	tab: PixelMapTab;
	selectedRegionId: string | null;
	selectedZoneId: string | null;
	onSelectRegion: (id: string) => void;
	onSelectZone: (id: string) => void;
	onChangeRegion: (region: PixelMapView["regions"][number]) => void;
	onChangeZone: (zone: PixelMapView["zones"][number]) => void;
	pictureSrc?: string;
}) {
	const src = useLivePreview(output, pictureSrc);
	const canvas = useRef<HTMLDivElement>(null);
	const [pictureFailed, setPictureFailed] = useState(false);
	const { width, height } = output;
	const selectedRegion =
		tab === "regions"
			? map.regions.find((region) => region.id === selectedRegionId)
			: undefined;
	const selectedZone =
		tab === "zones"
			? map.zones.find((zone) => zone.id === selectedZoneId)
			: undefined;
	return (
		<figure className="media-pixel-picture">
			{/* biome-ignore lint/a11y/useSemanticElements: a fieldset cannot hold the picture's aspect ratio reliably. */}
			<div
				ref={canvas}
				className="media-pixel-canvas"
				data-tab={tab}
				style={{
					aspectRatio: `${Math.max(width, 1)} / ${Math.max(height, 1)}`,
				}}
				role="group"
				aria-label={`Output picture, ${width} by ${height}`}
			>
				{!pictureFailed && (
					<img
						className="media-pixel-canvas-frame"
						src={src}
						alt=""
						draggable={false}
						onError={() => setPictureFailed(true)}
						onLoad={() => setPictureFailed(false)}
					/>
				)}
				{largestFirst(map.regions).map((region) => (
					<Shape
						key={region.id}
						canvas={canvas}
						shape={region}
						kind="region"
						active={tab === "regions"}
						selected={region.id === selectedRegionId}
						onSelect={onSelectRegion}
						onChange={onChangeRegion}
					/>
				))}
				{largestFirst(map.zones).map((zone) => (
					<Shape
						key={zone.id}
						canvas={canvas}
						shape={zone}
						kind="zone"
						active={tab === "zones"}
						selected={zone.id === selectedZoneId}
						title={`${zone.name}: ${zone.columns} by ${zone.rows}`}
						onSelect={onSelectZone}
						onChange={onChangeZone}
					/>
				))}
				{selectedRegion && (
					<ResizeHandles
						key={selectedRegion.id}
						canvas={canvas}
						shape={selectedRegion}
						kind="region"
						onSelect={onSelectRegion}
						onChange={onChangeRegion}
					/>
				)}
				{selectedZone && (
					<ResizeHandles
						key={selectedZone.id}
						canvas={canvas}
						shape={selectedZone}
						kind="zone"
						onSelect={onSelectZone}
						onChange={onChangeZone}
					/>
				)}
			</div>
			<figcaption>
				{output.name} · {width} × {height}
				{pictureFailed ? " · Live picture unavailable" : " · Live picture"}
				{(selectedRegion ?? selectedZone) &&
					" · Drag the selected shape to move it, or a corner to resize it"}
			</figcaption>
		</figure>
	);
}
