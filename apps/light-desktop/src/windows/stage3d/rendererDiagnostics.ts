import type * as THREE from "three";

export function rendererCapabilities(renderer: THREE.WebGLRenderer) {
	const context = renderer.getContext();
	const debug = context.getExtension("WEBGL_debug_renderer_info");
	return {
		isWebGL2: renderer.capabilities.isWebGL2,
		precision: renderer.capabilities.precision,
		maxTextures: renderer.capabilities.maxTextures,
		maxTextureSize: context.getParameter(context.MAX_TEXTURE_SIZE) as number,
		maxRenderbufferSize: context.getParameter(
			context.MAX_RENDERBUFFER_SIZE,
		) as number,
		renderer: debug
			? (context.getParameter(debug.UNMASKED_RENDERER_WEBGL) as string)
			: null,
		vendor: debug
			? (context.getParameter(debug.UNMASKED_VENDOR_WEBGL) as string)
			: null,
	};
}

export function transparentStageDrawCalls(scene: THREE.Scene) {
	let calls = 0;
	scene.traverseVisible((object) => {
		const renderable = object as THREE.Mesh;
		if (!renderable.geometry || !renderable.material) return;
		const materials = Array.isArray(renderable.material)
			? renderable.material
			: [renderable.material];
		if (!Array.isArray(renderable.material)) {
			if (renderable.material.transparent) calls++;
			return;
		}
		const groups = renderable.geometry.groups;
		if (!groups.length) {
			calls += materials.filter((material) => material.transparent).length;
			return;
		}
		for (const group of groups)
			if (materials[group.materialIndex ?? 0]?.transparent) calls++;
	});
	return calls;
}

export function visibleStageObjects(scene: THREE.Scene) {
	const counts = {
		beamVolumes: 0,
		improvedBeamVolumes: 0,
		improvedBeamLights: 0,
		centerLines: 0,
		groundFootprints: 0,
		directionGuides: 0,
		selectionOutlines: 0,
	};
	scene.traverseVisible((object) => {
		switch (object.name) {
			case "beam-volume":
				counts.beamVolumes++;
				break;
			case "beam-improved-volume":
				counts.improvedBeamVolumes++;
				break;
			case "stage-improved-spotlight":
				counts.improvedBeamLights++;
				break;
			case "beam-centerline":
				counts.centerLines++;
				break;
			case "beam-ground-footprint":
				counts.groundFootprints++;
				break;
			case "beam-direction-guide":
				counts.directionGuides++;
				break;
			case "selection-outline":
				counts.selectionOutlines++;
				break;
		}
	});
	return counts;
}
