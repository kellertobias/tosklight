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
	profileMode,
	resolvedColor,
} from "./attributeValues";
import { createImprovedBeamMesh } from "./emitterGeometry";
import { applyInstalledAppearance } from "./installedAppearance";
import type { StageProceduralResourceCache } from "./resources";
import {
	addSelectionOutline,
	emitterSurfaceMaterial,
	fixtureBody,
} from "./sceneObjects";
import { applyStageShaper, resolveStageShaper } from "./shaperAppearance";
import type {
	FixtureAttributeValues,
	Stage3dFixture,
	StageShaperState,
} from "./types";

type FallbackRenderState = {
	intensity: number;
	pan: number;
	tilt: number;
	focus: number;
	color: THREE.Color;
	distance: number;
	radius: number;
	shaper: StageShaperState;
};

const directionalFixtures = new WeakMap<PatchedFixture, boolean>();
const sourceVisibleColor = new THREE.Color();
const sourceBrightColor = new THREE.Color();
const sourceBaseColor = new THREE.Color(0x485158);
const sourceWhiteColor = new THREE.Color(0xffffff);
const rootBeamDirection = new THREE.Vector3();
const rootBeamDown = new THREE.Vector3(0, -1, 0);

export function fallbackEmitterIsDirectional(fixture: PatchedFixture) {
	const retained = directionalFixtures.get(fixture);
	if (retained !== undefined) return retained;
	const text =
		`${fixture.definition.device_type} ${fixture.definition.manufacturer} ${fixture.definition.name} ${fixture.definition.model}`.toLowerCase();
	const directional =
		!/sun\s*strip|sunstrip|strip light|striplight/.test(text) &&
		(!/\bstrobe\b/.test(text) || /blinder/.test(text));
	directionalFixtures.set(fixture, directional);
	return directional;
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
		color: applyInstalledAppearance(
			resolvedColor(attributes.get("color"), attributes),
			item.fixture,
			item.installedAppearance,
		),
		distance,
		radius: Math.tan(THREE.MathUtils.degToRad(4 + zoom * 23)) * distance,
		shaper: resolveStageShaper(
			item.fixture,
			profileMode(item.fixture),
			attributes,
			item.installedAppearance,
			item.shaperAngle,
		),
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
	applyFixtureRootTransform(root, item);
	return { root, instanceId };
}

export function applyFixtureRootTransform(
	root: THREE.Object3D,
	item: Stage3dFixture,
) {
	root.position.set(item.position.x, item.position.z, -item.position.y);
	root.rotation.set(
		THREE.MathUtils.degToRad(item.position.rotationX),
		THREE.MathUtils.degToRad(item.position.rotationZ),
		THREE.MathUtils.degToRad(item.position.rotationY),
	);
	// The bracket turns the fixture in its own frame, after the placement rotation: a lantern
	// faced across the stage and then angled down in its clamp points where both of those say.
	if (item.bracketAngle)
		root.rotateX(THREE.MathUtils.degToRad(item.bracketAngle));
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
	volume.scale.set(state.radius, state.distance, state.radius);
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

function createBeamCenter(
	state: FallbackRenderState,
	active = state.intensity > 0.001,
) {
	const geometry = new THREE.BufferGeometry().setFromPoints([
		new THREE.Vector3(),
		new THREE.Vector3(0, -state.distance, 0),
	]);
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
	resources?: StageProceduralResourceCache,
) {
	const createCone = () => {
		const geometry = new THREE.ConeGeometry(1, 1, 32, 1, true);
		geometry.translate(0, -0.5, 0);
		return geometry;
	};
	const cone =
		resources?.geometry("fallback-beam-cone:32:unit", createCone) ??
		createCone();
	if (beamAtRoot) addBeamSource(beam, state);
	const active = state.intensity > 0.001;
	const directional = fallbackEmitterIsDirectional(fixture);
	beam.userData.stageDirectionalBeam = directional;
	beam.userData.stageBeamActive = active;
	beam.userData.stageBeamIntensity = state.intensity;
	beam.userData.stageBeamColor = `#${state.color.getHexString()}`;
	beam.userData.stageBeamRadius = state.radius;
	beam.userData.stageBeamDistance = state.distance;
	applyStageShaper(beam, state.shaper);
	if (!directional) return;
	const drawBeams = active && renderQuality !== "lines_only";
	const drawLines =
		active &&
		(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	const volume = createFallbackVolume(cone, state);
	volume.visible = drawBeams && renderQuality !== "improved_beams";
	beam.add(volume);
	if (renderQuality === "improved_beams") {
		const improved = createImprovedBeamMesh(
			cone,
			state.color,
			state.intensity,
			state.focus,
		);
		improved.scale.set(state.radius, state.distance, state.radius);
		improved.visible = drawBeams;
		beam.add(improved);
	}
	const center = createBeamCenter(state, true);
	center.visible = drawLines;
	beam.add(center);
	const guide = createBeamCenter(state, false);
	guide.visible = !active && showBeamGuides;
	beam.add(guide);
	if (drawBeams) addGoboSpokes(beam, fixture, attributes, state);
}

function orientRootBeam(beam: THREE.Group, state: FallbackRenderState) {
	beam.position.y = -0.62;
	rootBeamDirection
		.set(
			-Math.sin(state.pan) * Math.sin(state.tilt),
			-Math.cos(state.tilt),
			-Math.cos(state.pan) * Math.sin(state.tilt),
		)
		.normalize();
	beam.quaternion.setFromUnitVectors(rootBeamDown, rootBeamDirection);
}

function fallbackBeamParent(
	root: THREE.Group,
	item: Stage3dFixture,
	state: FallbackRenderState,
	selected: boolean,
	resources?: StageProceduralResourceCache,
) {
	if (item.fixture.definition.model_asset) {
		root.add(fixtureBody(selected, resources));
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
	resources?: StageProceduralResourceCache,
) {
	const beamParent = fallbackBeamParent(root, item, state, selected, resources);
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
		resources,
	);
	beamParent.add(beam);
}

function updateBuiltInSource(source: THREE.Mesh, state: FallbackRenderState) {
	const level = THREE.MathUtils.clamp(state.intensity, 0, 1);
	sourceBrightColor
		.copy(state.color)
		.lerp(sourceWhiteColor, 0.82)
		.multiplyScalar(2.98);
	sourceVisibleColor.copy(sourceBaseColor).lerp(sourceBrightColor, level);
	const material = source.material as THREE.Material & {
		color?: THREE.Color;
	};
	material.color?.copy(sourceVisibleColor);
	source.userData.active = level > 0.001;
}

type FallbackFixtureRuntime = {
	yoke: THREE.Object3D | null;
	head: THREE.Object3D | null;
	scannerPan: THREE.Object3D | null;
	scannerTilt: THREE.Object3D | null;
	scannerBeam: THREE.Object3D | null;
	sources: THREE.Mesh[];
	beam: THREE.Group | null;
};

function fallbackFixtureRuntime(root: THREE.Object3D) {
	const retained = root.userData.stageFallbackFixtureRuntime as
		| FallbackFixtureRuntime
		| undefined;
	if (retained) return retained;
	const sources: THREE.Mesh[] = [];
	root.traverse((object) => {
		if (
			object.name.startsWith("light-emitting-surface") &&
			object instanceof THREE.Mesh
		)
			sources.push(object);
	});
	const beam = root.getObjectByName("fallback-beam");
	const runtime: FallbackFixtureRuntime = {
		yoke: root.getObjectByName("centered-rotating-yoke") ?? null,
		head: root.getObjectByName("tilting-head") ?? null,
		scannerPan: root.getObjectByName("scanner-pan") ?? null,
		scannerTilt: root.getObjectByName("scanner-tilt") ?? null,
		scannerBeam: root.getObjectByName("scanner-beam-mount") ?? null,
		sources,
		beam: beam instanceof THREE.Group ? beam : null,
	};
	root.userData.stageFallbackFixtureRuntime = runtime;
	return runtime;
}

export function updateFallbackFixture(
	root: THREE.Object3D,
	item: Stage3dFixture,
	_attributes: FixtureAttributeValues,
	state: FallbackRenderState,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
	resources?: StageProceduralResourceCache,
) {
	const runtime = fallbackFixtureRuntime(root);
	const yoke = runtime.yoke;
	if (yoke) yoke.rotation.y = state.pan;
	const head = runtime.head;
	if (head) head.rotation.x = state.tilt;
	const scannerPan = runtime.scannerPan;
	if (scannerPan) scannerPan.rotation.y = state.pan;
	const scannerTilt = runtime.scannerTilt;
	if (scannerTilt) scannerTilt.rotation.x = Math.PI / 4 + state.tilt / 2;
	const scannerBeam = runtime.scannerBeam;
	if (scannerBeam) scannerBeam.rotation.x = Math.PI / 2 - state.tilt;
	for (const source of runtime.sources) updateBuiltInSource(source, state);
	const beam = runtime.beam;
	if (!beam) return;
	if (beam.parent === root) orientRootBeam(beam, state);
	applyStageShaper(beam, state.shaper);
	const previousRadius = Number(beam.userData.stageBeamRadius) || state.radius;
	const radiusScale = state.radius / Math.max(previousRadius, 1e-6);
	beam.userData.stageBeamActive = state.intensity > 0.001;
	beam.userData.stageBeamIntensity = state.intensity;
	beam.userData.stageBeamColor = `#${state.color.getHexString()}`;
	beam.userData.stageBeamRadius = state.radius;
	const active = state.intensity > 0.001;
	const drawBeams = active && renderQuality !== "lines_only";
	const drawLines =
		active &&
		(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	const existingImproved = beam.getObjectByName("beam-improved-volume");
	if (renderQuality === "improved_beams" && !existingImproved) {
		const volume = beam.getObjectByName("beam-volume");
		if (volume instanceof THREE.Mesh) {
			const improved = createImprovedBeamMesh(
				resources ? volume.geometry : volume.geometry.clone(),
				state.color,
				state.intensity,
				state.focus,
			);
			improved.scale.copy(volume.scale);
			beam.add(improved);
		}
	}
	for (const object of beam.children) {
		if (
			object.name === "beam-volume" ||
			object.name === "beam-improved-volume"
		) {
			object.visible =
				drawBeams &&
				(object.name === "beam-improved-volume") ===
					(renderQuality === "improved_beams");
			object.scale.x *= radiusScale;
			object.scale.z *= radiusScale;
			if (object instanceof THREE.Mesh) {
				if (object.material instanceof THREE.ShaderMaterial) {
					object.material.uniforms.beamColor.value.copy(state.color);
					object.material.uniforms.beamOpacity.value =
						state.intensity * (0.045 + state.focus * 0.055);
				} else {
					const material = object.material as THREE.MeshBasicMaterial;
					material.color.copy(state.color);
					material.opacity = state.intensity * (0.035 + state.focus * 0.055);
				}
			}
		}
		if (object.name === "beam-centerline") {
			object.visible = drawLines;
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
