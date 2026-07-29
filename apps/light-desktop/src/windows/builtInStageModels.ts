import * as THREE from "three";
import type { PatchedFixture } from "../api/types";

export type BuiltInFixtureKind =
	| "wash-led"
	| "profile"
	| "profile-static"
	| "wash-classic"
	| "mirror-scanner"
	| "par"
	| "fresnel"
	| "strobe"
	| "sunstrip";

const dark = () =>
	new THREE.MeshStandardMaterial({
		color: 0x20262b,
		roughness: 0.48,
		metalness: 0.55,
	});
const black = () =>
	new THREE.MeshStandardMaterial({
		color: 0x0d1012,
		roughness: 0.6,
		metalness: 0.35,
	});
const metal = () =>
	new THREE.MeshStandardMaterial({
		color: 0x626a70,
		roughness: 0.32,
		metalness: 0.85,
	});

function mesh(
	geometry: THREE.BufferGeometry,
	material: THREE.Material = dark(),
) {
	return new THREE.Mesh(geometry, material);
}

function boxFrame(
	width: number,
	height: number,
	depth: number,
	thickness = 0.035,
) {
	const group = new THREE.Group();
	for (const x of [-width / 2, width / 2]) {
		const side = mesh(new THREE.BoxGeometry(thickness, height, depth));
		side.position.x = x;
		group.add(side);
	}
	const top = mesh(new THREE.BoxGeometry(width, thickness, depth));
	top.position.y = -height / 2;
	group.add(top);
	return group;
}

/** A complete square accessory frame in the Y/Z plane at the front of a static lantern. */
function frontFrame(size: number, thickness = 0.035, depth = 0.035) {
	const group = new THREE.Group();
	for (const y of [-size / 2, size / 2]) {
		const rail = mesh(
			new THREE.BoxGeometry(depth, thickness, size + thickness),
		);
		rail.position.y = y;
		group.add(rail);
	}
	for (const z of [-size / 2, size / 2]) {
		const rail = mesh(
			new THREE.BoxGeometry(depth, size + thickness, thickness),
		);
		rail.position.z = z;
		group.add(rail);
	}
	return group;
}

function lightSourceMaterial(color: THREE.Color, intensity: number) {
	// Real lenses read mostly white at output, with only a restrained hint of beam color.
	const level = THREE.MathUtils.clamp(intensity, 0, 1);
	// An inactive lens still catches neutral environment light, so broad wash and
	// blinder faces remain readable without looking powered.
	const darkLens = new THREE.Color(0x485158);
	const litLens = color
		.clone()
		.lerp(new THREE.Color(0xffffff), 0.82)
		.multiplyScalar(2.98);
	const visible = darkLens.lerp(litLens, level);
	return new THREE.MeshBasicMaterial({
		color: visible,
		side: THREE.DoubleSide,
		toneMapped: false,
	});
}

function lightSource(
	geometry: THREE.BufferGeometry,
	color: THREE.Color,
	intensity: number,
) {
	const source = mesh(geometry, lightSourceMaterial(color, intensity));
	source.name = "light-emitting-surface";
	source.renderOrder = 20;
	return source;
}

export function inferBuiltInFixtureKind(
	fixture: PatchedFixture,
): BuiltInFixtureKind {
	const text =
		`${fixture.definition.device_type} ${fixture.definition.manufacturer} ${fixture.definition.name} ${fixture.definition.model}`.toLowerCase();
	if (/mirror mover|moving mirror|mirror scanner|\bscanner\b/.test(text))
		return "mirror-scanner";
	if (/sun\s*strip|sunstrip|strip light|striplight/.test(text))
		return "sunstrip";
	if (/strobe|blinder|panel/.test(text)) return "strobe";
	if (/fresnel|\bpc\b|theatre|theater/.test(text)) return "fresnel";
	if (/\bpar\b|parcan|par can/.test(text)) return "par";
	if (/moving/.test(text) && /profile|spot|beam/.test(text)) return "profile";
	if (/profile|ellipsoidal|source four/.test(text)) return "profile-static";
	if (/a7|led.*wash|wash.*led/.test(text)) return "wash-led";
	if (/wash/.test(text)) return "wash-classic";
	if (/moving/.test(text)) return "profile";
	return "par";
}

export interface BuiltInFixtureModel {
	object: THREE.Group;
	beamMount: THREE.Object3D;
}

export const MOVING_LIGHT_TILT_RANGE_DEGREES = 320;

/** Maps normalized DMX tilt around the physical axle between the yoke arms. */
export function movingLightTiltRadians(normalizedTilt: number) {
	return THREE.MathUtils.degToRad(
		(normalizedTilt - 0.5) * MOVING_LIGHT_TILT_RANGE_DEGREES,
	);
}

function movingBase() {
	const base = mesh(new THREE.BoxGeometry(0.46, 0.16, 0.46));
	base.name = "fixed-square-base";
	return base;
}

function movingFixture(
	kind: "wash-led" | "profile" | "wash-classic",
	color: THREE.Color,
	intensity: number,
	pan: number,
	tilt: number,
): BuiltInFixtureModel {
	const object = new THREE.Group();
	const base = movingBase();
	object.add(base);
	const panGroup = new THREE.Group();
	panGroup.name = "centered-rotating-yoke";
	panGroup.rotation.y = pan;
	object.add(panGroup);
	const yoke = boxFrame(0.48, 0.48, 0.075, 0.055);
	yoke.name = "moving-yoke";
	yoke.position.y = -0.31;
	panGroup.add(yoke);
	const tiltGroup = new THREE.Group();
	tiltGroup.name = "tilting-head";
	tiltGroup.position.y = -0.48;
	// The head pivots on the axle joining the left and right yoke arms (local X),
	// not through the plane of either arm.
	tiltGroup.rotation.x = tilt;
	panGroup.add(tiltGroup);
	let head: THREE.Mesh;
	if (kind === "wash-led") {
		head = mesh(new THREE.CylinderGeometry(0.21, 0.21, 0.3, 24));
	} else {
		head = mesh(new THREE.SphereGeometry(0.25, 24, 16));
		head.scale.set(kind === "profile" ? 0.82 : 0.96, 1.18, 0.82);
	}
	head.name = "moving-head-body";
	tiltGroup.add(head);
	// The luminous source is a filled central disc. The surrounding lens/barrel is
	// intentionally not emissive, avoiding the false illuminated-ring appearance.
	const apertureRadius = kind === "profile" ? 0.075 : 0.095;
	const aperture = lightSource(
		new THREE.CircleGeometry(apertureRadius, 32),
		color,
		intensity,
	);
	aperture.name = "light-emitting-surface";
	aperture.userData.fixturePart = "moving-front-lens";
	aperture.rotation.x = -Math.PI / 2;
	aperture.position.y = kind === "wash-led" ? -0.218 : -0.305;
	tiltGroup.add(aperture);
	const beamMount = new THREE.Group();
	beamMount.name = "moving-beam-mount";
	beamMount.position.copy(aperture.position);
	tiltGroup.add(beamMount);
	return { object, beamMount };
}

function mirrorScanner(
	color: THREE.Color,
	intensity: number,
	pan: number,
	tilt: number,
): BuiltInFixtureModel {
	const object = new THREE.Group();
	object.name = "mirror-scanner";
	// Trackspot-style scanner: a long, fixed optical chassis. Lamp, color and
	// gobo optics remain inside; only the exposed mirror assembly moves.
	const chassis = mesh(new THREE.BoxGeometry(0.3, 0.28, 0.9));
	chassis.name = "scanner-chassis";
	chassis.position.set(0, -0.17, 0.18);
	object.add(chassis);
	const rearCap = mesh(new THREE.BoxGeometry(0.32, 0.22, 0.08), black());
	rearCap.position.set(0, -0.17, 0.66);
	object.add(rearCap);
	const lensWell = mesh(
		new THREE.CylinderGeometry(0.105, 0.105, 0.025, 28),
		black(),
	);
	lensWell.position.set(0, -0.018, -0.2);
	object.add(lensWell);

	// Fixed lamp/optical train exits upward through the chassis opening.
	const source = lightSource(
		new THREE.CircleGeometry(0.07, 32),
		color,
		intensity,
	);
	source.rotation.x = -Math.PI / 2;
	source.position.set(0, -0.002, -0.2);
	object.add(source);

	// Two small brackets carry the exposed mirror immediately above the opening.
	for (const x of [-0.12, 0.12]) {
		const bracket = mesh(new THREE.BoxGeometry(0.025, 0.22, 0.035), metal());
		bracket.position.set(x, 0.085, -0.2);
		object.add(bracket);
	}
	const panGroup = new THREE.Group();
	panGroup.name = "scanner-pan";
	panGroup.position.set(0, 0.185, -0.2);
	panGroup.rotation.y = pan;
	object.add(panGroup);
	const tiltGroup = new THREE.Group();
	tiltGroup.name = "scanner-tilt";
	// A mirror turns half as far as its reflected ray. Keep the visible plate near
	// its characteristic 45-degree neutral angle.
	tiltGroup.rotation.x = Math.PI / 4 + tilt / 2;
	panGroup.add(tiltGroup);
	const mirrorFrame = mesh(new THREE.BoxGeometry(0.26, 0.018, 0.2), black());
	const mirror = mesh(
		new THREE.PlaneGeometry(0.21, 0.16),
		new THREE.MeshBasicMaterial({
			color: 0xdde8ed,
			side: THREE.DoubleSide,
			toneMapped: false,
		}),
	);
	mirror.name = "moving-mirror";
	mirror.rotation.x = -Math.PI / 2;
	mirror.position.y = -0.011;
	tiltGroup.add(mirrorFrame, mirror);

	const beamMount = new THREE.Group();
	beamMount.name = "scanner-beam-mount";
	beamMount.position.y = 0.018;
	// Neutral reflection exits horizontally; tilt and pan redirect only the ray.
	beamMount.rotation.x = Math.PI / 2 - tilt;
	panGroup.add(beamMount);
	return { object, beamMount };
}

function addFresnelBody(
	body: THREE.Group,
	color: THREE.Color,
	intensity: number,
) {
	const housing = mesh(new THREE.BoxGeometry(0.48, 0.42, 0.54));
	housing.name = "fresnel-housing";
	body.add(housing);
	const barrel = mesh(new THREE.CylinderGeometry(0.23, 0.23, 0.18, 24));
	barrel.rotation.z = Math.PI / 2;
	barrel.position.x = 0.34;
	body.add(barrel);
	const aperture = lightSource(
		new THREE.CircleGeometry(0.21, 32),
		color,
		intensity,
	);
	aperture.rotation.y = Math.PI / 2;
	aperture.position.x = 0.445;
	const barnDoors = [
		{
			name: "top",
			size: [0.025, 0.28, 0.48],
			position: [0.48, 0.34, 0],
			rotation: [0, 0, -0.34],
		},
		{
			name: "bottom",
			size: [0.025, 0.28, 0.48],
			position: [0.48, -0.34, 0],
			rotation: [0, 0, 0.34],
		},
		{
			name: "left",
			size: [0.025, 0.42, 0.28],
			position: [0.48, 0, 0.35],
			rotation: [0, 0.34, 0],
		},
		{
			name: "right",
			size: [0.025, 0.42, 0.28],
			position: [0.48, 0, -0.35],
			rotation: [0, -0.34, 0],
		},
	] as const;
	for (const door of barnDoors) {
		const flap = mesh(
			new THREE.BoxGeometry(door.size[0], door.size[1], door.size[2]),
			black(),
		);
		flap.name = `fresnel-barn-door-${door.name}`;
		flap.position.set(door.position[0], door.position[1], door.position[2]);
		flap.rotation.set(door.rotation[0], door.rotation[1], door.rotation[2]);
		body.add(flap);
	}
	return aperture;
}

function staticFixture(
	kind: "par" | "profile-static" | "fresnel" | "strobe" | "sunstrip",
	color: THREE.Color,
	intensity: number,
): BuiltInFixtureModel {
	const object = new THREE.Group();
	const hanger = boxFrame(
		kind === "sunstrip" ? 1.25 : 0.55,
		0.45,
		0.055,
		0.035,
	);
	hanger.position.y = -0.2;
	object.add(hanger);
	const body = new THREE.Group();
	body.position.y = -0.42;
	// Static fixture geometry is authored with its lens on +X. Rotate the whole
	// lantern so the physical front follows Stage's canonical local -Y beam axis.
	body.rotation.z = -Math.PI / 2;
	object.add(body);
	let aperture: THREE.Mesh | null = null;
	if (kind === "par") {
		const can = mesh(new THREE.CylinderGeometry(0.19, 0.28, 0.62, 24));
		can.name = "par-can-body";
		can.rotation.z = Math.PI / 2;
		body.add(can);
		aperture = lightSource(
			new THREE.CircleGeometry(0.185, 32),
			color,
			intensity,
		);
		aperture.rotation.y = Math.PI / 2;
		aperture.position.x = 0.325;
		const gelFrame = frontFrame(0.47, 0.035, 0.03);
		gelFrame.name = "par-gel-frame";
		gelFrame.position.x = 0.345;
		body.add(gelFrame);
	} else if (kind === "profile-static") {
		const rear = mesh(new THREE.CylinderGeometry(0.17, 0.23, 0.42, 24));
		rear.name = "profile-rear-housing";
		rear.rotation.z = Math.PI / 2;
		rear.position.x = -0.18;
		body.add(rear);
		const gate = mesh(new THREE.BoxGeometry(0.16, 0.34, 0.38));
		gate.name = "profile-shutter-gate";
		gate.position.x = 0.1;
		body.add(gate);
		const barrel = mesh(new THREE.CylinderGeometry(0.105, 0.15, 0.52, 24));
		barrel.name = "profile-lens-barrel";
		barrel.rotation.z = Math.PI / 2;
		barrel.position.x = 0.44;
		body.add(barrel);
		const lensRing = mesh(
			new THREE.CylinderGeometry(0.155, 0.155, 0.045, 24),
			black(),
		);
		lensRing.rotation.z = Math.PI / 2;
		lensRing.position.x = 0.72;
		body.add(lensRing);
		const handle = mesh(new THREE.BoxGeometry(0.32, 0.035, 0.04), metal());
		handle.name = "profile-top-handle";
		handle.position.set(-0.03, -0.23, 0);
		body.add(handle);
		aperture = lightSource(
			new THREE.CircleGeometry(0.13, 32),
			color,
			intensity,
		);
		aperture.rotation.y = Math.PI / 2;
		aperture.position.x = 0.745;
	} else if (kind === "fresnel") {
		aperture = addFresnelBody(body, color, intensity);
	} else if (kind === "strobe") {
		body.add(mesh(new THREE.BoxGeometry(0.95, 0.48, 0.16)));
		aperture = lightSource(
			new THREE.PlaneGeometry(0.82, 0.36),
			color,
			intensity,
		);
		aperture.rotation.y = Math.PI / 2;
		aperture.position.x = 0.481;
	} else {
		body.add(mesh(new THREE.BoxGeometry(0.16, 0.22, 1.45)));
		for (let index = 0; index < 10; index++) {
			const lamp = lightSource(
				new THREE.CircleGeometry(0.052, 20),
				color,
				intensity,
			);
			lamp.name = `light-emitting-surface cell-${index + 1}`;
			lamp.rotation.y = Math.PI / 2;
			lamp.position.set(0.086, 0, -0.61 + index * 0.136);
			body.add(lamp);
		}
	}
	if (aperture) body.add(aperture);
	const beamMount = new THREE.Group();
	beamMount.rotation.z = Math.PI / 2;
	beamMount.position.set(
		kind === "profile-static"
			? 0.75
			: kind === "fresnel"
				? 0.45
				: kind === "strobe"
					? 0.49
					: kind === "sunstrip"
						? 0.1
						: 0.33,
		0,
		0,
	);
	body.add(beamMount);
	return { object, beamMount };
}

export function createBuiltInFixtureModel(
	fixture: PatchedFixture,
	color: THREE.Color,
	intensity: number,
	pan: number,
	tilt: number,
): BuiltInFixtureModel {
	const kind = inferBuiltInFixtureKind(fixture);
	if (kind === "mirror-scanner")
		return mirrorScanner(color, intensity, pan, tilt);
	return kind === "wash-led" || kind === "profile" || kind === "wash-classic"
		? movingFixture(kind, color, intensity, pan, tilt)
		: staticFixture(kind, color, intensity);
}
