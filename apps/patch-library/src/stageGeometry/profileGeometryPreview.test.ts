import * as THREE from "three";
import { describe, expect, it } from "vitest";
import { blankMode, geometryTemplate } from "../sheet/fixtureProfileModel";
import type { FixtureMode } from "../wire";
import {
	buildFixtureProfileGeometryPreview,
	visibleGeometryBounds,
} from "./profileGeometry";

function movingHead(): FixtureMode {
	const mode = blankMode();
	const head = mode.heads[0]?.id ?? "head";
	return {
		...mode,
		channels: [],
		geometry: geometryTemplate("moving_head", [head]),
	} as FixtureMode;
}

/** Whether the object and every parent up to the root is drawn. */
function drawn(object: THREE.Object3D, root: THREE.Object3D) {
	for (let current: THREE.Object3D | null = object; current; current = current.parent) {
		if (!current.visible) return false;
		if (current === root) return true;
	}
	return true;
}

describe("the fixture-library geometry preview", () => {
	it("draws the lamp's parts and nothing that leaves the lens", () => {
		const lamp = buildFixtureProfileGeometryPreview(movingHead());
		const shown: string[] = [];
		lamp.traverse((object) => {
			if (drawn(object, lamp)) shown.push(object.name);
		});
		expect(shown.filter((name) => name.startsWith("geometry-part:"))).toHaveLength(3);
		for (const beamPart of [
			"beam-volume",
			"beam-core",
			"beam-direction-guide",
		])
			expect(shown, `${beamPart} is hidden`).not.toContain(beamPart);
		const lines: THREE.Object3D[] = [];
		lamp.traverse((object) => {
			if (object instanceof THREE.Line && drawn(object, lamp)) lines.push(object);
		});
		expect(lines, "no beam line is drawn").toEqual([]);
	});

	it("bounds only what it draws, so the camera frames the lamp and not the throw", () => {
		const lamp = buildFixtureProfileGeometryPreview(movingHead());
		const size = visibleGeometryBounds(lamp).getSize(new THREE.Vector3());
		// A 160 mm default body, not a beam metres long.
		expect(size.length()).toBeGreaterThan(0.1);
		expect(size.length()).toBeLessThan(1);
		const everything = new THREE.Box3().setFromObject(lamp);
		expect(everything.getSize(new THREE.Vector3()).length()).toBeGreaterThan(
			size.length(),
		);
	});

	it("has nothing to bound without parts", () => {
		const lamp = buildFixtureProfileGeometryPreview({
			...movingHead(),
			geometry: { nodes: [], emitters: [] },
		});
		expect(visibleGeometryBounds(lamp).isEmpty()).toBe(true);
	});
});


it("composes motion around its authored axis after the compound neutral rotation", () => {
    const mode = movingHead();
    const node = mode.geometry.nodes.find(n => n.motion?.kind === "rotation")!;
    node.transform.rotation_degrees = { x: 23, y: -38, z: 71 };
    node.motion!.axis = { x: 1, y: 2, z: 3 };
    node.motion!.physical_min = 64;
    node.motion!.physical_max = 64;
    const lamp = buildFixtureProfileGeometryPreview(mode);
    const actual = lamp.getObjectByName(`geometry-node:${node.id}`)!.quaternion;
    const neutral = new THREE.Quaternion().setFromEuler(new THREE.Euler(23*Math.PI/180, -38*Math.PI/180, 71*Math.PI/180));
    const expected = neutral.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1,2,3).normalize(), 64*Math.PI/180));
    for (const basis of [new THREE.Vector3(1,0,0), new THREE.Vector3(0,1,0), new THREE.Vector3(0,0,1)]) {
        expect(basis.clone().applyQuaternion(actual).distanceTo(basis.clone().applyQuaternion(expected))).toBeLessThan(1e-10);
    }
});
