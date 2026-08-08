import { useCallback, useMemo, useRef, useState } from "react";
import type { Position3d, SpatialProjection } from "./contracts";
import { projectionKind } from "./projectionKinds";
import type { StageFixtureDot } from "./stageFixtureDots";

/**
 * A simplified 3D view of the Stage showing the projection as the shape it actually is: a
 * plane for planar, a cylinder for cylindrical, a sphere for spherical.
 *
 * Drag to orbit. Without that, a rotation about the viewing axis is invisible and the numbers
 * cannot be checked against anything.
 *
 * The camera is a perspective one, not the parallel projection the ranking itself uses. A
 * parallel picture of a cube gives away nothing about where it is being looked at from, and
 * whether the operator is above or below the shape is exactly what this view is for. What the
 * projection ranks by is unaffected: this is the picture, not the maths.
 *
 * The Stage box is a reference cube, not the real Stage extents. It sizes itself to hold
 * everything drawn so nothing leaves the frame, and it exists to make orientation legible
 * rather than to measure it.
 */

/** Half the drawn cube. Everything is scaled into this so the camera constants stay fixed. */
const DRAWN_HALF = 5;
/** Below this the picture would zoom into an empty Stage, so it is the smallest world shown. */
const MIN_EXTENT = 5;
const TOP_RATIO = 1.2;
const SIZE = { width: 320, height: 260 };
const DEFAULT_VIEW = { yaw: -35, pitch: 22 };

/** The camera orbits the middle of the reference cube at a fixed distance. */
const CAMERA_DISTANCE = 26;
/** Chosen so the cube fills about as much of the frame as it did without perspective. */
const FOCAL_LENGTH = 442;
/** Keeps geometry that swings behind the camera from turning the picture inside out. */
const MIN_DEPTH = 1;

type View = { yaw: number; pitch: number };
/** A view plus the world half-extent it is drawn at, which is all projection needs. */
type Frame = View & { extent: number };

function radians(value: number) {
	return (value * Math.PI) / 180;
}

/**
 * How much world the picture has to hold: everything drawn, so a rig larger than the default
 * cube shrinks to fit instead of running off the frame.
 */
function worldExtent(
	projection: SpatialProjection,
	fixtures: readonly StageFixtureDot[],
) {
	let extent = MIN_EXTENT;
	const consider = (point: Position3d) => {
		extent = Math.max(
			extent,
			Math.abs(point.x),
			Math.abs(point.y),
			Math.abs(point.z) / TOP_RATIO,
		);
	};
	consider(projection.anchor);
	for (const fixture of fixtures) consider(fixture.position);
	return extent;
}

/**
 * World is Z-up: `Top` looks down -Z and `Front` looks +Y.
 *
 * Yaw turns the camera about world Z, pitch lifts it above the horizon, and the perspective
 * divide is what makes the near face of the cube read as the near one. World units are scaled
 * into the drawn cube first, so the camera sees the same size box whatever the Stage measures.
 */
function project(point: Position3d, frame: Frame) {
	const yaw = radians(frame.yaw);
	const pitch = radians(frame.pitch);
	const [sinYaw, cosYaw] = [Math.sin(yaw), Math.cos(yaw)];
	const [sinPitch, cosPitch] = [Math.sin(pitch), Math.cos(pitch)];
	const unit = DRAWN_HALF / frame.extent;
	const relative = {
		x: point.x * unit,
		y: point.y * unit,
		z: (point.z - frame.extent * (TOP_RATIO / 2)) * unit,
	};
	const x = relative.x * cosYaw - relative.y * sinYaw;
	const y = relative.x * sinYaw + relative.y * cosYaw;
	const up = relative.z * cosPitch - y * sinPitch;
	const depth = Math.max(
		MIN_DEPTH,
		CAMERA_DISTANCE - y * cosPitch - relative.z * sinPitch,
	);
	const scale = FOCAL_LENGTH / depth;
	return { x: SIZE.width / 2 + x * scale, y: SIZE.height / 2 - up * scale };
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

function polyline(points: Position3d[], frame: Frame) {
	return points.map((point) => {
		const flat = project(point, frame);
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

/**
 * Where a cylinder measures its start angle from, derived from the axis alone. Mirrors the
 * engine's own reference so the drawn start direction is the one that ranks.
 */
function axisReference(axis: Position3d) {
	const reference =
		Math.abs(axis.x) > 1 - 1e-9 ? { x: 0, y: 0, z: 1 } : { x: 1, y: 0, z: 0 };
	const along = axis.x * reference.x + axis.y * reference.y + axis.z * reference.z;
	const start = normalize({
		x: reference.x - axis.x * along,
		y: reference.y - axis.y * along,
		z: reference.z - axis.z * along,
	});
	return [start, cross(axis, start)] as const;
}

function ring(
	centre: Position3d,
	axis: Position3d,
	radius: number,
	frame: Frame,
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
		frame,
	).join(" ");
}

function cubeEdges(extent: number): ReadonlyArray<[Position3d, Position3d]> {
	const top = extent * TOP_RATIO;
	const corners: Position3d[] = [];
	for (const x of [-extent, extent])
		for (const y of [-extent, extent]) for (const z of [0, top]) corners.push({ x, y, z });
	const edges: [Position3d, Position3d][] = [];
	for (const a of corners)
		for (const b of corners) {
			const shared = Number(a.x === b.x) + Number(a.y === b.y) + Number(a.z === b.z);
			if (shared === 2 && (a.x < b.x || a.y < b.y || a.z < b.z)) edges.push([a, b]);
		}
	return edges;
}

function ProjectionBody({
	projection,
	frame,
}: {
	projection: SpatialProjection;
	frame: Frame;
}) {
	const kind = projectionKind(projection);
	const centre = projection.anchor;
	const radius = frame.extent * 0.8;

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
					points={polyline(corners, frame).join(" ")}
				/>
				<polyline
					className="projection-stage-start"
					points={polyline(
						[centre, add(centre, scaled(normal, radius))],
						frame,
					).join(" ")}
					markerEnd="url(#projection-arrow)"
				/>
			</>
		);
	}

	if (kind === "cylindrical") {
		// The direction is the axis and the rotation is the start angle around it, exactly as
		// the engine reads them.
		const axis = normalize(projection.view_direction);
		const [u, v] = basis(axis);
		const top = add(centre, scaled(axis, frame.extent));
		const bottom = add(centre, scaled(axis, -frame.extent));
		const [seed, side] = axisReference(axis);
		const start = radians(projection.rotation_degrees);
		const startDirection = normalize(
			add(scaled(seed, Math.cos(start)), scaled(side, Math.sin(start))),
		);
		return (
			<>
				<polyline
					className="projection-stage-surface-line"
					points={ring(top, axis, radius, frame)}
				/>
				<polyline
					className="projection-stage-surface-line"
					points={ring(bottom, axis, radius, frame)}
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
							frame,
						).join(" ")}
					/>
				))}
				<polyline
					className="projection-stage-axis"
					points={polyline([bottom, top], frame).join(" ")}
				/>
				<polyline
					className="projection-stage-start"
					points={polyline(
						[centre, add(centre, scaled(startDirection, radius))],
						frame,
					).join(" ")}
					markerEnd="url(#projection-arrow)"
				/>
			</>
		);
	}

	// Spherical: three great circles read as a sphere, plus the ray to the spread's centre,
	// which is simply the direction.
	const direction = normalize(projection.view_direction);
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
					points={ring(centre, axis, radius, frame)}
				/>
			))}
			<polyline
				className="projection-stage-start"
				points={polyline(
					[centre, add(centre, scaled(direction, radius))],
					frame,
				).join(" ")}
				markerEnd="url(#projection-arrow)"
			/>
		</>
	);
}

const NO_FIXTURES: readonly StageFixtureDot[] = [];

export function ProjectionStagePreview({
	projection,
	fixtures = NO_FIXTURES,
}: {
	projection: SpatialProjection;
	/** Lamp positions drawn as plain dots. No beams: this places the shape, it does not light. */
	fixtures?: readonly StageFixtureDot[];
}) {
	const [view, setView] = useState<View>(DEFAULT_VIEW);
	const drag = useRef<{ x: number; y: number } | null>(null);
	const kind = projectionKind(projection);
	const extent = useMemo(
		() => worldExtent(projection, fixtures),
		[projection, fixtures],
	);
	const frame: Frame = { ...view, extent };

	const orbit = useCallback((dx: number, dy: number) => {
		setView((current) => ({
			yaw: current.yaw + dx,
			// Past vertical the scene turns inside out, so the pitch stops at the poles.
			pitch: Math.min(89, Math.max(-89, current.pitch + dy)),
		}));
	}, []);

	const centre = project(projection.anchor, frame);
	const label = fixtures.length
		? `${kind} projection over ${fixtures.length} ${fixtures.length === 1 ? "lamp" : "lamps"} in the Stage, drag to orbit`
		: `${kind} projection in the Stage, drag to orbit`;
	return (
		<svg
			className="projection-stage-preview"
			viewBox={`0 0 ${SIZE.width} ${SIZE.height}`}
			role="img"
			aria-label={label}
			tabIndex={0}
			onPointerDown={(event) => {
				drag.current = { x: event.clientX, y: event.clientY };
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (!drag.current) return;
				// The pointer drags the camera, not the scene, so yaw follows the hand and pitch
				// lifts the way a hand lifts a lid.
				orbit(
					(drag.current.x - event.clientX) * 0.6,
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
			<title>{label}</title>
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
				{cubeEdges(extent).map(([from, to]) => {
					const a = project(from, frame);
					const b = project(to, frame);
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
			<ProjectionBody projection={projection} frame={frame} />
			<g className="projection-stage-fixtures">
				{fixtures.map((fixture) => {
					const dot = project(fixture.position, frame);
					return (
						<circle
							key={fixture.id}
							className={
								fixture.selected
									? "projection-stage-fixture selected"
									: "projection-stage-fixture"
							}
							cx={dot.x}
							cy={dot.y}
							r={fixture.selected ? 3 : 2}
						/>
					);
				})}
			</g>
			<circle
				className="projection-stage-centre"
				cx={centre.x}
				cy={centre.y}
				r={4}
			/>
		</svg>
	);
}
