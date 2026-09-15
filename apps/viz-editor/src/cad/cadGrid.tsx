/**
 * The grid over a CAD viewport: pale lines at the scale indicator's step, and optionally a
 * sub-grid of small plus signs at its quarter steps.
 *
 * It is drawn as its own layer over the linework and follows the tile's camera exactly as the
 * canvas does, so it moves and zooms with the plan. Spacing follows the scale indicator unless a
 * fixed spacing is chosen in Settings, and a grid that would be denser than a few pixels is left
 * out rather than filling the view.
 */
import { useEffect, useRef, useState } from "react";
import type { TileCamera } from "./types";

export interface CadGridSettings {
	show: boolean;
	/** The grid's colour, as `#RRGGBB`. */
	colour: string;
	/** Fixed spacing in millimetres; null follows the scale indicator's step. */
	spacingMillimetres: number | null;
	/** Plus signs at the quarter steps between the lines. */
	subGrid: boolean;
}

export const DEFAULT_GRID: CadGridSettings = {
	show: true,
	colour: "#c9d1d9",
	spacingMillimetres: null,
	subGrid: false,
};

/** Grid steps a viewport can offer in Settings besides following the scale. */
export const GRID_SPACINGS_MILLIMETRES = [100, 250, 500, 1000, 2000, 5000, 10000] as const;

/** The scale indicator divides its step into quarters, so the sub-grid does too. */
export const SUB_GRID_DIVISIONS = 4;

const MIN_PIXELS = 6;
const MAX_LINES = 600;
const MAX_CROSSES = 5000;

export interface GridGeometry {
	/** Screen-space lines, from edge to edge of the viewport. */
	lines: { x1: number; y1: number; x2: number; y2: number }[];
	/** Screen-space centres of the sub-grid's plus signs. */
	crosses: { x: number; y: number }[];
}

/** Where a grid of `stepMillimetres` falls on a viewport of this size and camera, in pixels. */
export function gridGeometry({
	width,
	height,
	camera,
	stepMillimetres,
	subdivisions,
}: {
	width: number;
	height: number;
	camera: TileCamera;
	stepMillimetres: number;
	subdivisions: number;
}): GridGeometry {
	const empty: GridGeometry = { lines: [], crosses: [] };
	const { zoom, pan } = camera;
	if (!(width > 0 && height > 0 && zoom > 0 && stepMillimetres > 0)) return empty;
	if (stepMillimetres * zoom < MIN_PIXELS) return empty;
	const screenX = (x: number) => width / 2 + (x + pan[0]) * zoom;
	const screenY = (y: number) => height / 2 - (y + pan[1]) * zoom;
	const minX = -width / 2 / zoom - pan[0];
	const maxX = width / 2 / zoom - pan[0];
	const minY = -height / 2 / zoom - pan[1];
	const maxY = height / 2 / zoom - pan[1];
	const range = (low: number, high: number, step: number) => {
		const first = Math.ceil(low / step);
		const last = Math.floor(high / step);
		return last - first + 1 > MAX_LINES
			? []
			: Array.from({ length: Math.max(0, last - first + 1) }, (_, index) => (first + index) * step);
	};
	const lines = [
		...range(minX, maxX, stepMillimetres).map((x) => ({
			x1: screenX(x),
			y1: 0,
			x2: screenX(x),
			y2: height,
		})),
		...range(minY, maxY, stepMillimetres).map((y) => ({
			x1: 0,
			y1: screenY(y),
			x2: width,
			y2: screenY(y),
		})),
	];
	const sub = stepMillimetres / subdivisions;
	if (subdivisions < 2 || sub * zoom < MIN_PIXELS) return { lines, crosses: [] };
	const columns = range(minX, maxX, sub);
	const rows = range(minY, maxY, sub);
	if (columns.length * rows.length > MAX_CROSSES) return { lines, crosses: [] };
	// A plus sign on a grid line would only thicken the line, so the sub-grid skips them.
	const onLine = (value: number) =>
		Math.abs(value / stepMillimetres - Math.round(value / stepMillimetres)) < 1e-6;
	const crosses = columns
		.filter((x) => !onLine(x))
		.flatMap((x) =>
			rows.filter((y) => !onLine(y)).map((y) => ({ x: screenX(x), y: screenY(y) })),
		);
	return { lines, crosses };
}

export function CadGrid({
	camera,
	settings,
	stepMillimetres,
}: {
	camera: TileCamera;
	settings: CadGridSettings;
	/** The spacing this tile draws, already resolved against the scale indicator. */
	stepMillimetres: number;
}) {
	const svg = useRef<SVGSVGElement>(null);
	const [size, setSize] = useState({ width: 0, height: 0 });
	useEffect(() => {
		const element = svg.current;
		if (!element) return;
		const measure = () =>
			setSize({ width: element.clientWidth, height: element.clientHeight });
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	const geometry = settings.show
		? gridGeometry({
				...size,
				camera,
				stepMillimetres,
				subdivisions: settings.subGrid ? SUB_GRID_DIVISIONS : 0,
			})
		: { lines: [], crosses: [] };
	return (
		<svg
			ref={svg}
			className="cad-grid"
			aria-hidden="true"
			data-grid-step={settings.show ? stepMillimetres : undefined}
		>
			<g stroke={settings.colour} strokeOpacity={0.22} strokeWidth={1} shapeRendering="crispEdges">
				{geometry.lines.map((line) => (
					<line key={`${line.x1},${line.y1},${line.x2},${line.y2}`} {...line} />
				))}
			</g>
			{geometry.crosses.length ? (
				<path
					stroke={settings.colour}
					strokeOpacity={0.4}
					strokeWidth={1}
					shapeRendering="crispEdges"
					d={geometry.crosses
						.map(({ x, y }) => `M${x - 3} ${y}h6M${x} ${y - 3}v6`)
						.join("")}
				/>
			) : null}
		</svg>
	);
}
