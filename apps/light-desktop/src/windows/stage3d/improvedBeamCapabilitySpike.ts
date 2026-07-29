import * as THREE from "three";
import { createImprovedBeamMesh } from "../stage3dScene/emitterGeometry";
import {
	disposeImprovedBeamLighting,
	IMPROVED_STAGE_SHADOW_MAP_SIZE,
	MAX_IMPROVED_STAGE_LIGHTS,
	refreshImprovedBeamLighting,
} from "../stage3dScene/improvedBeamLighting";

const RENDER_SIZE = 256;
const LARGE_STAGE_INSTANCES = 500;
const PERFORMANCE_FRAMES = 60;

export interface ImprovedBeamCapabilitySpikeResult {
	technique: {
		surfaceLighting: string;
		firstOccluderTermination: string;
		shadows: string;
		beamVolume: string;
	};
	capabilities: {
		isWebGL2: boolean;
		renderer: string | null;
		vendor: string | null;
		gpuTimerQueryAvailable: boolean;
	};
	visual: {
		unlitReceiverLuminance: number;
		litReceiverLuminance: number;
		unoccludedShadowPointLuminance: number;
		occludedShadowPointLuminance: number;
		surfaceLightingPassed: boolean;
		shadowPassed: boolean;
	};
	termination: {
		firstHit: "occluder" | "receiver" | "none";
		firstHitDistance: number | null;
		receiverDistance: number | null;
		passed: boolean;
	};
	performance: {
		fixtureInstances: number;
		shadowLights: number;
		shadowMapSize: number;
		frames: number;
		synchronizedFrameP95Ms: number;
		synchronizedFrameMaxMs: number;
		maximumSynchronizedFrameP95Ms: number;
		gpuCompletionMethod: "gl.finish";
		drawCalls: number;
		triangles: number;
		geometries: number;
		textures: number;
		passed: boolean;
	};
	extensionAccepted: boolean;
}

function measureImprovedBeamPerformance(
	scene: THREE.Scene,
	renderer: THREE.WebGLRenderer,
	camera: THREE.Camera,
	primaryBeam: ReturnType<typeof productionBeam>,
) {
	const fixtureGeometry = new THREE.BoxGeometry(0.12, 0.25, 0.12);
	const fixtureMaterial = new THREE.MeshStandardMaterial({ color: 0x30343b });
	const fixtures: THREE.Mesh[] = [];
	for (let index = 0; index < LARGE_STAGE_INSTANCES; index++) {
		const fixture = new THREE.Mesh(fixtureGeometry, fixtureMaterial);
		fixture.position.set(
			(index % 25) * 0.36 - 4.32,
			0.125,
			Math.floor(index / 25) * 0.36 - 3.42,
		);
		fixture.castShadow = true;
		fixtures.push(fixture);
		scene.add(fixture);
	}
	const beams = [primaryBeam];
	for (let index = 1; index < MAX_IMPROVED_STAGE_LIGHTS; index++) {
		const beam = productionBeam(
			(index / MAX_IMPROVED_STAGE_LIGHTS) * Math.PI * 2,
			0x66ccff,
		);
		beams.push(beam);
		scene.add(beam.group);
	}
	refreshImprovedBeamLighting(scene, "improved_beams");
	renderer.shadowMap.needsUpdate = true;
	const gl = renderer.getContext();
	for (let warmup = 0; warmup < 2; warmup++) {
		renderer.render(scene, camera);
		gl.finish();
	}
	const durations: number[] = [];
	for (let frame = 0; frame < PERFORMANCE_FRAMES; frame++) {
		fixtures[frame % fixtures.length].rotation.y += 0.01;
		const startedAt = performance.now();
		refreshImprovedBeamLighting(scene, "improved_beams");
		renderer.shadowMap.needsUpdate =
			scene.userData.stageImprovedShadowsDirty === true;
		renderer.render(scene, camera);
		gl.finish();
		scene.userData.stageImprovedShadowsDirty = false;
		renderer.shadowMap.needsUpdate = false;
		durations.push(performance.now() - startedAt);
	}
	durations.sort((left, right) => left - right);
	const synchronizedFrameP95Ms = percentile(durations, 95);
	const result = {
		synchronizedFrameP95Ms,
		synchronizedFrameMaxMs: durations.at(-1) ?? 0,
		passed: synchronizedFrameP95Ms <= 16.7,
		shadowLights: scene.userData.stageImprovedBeamBudget.length as number,
		drawCalls: renderer.info.render.calls,
		triangles: renderer.info.render.triangles,
		geometries: renderer.info.memory.geometries,
		textures: renderer.info.memory.textures,
	};
	fixtureGeometry.dispose();
	fixtureMaterial.dispose();
	for (const beam of beams) {
		beam.geometry.dispose();
		beam.material.dispose();
	}
	return result;
}

function capabilitySpikeResult({
	renderer,
	unlitReceiverLuminance,
	litReceiverLuminance,
	unoccludedShadowPointLuminance,
	occludedShadowPointLuminance,
	firstHit,
	firstHitDistance,
	receiverDistance,
	terminationPassed,
	performanceResult,
}: {
	renderer: THREE.WebGLRenderer;
	unlitReceiverLuminance: number;
	litReceiverLuminance: number;
	unoccludedShadowPointLuminance: number;
	occludedShadowPointLuminance: number;
	firstHit: string | undefined;
	firstHitDistance: number | null;
	receiverDistance: number | null;
	terminationPassed: boolean;
	performanceResult: ReturnType<typeof measureImprovedBeamPerformance>;
}): ImprovedBeamCapabilitySpikeResult {
	const gl = renderer.getContext();
	const debugRenderer = gl.getExtension("WEBGL_debug_renderer_info");
	const isWebGL2 = renderer.capabilities.isWebGL2;
	const gpuTimerQueryAvailable = Boolean(
		isWebGL2
			? gl.getExtension("EXT_disjoint_timer_query_webgl2")
			: gl.getExtension("EXT_disjoint_timer_query"),
	);
	const surfaceLightingPassed =
		litReceiverLuminance >= unlitReceiverLuminance + 16;
	const shadowPassed =
		unoccludedShadowPointLuminance >= occludedShadowPointLuminance + 8 &&
		occludedShadowPointLuminance <= unoccludedShadowPointLuminance * 0.6;
	return {
		technique: {
			surfaceLighting:
				"Three.js SpotLight applied to opaque Stage receiving geometry",
			firstOccluderTermination:
				"CPU Raycaster first opaque hit shortens one retained feathered volume",
			shadows: "PCFSoftShadowMap with at most eight stable 128x128 shadow maps",
			beamVolume: "One transparent feathered cone; no stacked volume meshes",
		},
		capabilities: {
			isWebGL2,
			renderer: debugRenderer
				? String(gl.getParameter(debugRenderer.UNMASKED_RENDERER_WEBGL))
				: null,
			vendor: debugRenderer
				? String(gl.getParameter(debugRenderer.UNMASKED_VENDOR_WEBGL))
				: null,
			gpuTimerQueryAvailable,
		},
		visual: {
			unlitReceiverLuminance,
			litReceiverLuminance,
			unoccludedShadowPointLuminance,
			occludedShadowPointLuminance,
			surfaceLightingPassed,
			shadowPassed,
		},
		termination: {
			firstHit:
				firstHit === "occluder" || firstHit === "receiver" ? firstHit : "none",
			firstHitDistance,
			receiverDistance,
			passed: terminationPassed,
		},
		performance: {
			fixtureInstances: LARGE_STAGE_INSTANCES,
			shadowLights: performanceResult.shadowLights,
			shadowMapSize: IMPROVED_STAGE_SHADOW_MAP_SIZE,
			frames: PERFORMANCE_FRAMES,
			synchronizedFrameP95Ms: performanceResult.synchronizedFrameP95Ms,
			synchronizedFrameMaxMs: performanceResult.synchronizedFrameMaxMs,
			maximumSynchronizedFrameP95Ms: 16.7,
			gpuCompletionMethod: "gl.finish",
			drawCalls: performanceResult.drawCalls,
			triangles: performanceResult.triangles,
			geometries: performanceResult.geometries,
			textures: performanceResult.textures,
			passed: performanceResult.passed,
		},
		extensionAccepted:
			surfaceLightingPassed &&
			shadowPassed &&
			terminationPassed &&
			performanceResult.passed,
	};
}

function createCapabilityRenderer(canvas: HTMLCanvasElement) {
	const renderer = new THREE.WebGLRenderer({
		canvas,
		antialias: true,
		preserveDrawingBuffer: false,
	});
	renderer.setPixelRatio(1);
	renderer.setSize(RENDER_SIZE, RENDER_SIZE, false);
	renderer.shadowMap.enabled = true;
	renderer.shadowMap.type = THREE.PCFSoftShadowMap;
	renderer.shadowMap.autoUpdate = false;
	renderer.outputColorSpace = THREE.SRGBColorSpace;
	const target = new THREE.WebGLRenderTarget(RENDER_SIZE, RENDER_SIZE, {
		depthBuffer: true,
		stencilBuffer: false,
	});
	renderer.setRenderTarget(target);
	return { renderer, target };
}

export function runImprovedBeamCapabilitySpike(): ImprovedBeamCapabilitySpikeResult {
	const canvas = document.createElement("canvas");
	const { renderer, target } = createCapabilityRenderer(canvas);

	const scene = new THREE.Scene();
	scene.background = new THREE.Color(0x000000);
	const camera = new THREE.OrthographicCamera(-5, 5, 5, -5, 0.1, 30);
	camera.position.set(0, 10, 0.01);
	camera.up.set(0, 0, -1);
	camera.lookAt(0, 0, 0);
	camera.updateMatrixWorld();
	camera.updateProjectionMatrix();

	const receiverGeometry = new THREE.PlaneGeometry(10, 10);
	const receiverMaterial = new THREE.MeshStandardMaterial({
		color: 0xffffff,
		roughness: 1,
		metalness: 0,
	});
	const receiver = new THREE.Mesh(receiverGeometry, receiverMaterial);
	receiver.name = "stage-floor";
	receiver.rotation.x = -Math.PI / 2;
	receiver.receiveShadow = true;
	scene.add(receiver);

	const occluderGeometry = new THREE.BoxGeometry(0.8, 1.5, 0.8);
	const occluderMaterial = new THREE.MeshStandardMaterial({ color: 0x808080 });
	const occluder = new THREE.Mesh(occluderGeometry, occluderMaterial);
	occluder.name = "occluder";
	occluder.position.set(0, 0.75, 0);
	occluder.castShadow = true;
	scene.add(occluder);

	const buffer = new Uint8Array(RENDER_SIZE * RENDER_SIZE * 4);
	occluder.visible = false;
	renderer.render(scene, camera);
	renderer.readRenderTargetPixels(
		target,
		0,
		0,
		RENDER_SIZE,
		RENDER_SIZE,
		buffer,
	);
	const unlitReceiverLuminance = luminanceAt(
		buffer,
		new THREE.Vector3(2, 0, 1),
		camera,
	);

	const primaryBeam = productionBeam(0, 0x66ccff);
	scene.add(primaryBeam.group);
	refreshImprovedBeamLighting(scene, "improved_beams");
	renderer.shadowMap.needsUpdate = true;
	renderer.render(scene, camera);
	renderer.readRenderTargetPixels(
		target,
		0,
		0,
		RENDER_SIZE,
		RENDER_SIZE,
		buffer,
	);
	const litReceiverLuminance = luminanceAt(
		buffer,
		new THREE.Vector3(2, 0, 1),
		camera,
	);
	const shadowPoint = new THREE.Vector3(0, 0, -0.75);
	const unoccludedShadowPointLuminance = luminanceAt(
		buffer,
		shadowPoint,
		camera,
	);

	occluder.visible = true;
	refreshImprovedBeamLighting(scene, "improved_beams");
	renderer.shadowMap.needsUpdate = true;
	renderer.render(scene, camera);
	renderer.readRenderTargetPixels(
		target,
		0,
		0,
		RENDER_SIZE,
		RENDER_SIZE,
		buffer,
	);
	const occludedShadowPointLuminance = luminanceAt(buffer, shadowPoint, camera);

	const rayOrigin = primaryBeam.group.position.clone();
	const rayDirection = new THREE.Vector3(0, 0, 0).sub(rayOrigin).normalize();
	const raycaster = new THREE.Raycaster(rayOrigin, rayDirection);
	const hits = raycaster.intersectObjects([occluder, receiver], false);
	const firstHit =
		hits[0]?.object === receiver ? "receiver" : hits[0]?.object.name;
	const receiverHit = hits.find((hit) => hit.object === receiver);
	const firstHitDistance = hits[0]?.distance ?? null;
	const receiverDistance = receiverHit?.distance ?? null;
	const terminationPassed =
		firstHit === "occluder" &&
		firstHitDistance !== null &&
		receiverDistance !== null &&
		firstHitDistance < receiverDistance &&
		Math.abs(
			Number(primaryBeam.volume.userData.stageImprovedLengthRatio) -
				firstHitDistance / primaryBeam.distance,
		) < 0.01;

	const performanceResult = measureImprovedBeamPerformance(scene, renderer, camera, primaryBeam);
	const result = capabilitySpikeResult({
		renderer,
		unlitReceiverLuminance,
		litReceiverLuminance,
		unoccludedShadowPointLuminance,
		occludedShadowPointLuminance,
		firstHit,
		firstHitDistance,
		receiverDistance,
		terminationPassed,
		performanceResult,
	});

	renderer.setRenderTarget(null);
	target.dispose();
	renderer.dispose();
	receiverGeometry.dispose();
	receiverMaterial.dispose();
	occluderGeometry.dispose();
	occluderMaterial.dispose();
	disposeImprovedBeamLighting(scene);
	return result;
}

function productionBeam(
	angle: number,
	color: THREE.ColorRepresentation,
): {
	group: THREE.Group;
	volume: THREE.Mesh;
	geometry: THREE.BufferGeometry;
	material: THREE.Material;
	distance: number;
} {
	const distance = 12;
	const radius = 8;
	const group = new THREE.Group();
	group.name = `production-spike-beam:${angle}`;
	group.position.set(Math.sin(angle) * 3, 6, Math.cos(angle) * 3);
	group.quaternion.setFromUnitVectors(
		new THREE.Vector3(0, -1, 0),
		new THREE.Vector3().sub(group.position).normalize(),
	);
	group.userData.stageDirectionalBeam = true;
	group.userData.stageBeamActive = true;
	group.userData.stageBeamIntensity = 1;
	group.userData.stageBeamColor = `#${new THREE.Color(color).getHexString()}`;
	group.userData.stageBeamRadius = radius;
	group.userData.stageBeamDistance = distance;
	const geometry = new THREE.ConeGeometry(radius, distance, 24, 1, true);
	geometry.translate(0, -distance / 2, 0);
	const volume = createImprovedBeamMesh(
		geometry,
		new THREE.Color(color),
		1,
		0.65,
	);
	group.add(volume);
	return {
		group,
		volume,
		geometry,
		material: volume.material,
		distance,
	};
}

function luminanceAt(
	buffer: Uint8Array,
	point: THREE.Vector3,
	camera: THREE.Camera,
): number {
	const projected = point.clone().project(camera);
	const x = THREE.MathUtils.clamp(
		Math.round((projected.x * 0.5 + 0.5) * (RENDER_SIZE - 1)),
		0,
		RENDER_SIZE - 1,
	);
	const y = THREE.MathUtils.clamp(
		Math.round((projected.y * 0.5 + 0.5) * (RENDER_SIZE - 1)),
		0,
		RENDER_SIZE - 1,
	);
	const offset = (y * RENDER_SIZE + x) * 4;
	return (
		(buffer[offset] ?? 0) * 0.2126 +
		(buffer[offset + 1] ?? 0) * 0.7152 +
		(buffer[offset + 2] ?? 0) * 0.0722
	);
}

function percentile(sorted: readonly number[], value: number): number {
	if (sorted.length === 0) return 0;
	return sorted[Math.max(0, Math.ceil((value / 100) * sorted.length) - 1)] ?? 0;
}
