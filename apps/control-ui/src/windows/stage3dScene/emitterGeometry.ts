import * as THREE from "three";
import type { GeometryEmitter } from "../../api/types";
import { normalized } from "./attributeValues";
import { emitterSurfaceMaterial, millimetres } from "./sceneObjects";
import type { FixtureAttributeValues } from "./types";

type BeamMetrics = {
	distance: number;
	focus: number;
	beamAngle: number;
	fieldAngle: number;
	beamRadius: number;
	radius: number;
};

type EmitterSourceContext = {
	emitter: GeometryEmitter;
	color: THREE.Color;
	intensity: number;
	metrics: BeamMetrics;
	showBeamGuides: boolean;
};

function matrixOffsets(layout: Extract<GeometryEmitter["layout"], { type: "matrix" }>) {
	const offsets: THREE.Vector3[] = [];
	for (let row = 0; row < layout.rows; row++) {
		for (let column = 0; column < layout.columns; column++) {
			offsets.push(
				new THREE.Vector3(
					((column - (layout.columns - 1) / 2) * layout.spacing.x) / 1_000,
					((row - (layout.rows - 1) / 2) * layout.spacing.y) / 1_000,
					((row - (layout.rows - 1) / 2) * layout.spacing.z) / 1_000,
				),
			);
		}
	}
	return offsets;
}

function layoutOffsets(layout: GeometryEmitter["layout"]) {
	if (layout.type === "point") return [new THREE.Vector3()];
	if (layout.type === "explicit_pixels") return layout.positions.map(millimetres);
	if (layout.type === "matrix") return matrixOffsets(layout);
	return Array.from({ length: layout.count }, (_, index) => {
		if (layout.type === "ring") {
			const angle = (index / layout.count) * Math.PI * 2;
			return new THREE.Vector3(
				(Math.cos(angle) * layout.radius_millimetres) / 1_000,
				0,
				(Math.sin(angle) * layout.radius_millimetres) / 1_000,
			);
		}
		return new THREE.Vector3(
			((index - (layout.count - 1) / 2) * layout.spacing_millimetres) /
				1_000,
			0,
			0,
		);
	});
}

function resolveBeamMetrics(
	emitter: GeometryEmitter,
	attributes: FixtureAttributeValues,
): BeamMetrics {
	const distance = 7;
	const zoom = normalized(
		attributes.get("beam.zoom") ?? attributes.get("zoom"),
		0.5,
	);
	const focus = normalized(
		attributes.get("beam.focus") ?? attributes.get("focus"),
		emitter.focus,
	);
	const zoomScale = 0.6 + zoom * 0.8;
	const beamAngle = emitter.beam_angle_degrees * zoomScale;
	const fieldAngle = emitter.field_angle_degrees * zoomScale;
	return {
		distance,
		focus,
		beamAngle,
		fieldAngle,
		beamRadius:
			Math.tan(THREE.MathUtils.degToRad(beamAngle / 2)) * distance,
		radius: Math.tan(THREE.MathUtils.degToRad(fieldAngle / 2)) * distance,
	};
}

function createConeGeometry(radius: number, distance: number) {
	const geometry = new THREE.ConeGeometry(radius, distance, 24, 1, true);
	geometry.translate(0, -distance / 2, 0);
	return geometry;
}

function createBeamMesh(
	geometry: THREE.BufferGeometry,
	color: THREE.Color,
	opacity: number,
	name: string,
) {
	const mesh = new THREE.Mesh(
		geometry,
		new THREE.MeshBasicMaterial({
			color,
			transparent: true,
			opacity,
			side: THREE.DoubleSide,
			depthWrite: false,
			blending: THREE.AdditiveBlending,
		}),
	);
	mesh.name = name;
	return mesh;
}

function createSourceSurface(context: EmitterSourceContext) {
	const { radius } = context.metrics;
	const source = new THREE.Mesh(
		new THREE.CircleGeometry(Math.max(0.012, Math.min(0.08, radius / 18)), 12),
		emitterSurfaceMaterial(context.color, context.intensity),
	);
	source.name = "light-emitting-surface";
	source.userData.active = context.intensity > 0.001;
	source.rotation.x = -Math.PI / 2;
	return source;
}

function createBeamOutline(
	geometry: THREE.BufferGeometry,
	color: THREE.Color,
	intensity: number,
) {
	const active = intensity > 0.001;
	const material = active
		? new THREE.LineBasicMaterial({
				color,
				transparent: true,
				opacity: 0.25 + intensity * 0.5,
			})
		: new THREE.LineDashedMaterial({
				color: 0x7b858d,
				transparent: true,
				opacity: 0.3,
				dashSize: 0.18,
				gapSize: 0.14,
			});
	const outline = new THREE.LineSegments(
		new THREE.EdgesGeometry(geometry, 28),
		material,
	);
	outline.name = "beam-outline";
	if (!active) outline.computeLineDistances();
	return outline;
}

function createInactiveBeamGuide(distance: number) {
	const geometry = new THREE.BufferGeometry().setFromPoints([
		new THREE.Vector3(),
		new THREE.Vector3(0, -distance, 0),
	]);
	const guide = new THREE.Line(
		geometry,
		new THREE.LineDashedMaterial({
			color: 0x8d989f,
			transparent: true,
			opacity: 0.42,
			dashSize: 0.18,
			gapSize: 0.14,
		}),
	);
	guide.name = "beam-direction-guide";
	guide.computeLineDistances();
	return guide;
}

function createEmitterSource(
	offset: THREE.Vector3,
	index: number,
	context: EmitterSourceContext,
) {
	const { emitter, metrics, intensity, color } = context;
	const beam = new THREE.Group();
	beam.name = `geometry-source:${emitter.id}:${index}`;
	beam.position.copy(offset);
	beam.userData.emitterId = emitter.id;
	beam.userData.headId = emitter.head_id;
	beam.userData.layout = emitter.layout.type;
	const cone = createConeGeometry(metrics.radius, metrics.distance);
	const volumeOpacity =
		intensity *
		(0.025 + (1 - emitter.feather) * 0.035 + metrics.focus * 0.04);
	const volume = createBeamMesh(cone, color, volumeOpacity, "beam-volume");
	const core = createBeamMesh(
		createConeGeometry(metrics.beamRadius, metrics.distance),
		color,
		intensity * (0.02 + metrics.focus * 0.045),
		"beam-core",
	);
	beam.add(createSourceSurface(context), volume, core);
	const active = intensity > 0.001;
	const directional = emitter.directional ?? true;
	if (active || (directional && context.showBeamGuides)) {
		beam.add(createBeamOutline(cone, color, intensity));
	}
	if (!active && directional && context.showBeamGuides) {
		beam.add(createInactiveBeamGuide(metrics.distance));
	}
	return beam;
}

function createEmitterGroup(
	emitter: GeometryEmitter,
	offsets: THREE.Vector3[],
	metrics: BeamMetrics,
	intensity: number,
	color: THREE.Color,
) {
	const group = new THREE.Group();
	group.name = `geometry-emitter:${emitter.id}`;
	group.userData.beamAngleDegrees = metrics.beamAngle;
	group.userData.fieldAngleDegrees = metrics.fieldAngle;
	group.userData.feather = emitter.feather;
	group.userData.focus = metrics.focus;
	group.userData.sourceCount = offsets.length;
	group.userData.intensity = intensity;
	group.userData.color = `#${color.getHexString()}`;
	group.position.copy(millimetres(emitter.origin));
	group.rotation.set(
		THREE.MathUtils.degToRad(emitter.orientation_degrees.x),
		THREE.MathUtils.degToRad(emitter.orientation_degrees.y),
		THREE.MathUtils.degToRad(emitter.orientation_degrees.z),
	);
	return group;
}

export function buildGeometryBeam(
	emitter: GeometryEmitter,
	attributes: FixtureAttributeValues,
	intensity: number,
	color: THREE.Color,
	showBeamGuides: boolean,
) {
	const metrics = resolveBeamMetrics(emitter, attributes);
	const offsets = layoutOffsets(emitter.layout);
	const group = createEmitterGroup(
		emitter,
		offsets,
		metrics,
		intensity,
		color,
	);
	const context = { emitter, color, intensity, metrics, showBeamGuides };
	offsets.forEach((offset, index) => {
		group.add(createEmitterSource(offset, index, context));
	});
	return group;
}
