import * as THREE from "three";
import type {
	AttributeValue,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
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
	const pan = (fixtureParameter(item, attributes, "pan", 0.5) - 0.5) *
		Math.PI *
		2;
	const tilt = movingLightTiltRadians(
		fixtureParameter(item, attributes, "tilt", 0.5),
	);
	const zoom = fixtureParameter(item, attributes, "zoom", 0.35);
	const distance = 7;
	return {
		intensity: fallbackIntensity(
			item,
			attributes,
			snapshot,
			virtualHighlight,
		),
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
	return new THREE.Mesh(
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
	const showGuide =
		active || (fallbackEmitterIsDirectional(fixture) && showBeamGuides);
	beam.add(createFallbackVolume(cone, state));
	if (showGuide) beam.add(createBeamOutline(cone, state));
	if (showGuide) beam.add(createBeamCenter(state));
	addGoboSpokes(beam, fixture, attributes, state);
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
	if (selected) addSelectionOutline(model.object);
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
) {
	const beamParent = fallbackBeamParent(root, item, state, selected);
	const beam = new THREE.Group();
	const beamAtRoot = beamParent === root;
	if (beamAtRoot) orientRootBeam(beam, state);
	addFallbackBeamVisuals(
		beam,
		item.fixture,
		attributes,
		state,
		showBeamGuides,
		beamAtRoot,
	);
	beamParent.add(beam);
}
