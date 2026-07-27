import * as THREE from "three";
import type {
	AttributeValue,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import type { StageRenderQuality } from "../../types";
import {
	createBuiltInFixtureModel,
	movingLightTiltRadians,
} from "../builtInStageModels";
import {
	capabilityName,
	normalized,
	parameterDefault,
	resolvedColor,
} from "./attributeValues";
import {
	addSelectionOutline,
	emitterSurfaceMaterial,
	fixtureBody,
} from "./sceneObjects";
import type { FixtureAttributeValues, Stage3dFixture } from "./types";

type FallbackRenderState = {
	intensity: number;
	pan: number;
	tilt: number;
	focus: number;
	color: THREE.Color;
	distance: number;
	radius: number;
};

export function fallbackEmitterIsDirectional(fixture: PatchedFixture) {
	const text =
		`${fixture.definition.device_type} ${fixture.definition.manufacturer} ${fixture.definition.name} ${fixture.definition.model}`.toLowerCase();
	if (/sun\s*strip|sunstrip|strip light|striplight/.test(text)) return false;
	if (/\bstrobe\b/.test(text) && !/blinder/.test(text)) return false;
	return true;
}

function fallbackIntensity(
	item: Stage3dFixture,
	attributes: FixtureAttributeValues,
	snapshot: VisualizationSnapshot | null,
	virtualHighlight: boolean,
) {
	if (virtualHighlight) return 1;
	const intensity = normalized(
		attributes.get("intensity"),
		parameterDefault(item.fixture, "intensity", 0),
	);
	return (snapshot?.blackout ? 0 : intensity) * (snapshot?.grand_master ?? 1);
}

function fixtureParameter(
	item: Stage3dFixture,
	attributes: FixtureAttributeValues,
	attribute: string,
	fallback: number,
) {
	return normalized(
		attributes.get(attribute),
		parameterDefault(item.fixture, attribute, fallback),
	);
}

export function fallbackRenderState(
	item: Stage3dFixture,
	attributes: FixtureAttributeValues,
	snapshot: VisualizationSnapshot | null,
	virtualHighlight: boolean,
): FallbackRenderState {
	const pan =
		(fixtureParameter(item, attributes, "pan", 0.5) - 0.5) * Math.PI * 2;
	const tilt = movingLightTiltRadians(
		fixtureParameter(item, attributes, "tilt", 0.5),
	);
	const zoom = fixtureParameter(item, attributes, "zoom", 0.35);
	const distance = 7;
	return {
		intensity: fallbackIntensity(item, attributes, snapshot, virtualHighlight),
		pan,
		tilt,
		focus: fixtureParameter(item, attributes, "focus", 0.65),
		color: resolvedColor(attributes.get("color"), attributes),
		distance,
		radius: Math.tan(THREE.MathUtils.degToRad(4 + zoom * 23)) * distance,
	};
}

export function createFixtureRoot(item: Stage3dFixture, selected: boolean) {
	const id = item.fixture.fixture_id;
	const instanceId = item.instanceId ?? id;
	const root = new THREE.Group();
	root.name = `fixture:${id}:${instanceId}`;
	root.userData.fixtureId = id;
	root.userData.instanceId = instanceId;
	root.userData.stageSelected = selected;
	root.position.set(item.position.x, item.position.z, -item.position.y);
	root.rotation.set(
		THREE.MathUtils.degToRad(item.position.rotationX),
		THREE.MathUtils.degToRad(item.position.rotationZ),
		THREE.MathUtils.degToRad(item.position.rotationY),
	);
	return { root, instanceId };
}

function createFallbackVolume(
	geometry: THREE.BufferGeometry,
	state: FallbackRenderState,
) {
	const volume = new THREE.Mesh(
		geometry,
		new THREE.MeshBasicMaterial({
			color: state.color,
			transparent: true,
			opacity: state.intensity * (0.035 + state.focus * 0.055),
			side: THREE.DoubleSide,
			depthWrite: false,
			blending: THREE.AdditiveBlending,
		}),
	);
	volume.name = "beam-volume";
	return volume;
}

function addBeamSource(beam: THREE.Group, state: FallbackRenderState) {
	const source = new THREE.Mesh(
		new THREE.CircleGeometry(
			Math.max(0.04, Math.min(0.11, state.radius / 16)),
			24,
		),
		emitterSurfaceMaterial(state.color, state.intensity),
	);
	source.name = "light-emitting-surface";
	source.userData.active = state.intensity > 0.001;
	source.rotation.x = -Math.PI / 2;
	beam.add(source);
}

function guideMaterial(state: FallbackRenderState) {
	if (state.intensity > 0.001) {
		return new THREE.LineBasicMaterial({
			color: state.color,
			transparent: true,
			opacity: 0.28 + state.intensity * 0.55,
		});
	}
	return new THREE.LineDashedMaterial({
		color: 0x7b858d,
		transparent: true,
		opacity: 0.3,
		dashSize: 0.18,
		gapSize: 0.14,
	});
}

function createBeamOutline(
	geometry: THREE.BufferGeometry,
	state: FallbackRenderState,
) {
	const outline = new THREE.LineSegments(
		new THREE.EdgesGeometry(geometry, 28),
		guideMaterial(state),
	);
	outline.name = "beam-outline";
	if (state.intensity <= 0.001) outline.computeLineDistances();
	return outline;
}

function createBeamCenter(state: FallbackRenderState) {
	const geometry = new THREE.BufferGeometry().setFromPoints([
		new THREE.Vector3(),
		new THREE.Vector3(0, -state.distance, 0),
	]);
	const active = state.intensity > 0.001;
	const material = active
		? new THREE.LineBasicMaterial({
				color: state.color,
				transparent: true,
				opacity: 0.45 + state.intensity * 0.4,
			})
		: new THREE.LineDashedMaterial({
				color: 0x7b858d,
				transparent: true,
				opacity: 0.35,
				dashSize: 0.18,
				gapSize: 0.14,
			});
	const center = new THREE.Line(geometry, material);
	center.name = active ? "beam-centerline" : "beam-direction-guide";
	if (!active) center.computeLineDistances();
	return center;
}

function addGoboSpokes(
	beam: THREE.Group,
	fixture: PatchedFixture,
	attributes: Map<string, AttributeValue>,
	state: FallbackRenderState,
) {
	const gobo = capabilityName(fixture, "gobo", attributes.get("gobo"));
	if (!gobo || gobo.toLowerCase() === "open") return;
	for (let spoke = 0; spoke < 6; spoke++) {
		const angle = (spoke / 6) * Math.PI * 2;
		const geometry = new THREE.BufferGeometry().setFromPoints([
			new THREE.Vector3(),
			new THREE.Vector3(
				Math.cos(angle) * state.radius,
				-state.distance,
				Math.sin(angle) * state.radius,
			),
		]);
		beam.add(
			new THREE.Line(
				geometry,
				new THREE.LineBasicMaterial({
					color: state.color,
					transparent: true,
					opacity: state.intensity * 0.45,
				}),
			),
		);
	}
}

function addFallbackBeamVisuals(
	beam: THREE.Group,
	fixture: PatchedFixture,
	attributes: FixtureAttributeValues,
	state: FallbackRenderState,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
	beamAtRoot: boolean,
) {
	const cone = new THREE.ConeGeometry(
		state.radius,
		state.distance,
		32,
		1,
		true,
	);
	cone.translate(0, -state.distance / 2, 0);
	if (beamAtRoot) addBeamSource(beam, state);
	const active = state.intensity > 0.001;
	const directional = fallbackEmitterIsDirectional(fixture);
	beam.userData.stageDirectionalBeam = directional;
	beam.userData.stageBeamActive = active;
	beam.userData.stageBeamColor = `#${state.color.getHexString()}`;
	beam.userData.stageBeamRadius = state.radius;
	beam.userData.stageBeamDistance = state.distance;
	if (!directional) return;
	const drawBeams = active && renderQuality !== "lines_only";
	const drawLines =
		active &&
		(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	const volume = createFallbackVolume(cone, state);
	volume.visible = drawBeams;
	beam.add(volume);
	const visibleState = { ...state, intensity: Math.max(state.intensity, 1) };
	const outline = createBeamOutline(cone, visibleState);
	const center = createBeamCenter(visibleState);
	outline.visible = drawLines;
	center.visible = drawLines;
	beam.add(outline, center);
	const guide = createBeamCenter({ ...state, intensity: 0 });
	guide.visible = !active && showBeamGuides;
	beam.add(guide);
	if (drawBeams) addGoboSpokes(beam, fixture, attributes, state);
}

function orientRootBeam(beam: THREE.Group, state: FallbackRenderState) {
	beam.position.y = -0.62;
	const direction = new THREE.Vector3(
		-Math.sin(state.pan) * Math.sin(state.tilt),
		-Math.cos(state.tilt),
		-Math.cos(state.pan) * Math.sin(state.tilt),
	).normalize();
	beam.quaternion.setFromUnitVectors(new THREE.Vector3(0, -1, 0), direction);
}

function fallbackBeamParent(
	root: THREE.Group,
	item: Stage3dFixture,
	state: FallbackRenderState,
	selected: boolean,
) {
	if (item.fixture.definition.model_asset) {
		root.add(fixtureBody(selected));
		return root;
	}
	const model = createBuiltInFixtureModel(
		item.fixture,
		state.color,
		state.intensity,
		state.pan,
		state.tilt,
	);
	model.object.name = "fixture-placeholder";
	addSelectionOutline(model.object, selected);
	root.add(model.object);
	return model.beamMount;
}

export function mountFallbackFixture(
	root: THREE.Group,
	item: Stage3dFixture,
	attributes: FixtureAttributeValues,
	state: FallbackRenderState,
	selected: boolean,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
) {
	const beamParent = fallbackBeamParent(root, item, state, selected);
	const beam = new THREE.Group();
	beam.name = "fallback-beam";
	const beamAtRoot = beamParent === root;
	if (beamAtRoot) orientRootBeam(beam, state);
	addFallbackBeamVisuals(
		beam,
		item.fixture,
		attributes,
		state,
		showBeamGuides,
		renderQuality,
		beamAtRoot,
	);
	beamParent.add(beam);
}

function updateBuiltInSource(source: THREE.Mesh, state: FallbackRenderState) {
	const level = THREE.MathUtils.clamp(state.intensity, 0, 1);
	const visible = new THREE.Color(0x485158).lerp(
		state.color
			.clone()
			.lerp(new THREE.Color(0xffffff), 0.82)
			.multiplyScalar(2.98),
		level,
	);
	const material = source.material as THREE.Material & {
		color?: THREE.Color;
	};
	material.color?.copy(visible);
	material.needsUpdate = true;
	source.userData.active = level > 0.001;
}

export function updateFallbackFixture(
	root: THREE.Object3D,
	item: Stage3dFixture,
	attributes: FixtureAttributeValues,
	state: FallbackRenderState,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
) {
	const yoke = root.getObjectByName("centered-rotating-yoke");
	if (yoke) yoke.rotation.y = state.pan;
	const head = root.getObjectByName("tilting-head");
	if (head) head.rotation.x = state.tilt;
	const scannerPan = root.getObjectByName("scanner-pan");
	if (scannerPan) scannerPan.rotation.y = state.pan;
	const scannerTilt = root.getObjectByName("scanner-tilt");
	if (scannerTilt) scannerTilt.rotation.x = Math.PI / 4 + state.tilt / 2;
	const scannerBeam = root.getObjectByName("scanner-beam-mount");
	if (scannerBeam) scannerBeam.rotation.x = Math.PI / 2 - state.tilt;
	root.traverse((object) => {
		if (
			object.name.startsWith("light-emitting-surface") &&
			object instanceof THREE.Mesh
		)
			updateBuiltInSource(object, state);
	});
	const beam = root.getObjectByName("fallback-beam");
	if (!(beam instanceof THREE.Group)) return;
	if (beam.parent === root) orientRootBeam(beam, state);
	const previousRadius = Number(beam.userData.stageBeamRadius) || state.radius;
	const radiusScale = state.radius / Math.max(previousRadius, 1e-6);
	beam.userData.stageBeamActive = state.intensity > 0.001;
	beam.userData.stageBeamColor = `#${state.color.getHexString()}`;
	beam.userData.stageBeamRadius = state.radius;
	const active = state.intensity > 0.001;
	const drawBeams = active && renderQuality !== "lines_only";
	const drawLines =
		active &&
		(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	for (const object of beam.children) {
		if (object.name === "beam-volume") {
			object.visible = drawBeams;
			object.scale.x *= radiusScale;
			object.scale.z *= radiusScale;
			if (object instanceof THREE.Mesh) {
				const material = object.material as THREE.MeshBasicMaterial;
				material.color.copy(state.color);
				material.opacity = state.intensity * (0.035 + state.focus * 0.055);
			}
		}
		if (object.name === "beam-outline" || object.name === "beam-centerline") {
			object.visible = drawLines;
			if (object.name === "beam-outline") {
				object.scale.x *= radiusScale;
				object.scale.z *= radiusScale;
			}
			const material = (object as THREE.Line)
				.material as THREE.LineBasicMaterial;
			material.color.copy(state.color);
			material.opacity = 0.35 + state.intensity * 0.5;
		}
		if (object.name === "beam-direction-guide")
			object.visible =
				!active && fallbackEmitterIsDirectional(item.fixture) && showBeamGuides;
	}
}
