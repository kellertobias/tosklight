import { useCallback, useRef, useState } from "react";
import type { Position3d, SpatialProjection } from "./contracts";
import { axisRotation, projectionKind } from "./projectionKinds";

/**
 * A simplified 3D view of the Stage showing the projection as the shape it actually is: a
 * plane for planar, a cylinder for cylindrical, a sphere for spherical.
 *
 * Drag to orbit. Without that, a rotation about the viewing axis is invisible and the numbers
 * cannot be checked against anything.
 *
 * The Stage box is a fixed reference cube rather than the real Stage extents. It exists to
 * make orientation legible, not to measure it.
 */

const HALF = 5;
const TOP = HALF * 1.2;
const SIZE = { width: 320, height: 260 };
const DEFAULT_VIEW = { yaw: -35, pitch: 22 };

type View = { yaw: number; pitch: number };

function radians(value: number) {
	return (value * Math.PI) / 180;
}

/** World is Z-up: `Top` looks down -Z and `Front` looks +Y. */
function project(point: Position3d, view: View) {
	const yaw = radians(view.yaw);
	const pitch = radians(view.pitch);
	const x = point.x * Math.cos(yaw) - point.y * Math.sin(yaw);
	const y = point.x * Math.sin(yaw) + point.y * Math.cos(yaw);
	const scale = 17;
	return {
		x: SIZE.width / 2 + x * scale,
		y:
			SIZE.height / 2 +
			(y * Math.sin(pitch) - point.z * Math.cos(pitch)) * scale,
	};
}

/** Euler degrees about X, then Y, then Z — the same order the engine applies. */
function rotate(vector: Position3d, degrees: Position3d): Position3d {
	const [sx, cx] = [Math.sin(radians(degrees.x)), Math.cos(radians(degrees.x))];
	const [sy, cy] = [Math.sin(radians(degrees.y)), Math.cos(radians(degrees.y))];
	const [sz, cz] = [Math.sin(radians(degrees.z)), Math.cos(radians(degrees.z))];
	const ax = {
		x: vector.x,
		y: vector.y * cx - vector.z * sx,
		z: vector.y * sx + vector.z * cx,
	};
	const ay = {
		x: ax.x * cy + ax.z * sy,
		y: ax.y,
		z: -ax.x * sy + ax.z * cy,
	};
	return { x: ay.x * cz - ay.y * sz, y: ay.x * sz + ay.y * cz, z: ay.z };
}

function normalize(vector: Position3d): Position3d {
	const length = Math.hypot(vector.x, vector.y, vector.z);
	return length <= 1e-12
		? { x: 0, y: 0, z: 1 }
		: { x: vector.x / length, y: vector.y / length, z: vector.z / length };
}

function cross(a: Position3d, b: Position3d): Position3d {
	return {
		x: a.y * b.z - a.z * b.y,
		y: a.z * b.x - a.x * b.z,
		z: a.x * b.y - a.y * b.x,
	};
}

function add(...points: Position3d[]): Position3d {
	return points.reduce((total, point) => ({
		x: total.x + point.x,
		y: total.y + point.y,
		z: total.z + point.z,
	}));
}

function scaled(vector: Position3d, by: number): Position3d {
	return { x: vector.x * by, y: vector.y * by, z: vector.z * by };
}

function polyline(points: Position3d[], view: View) {
	return points.map((point) => {
		const flat = project(point, view);
		return `${flat.x.toFixed(1)},${flat.y.toFixed(1)}`;
	});
}

/** A perpendicular pair spanning the plane at right angles to `axis`. */
function basis(axis: Position3d) {
	const reference =
		Math.abs(axis.z) > 0.9 ? { x: 1, y: 0, z: 0 } : { x: 0, y: 0, z: 1 };
	const right = normalize(cross(axis, reference));
	return [right, normalize(cross(axis, right))] as const;
}

function ring(
	centre: Position3d,
	axis: Position3d,
	radius: number,
	view: View,
	steps = 48,
) {
	const [u, v] = basis(axis);
	return polyline(
		Array.from({ length: steps + 1 }, (_, index) => {
			const angle = (index / steps) * Math.PI * 2;
			return add(
				centre,
				scaled(u, Math.cos(angle) * radius),
				scaled(v, Math.sin(angle) * radius),
			);
		}),
		view,
	).join(" ");
}

const CUBE_EDGES: ReadonlyArray<[Position3d, Position3d]> = (() => {
	const corners: Position3d[] = [];
	for (const x of [-HALF, HALF])
		for (const y of [-HALF, HALF]) for (const z of [0, TOP]) corners.push({ x, y, z });
	const edges: [Position3d, Position3d][] = [];
	for (const a of corners)
		for (const b of corners) {
			const shared =
				Number(a.x === b.x) + Number(a.y === b.y) + Number(a.z === b.z);
			if (shared === 2 && (a.x < b.x || a.y < b.y || a.z < b.z))
				edges.push([a, b]);
		}
	return edges;
})();

function ProjectionBody({
	projection,
	view,
}: {
	projection: SpatialProjection;
	view: View;
}) {
	const kind = projectionKind(projection);
	const centre = projection.anchor;
	const radius = HALF * 0.8;

	if (kind === "planar") {
		// The plane the projection ranks across, drawn perpendicular to the view direction.
		const normal = normalize(projection.view_direction);
		const [u, v] = basis(normal);
		const corners = [
			add(centre, scaled(u, radius), scaled(v, radius)),
			add(centre, scaled(u, -radius), scaled(v, radius)),
			add(centre, scaled(u, -radius), scaled(v, -radius)),
			add(centre, scaled(u, radius), scaled(v, -radius)),
		];
		return (
			<>
				<polygon
					className="projection-stage-surface"
					points={polyline(corners, view).join(" ")}
				/>
				<polyline
					className="projection-stage-start"
					points={polyline(
						[centre, add(centre, scaled(normal, radius))],
						view,
					).join(" ")}
					markerEnd="url(#projection-arrow)"
				/>
			</>
		);
	}

	if (kind === "cylindrical") {
		const axis = normalize(rotate({ x: 0, y: 0, z: 1 }, axisRotation(projection)));
		const [u, v] = basis(axis);
		const top = add(centre, scaled(axis, HALF));
		const bottom = add(centre, scaled(axis, -HALF));
		const seed = normalize(
			rotate({ x: 1, y: 0, z: 0 }, axisRotation(projection)),
		);
		const start = radians(projection.start_angle_degrees ?? 0);
		const side = cross(axis, seed);
		const startDirection = normalize(
			add(scaled(seed, Math.cos(start)), scaled(side, Math.sin(start))),
		);
		return (
			<>
				<polyline
					className="projection-stage-surface-line"
					points={ring(top, axis, radius, view)}
				/>
				<polyline
					className="projection-stage-surface-line"
					points={ring(bottom, axis, radius, view)}
				/>
				{[u, scaled(u, -1), v, scaled(v, -1)].map((direction, index) => (
					<polyline
						key={index}
						className="projection-stage-surface-line"
						points={polyline(
							[
								add(top, scaled(direction, radius)),
								add(bottom, scaled(direction, radius)),
							],
							view,
						).join(" ")}
					/>
				))}
				<polyline
					className="projection-stage-axis"
					points={polyline([bottom, top], view).join(" ")}
				/>
				<polyline
					className="projection-stage-start"
					points={polyline(
						[centre, add(centre, scaled(startDirection, radius))],
						view,
					).join(" ")}
					markerEnd="url(#projection-arrow)"
				/>
			</>
		);
	}

	// Spherical: three great circles read as a sphere, plus the ray to the spread's centre.
	const azimuth = radians(projection.start_angle_degrees ?? 0);
	const elevation = radians(projection.elevation_degrees ?? 0);
	const direction = {
		x: Math.cos(elevation) * Math.cos(azimuth),
		y: Math.cos(elevation) * Math.sin(azimuth),
		z: Math.sin(elevation),
	};
	return (
		<>
			{[
				{ x: 0, y: 0, z: 1 },
				{ x: 1, y: 0, z: 0 },
				{ x: 0, y: 1, z: 0 },
			].map((axis, index) => (
				<polyline
					key={index}
					className="projection-stage-surface-line"
					points={ring(centre, axis, radius, view)}
				/>
			))}
			<polyline
				className="projection-stage-start"
				points={polyline(
					[centre, add(centre, scaled(direction, radius))],
					view,
				).join(" ")}
				markerEnd="url(#projection-arrow)"
			/>
		</>
	);
}

export function ProjectionStagePreview({
	projection,
}: {
	projection: SpatialProjection;
}) {
	const [view, setView] = useState<View>(DEFAULT_VIEW);
	const drag = useRef<{ x: number; y: number } | null>(null);
	const kind = projectionKind(projection);

	const orbit = useCallback((dx: number, dy: number) => {
		setView((current) => ({
			yaw: current.yaw + dx,
			// Past vertical the scene turns inside out, so the pitch stops at the poles.
			pitch: Math.min(89, Math.max(-89, current.pitch + dy)),
		}));
	}, []);

	const centre = project(projection.anchor, view);
	return (
		<svg
			className="projection-stage-preview"
			viewBox={`0 0 ${SIZE.width} ${SIZE.height}`}
			role="img"
			aria-label={`${kind} projection in the Stage, drag to orbit`}
			tabIndex={0}
			onPointerDown={(event) => {
				drag.current = { x: event.clientX, y: event.clientY };
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (!drag.current) return;
				orbit(
					(event.clientX - drag.current.x) * 0.6,
					(event.clientY - drag.current.y) * 0.6,
				);
				drag.current = { x: event.clientX, y: event.clientY };
			}}
			onPointerUp={(event) => {
				drag.current = null;
				event.currentTarget.releasePointerCapture(event.pointerId);
			}}
			onKeyDown={(event) => {
				const step = event.shiftKey ? 15 : 5;
				if (event.key === "ArrowLeft") orbit(-step, 0);
				else if (event.key === "ArrowRight") orbit(step, 0);
				else if (event.key === "ArrowUp") orbit(0, -step);
				else if (event.key === "ArrowDown") orbit(0, step);
				else if (event.key === "Home") setView(DEFAULT_VIEW);
				else return;
				event.preventDefault();
			}}
		>
			<title>{`${kind} projection in the Stage, drag to orbit`}</title>
			<defs>
				<marker
					id="projection-arrow"
					viewBox="0 0 8 8"
					refX="7"
					refY="4"
					markerWidth="5"
					markerHeight="5"
					orient="auto-start-reverse"
				>
					<path d="M 0 1 L 7 4 L 0 7 z" className="projection-stage-arrowhead" />
				</marker>
			</defs>
			<g className="projection-stage-box">
				{CUBE_EDGES.map(([from, to]) => {
					const a = project(from, view);
					const b = project(to, view);
					return (
						<line
							key={`${JSON.stringify(from)}-${JSON.stringify(to)}`}
							x1={a.x}
							y1={a.y}
							x2={b.x}
							y2={b.y}
						/>
					);
				})}
			</g>
			<ProjectionBody projection={projection} view={view} />
			<circle
				className="projection-stage-centre"
				cx={centre.x}
				cy={centre.y}
				r={4}
			/>
		</svg>
	);
}
