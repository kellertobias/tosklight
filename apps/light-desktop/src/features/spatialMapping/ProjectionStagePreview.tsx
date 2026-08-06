import type { Position3d, SpatialProjection } from "./contracts";
import { axisRotation, projectionKind } from "./projectionKinds";

/**
 * A simplified isometric view of the Stage showing where a projection sits in it: the centre
 * point, the axis it spreads around, and the direction the spread starts from.
 *
 * The Stage box is a fixed reference cube rather than the real Stage extents. It exists to
 * make rotations and offsets legible, not to measure them.
 */

const HALF = 5;
const SIZE = { width: 260, height: 190 };

/** World is Z-up: `Top` looks down -Z and `Front` looks +Y. */
function project(point: Position3d) {
	const scale = 15;
	return {
		x: SIZE.width / 2 + (point.x - point.y) * 0.87 * scale,
		y: SIZE.height / 2 + ((point.x + point.y) * 0.5 - point.z) * scale,
	};
}

/** Euler degrees about X, then Y, then Z — the same order the engine applies. */
function rotate(vector: Position3d, degrees: Position3d): Position3d {
	const rad = (value: number) => (value * Math.PI) / 180;
	const [sx, cx] = [Math.sin(rad(degrees.x)), Math.cos(rad(degrees.x))];
	const [sy, cy] = [Math.sin(rad(degrees.y)), Math.cos(rad(degrees.y))];
	const [sz, cz] = [Math.sin(rad(degrees.z)), Math.cos(rad(degrees.z))];
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

function scaled(vector: Position3d, by: number): Position3d {
	return { x: vector.x * by, y: vector.y * by, z: vector.z * by };
}

function offset(from: Position3d, by: Position3d): Position3d {
	return { x: from.x + by.x, y: from.y + by.y, z: from.z + by.z };
}

const CUBE_EDGES: ReadonlyArray<[Position3d, Position3d]> = (() => {
	const corners: Position3d[] = [];
	for (const x of [-HALF, HALF])
		for (const y of [-HALF, HALF]) for (const z of [0, HALF * 1.2]) corners.push({ x, y, z });
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

function line(from: Position3d, to: Position3d) {
	const a = project(from);
	const b = project(to);
	return { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
}

export function ProjectionStagePreview({
	projection,
}: {
	projection: SpatialProjection;
}) {
	const kind = projectionKind(projection);
	const centre = projection.anchor;
	const axis =
		kind === "cylindrical"
			? normalize(rotate({ x: 0, y: 0, z: 1 }, axisRotation(projection)))
			: null;
	const startAngle = ((projection.start_angle_degrees ?? 0) * Math.PI) / 180;

	// Where the spread starts. For a cylinder that is a direction perpendicular to the axis;
	// for a sphere it is the point the two angles name.
	let startRay: Position3d | null = null;
	if (kind === "cylindrical" && axis) {
		const seed = normalize(rotate({ x: 1, y: 0, z: 0 }, axisRotation(projection)));
		const side = cross(axis, seed);
		startRay = normalize({
			x: seed.x * Math.cos(startAngle) + side.x * Math.sin(startAngle),
			y: seed.y * Math.cos(startAngle) + side.y * Math.sin(startAngle),
			z: seed.z * Math.cos(startAngle) + side.z * Math.sin(startAngle),
		});
	} else if (kind === "spherical") {
		const elevation = ((projection.elevation_degrees ?? 0) * Math.PI) / 180;
		startRay = {
			x: Math.cos(elevation) * Math.cos(startAngle),
			y: Math.cos(elevation) * Math.sin(startAngle),
			z: Math.sin(elevation),
		};
	} else {
		startRay = normalize(projection.view_direction);
	}

	const centrePoint = project(centre);
	return (
		<svg
			className="projection-stage-preview"
			viewBox={`0 0 ${SIZE.width} ${SIZE.height}`}
			role="img"
			aria-label={`${kind} projection in the Stage`}
		>
			<title>{`${kind} projection in the Stage`}</title>
			<g className="projection-stage-box">
				{CUBE_EDGES.map(([from, to]) => (
					<line key={`${JSON.stringify(from)}-${JSON.stringify(to)}`} {...line(from, to)} />
				))}
			</g>
			{kind === "spherical" && (
				<circle
					className="projection-stage-sphere"
					cx={centrePoint.x}
					cy={centrePoint.y}
					r={38}
				/>
			)}
			{axis && (
				<line
					className="projection-stage-axis"
					{...line(offset(centre, scaled(axis, -HALF)), offset(centre, scaled(axis, HALF)))}
				/>
			)}
			{startRay && (
				<line
					className="projection-stage-start"
					{...line(centre, offset(centre, scaled(startRay, HALF * 0.8)))}
				/>
			)}
			<circle
				className="projection-stage-centre"
				cx={centrePoint.x}
				cy={centrePoint.y}
				r={4}
			/>
		</svg>
	);
}
