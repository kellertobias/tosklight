import * as THREE from "three";
import type { PatchedFixture } from "../api/types";

export type BuiltInStageAssetId =
  | "truss-0.5m" | "truss-1m" | "truss-2m" | "truss-3m"
  | "stage-2x1m"
  | "emissive-round" | "emissive-sphere" | "emissive-square" | "emissive-cube" | "emissive-tube";

export const BUILT_IN_STAGE_ASSETS: Array<{ id: BuiltInStageAssetId; name: string }> = [
  { id: "truss-0.5m", name: "4-point truss · 0.5 m" },
  { id: "truss-1m", name: "4-point truss · 1 m" },
  { id: "truss-2m", name: "4-point truss · 2 m" },
  { id: "truss-3m", name: "4-point truss · 3 m" },
  { id: "stage-2x1m", name: "Stage deck · 2 × 1 m" },
  { id: "emissive-round", name: "Emissive round surface" },
  { id: "emissive-sphere", name: "Emissive sphere" },
  { id: "emissive-square", name: "Emissive square" },
  { id: "emissive-cube", name: "Emissive cube" },
  { id: "emissive-tube", name: "Emissive tube" },
];

export type BuiltInFixtureKind = "wash-led" | "profile" | "wash-classic" | "par" | "fresnel" | "strobe" | "sunstrip";

const dark = () => new THREE.MeshStandardMaterial({ color: 0x20262b, roughness: .48, metalness: .55 });
const black = () => new THREE.MeshStandardMaterial({ color: 0x0d1012, roughness: .6, metalness: .35 });
const metal = () => new THREE.MeshStandardMaterial({ color: 0x626a70, roughness: .32, metalness: .85 });

function mesh(geometry: THREE.BufferGeometry, material: THREE.Material = dark()) {
  return new THREE.Mesh(geometry, material);
}

function boxFrame(width: number, height: number, depth: number, thickness = .035) {
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

function emissiveMaterial(color: THREE.Color, intensity: number) {
  return new THREE.MeshStandardMaterial({
    color: color.clone().multiplyScalar(.28 + intensity * .72),
    emissive: color,
    emissiveIntensity: .15 + intensity * 3.2,
    roughness: .24,
    metalness: 0,
  });
}

function lightSourceMaterial(color: THREE.Color, intensity: number) {
  // Real lenses read mostly white at output, with only a restrained hint of beam color.
  const level = THREE.MathUtils.clamp(intensity, 0, 1);
  const darkLens = new THREE.Color(0x080a0b);
  const litLens = color.clone().lerp(new THREE.Color(0xffffff), .82).multiplyScalar(2.98);
  const visible = darkLens.lerp(litLens, level);
  return new THREE.MeshBasicMaterial({
    color: visible,
    side: THREE.DoubleSide,
    toneMapped: false,
  });
}

function lightSource(geometry: THREE.BufferGeometry, color: THREE.Color, intensity: number) {
  const source = mesh(geometry, lightSourceMaterial(color, intensity));
  source.name = "light-emitting-surface";
  source.renderOrder = 20;
  return source;
}

export function inferBuiltInFixtureKind(fixture: PatchedFixture): BuiltInFixtureKind {
  const text = `${fixture.definition.device_type} ${fixture.definition.manufacturer} ${fixture.definition.name} ${fixture.definition.model}`.toLowerCase();
  if (/sun\s*strip|sunstrip|strip light|striplight/.test(text)) return "sunstrip";
  if (/strobe|blinder|panel/.test(text)) return "strobe";
  if (/fresnel|\bpc\b|theatre|theater/.test(text)) return "fresnel";
  if (/\bpar\b|parcan|par can/.test(text)) return "par";
  if (/profile|spot|beam/.test(text)) return "profile";
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
  return THREE.MathUtils.degToRad((normalizedTilt - .5) * MOVING_LIGHT_TILT_RANGE_DEGREES);
}

function movingBase(square: boolean) {
  return square
    ? mesh(new THREE.BoxGeometry(.5, .16, .42))
    : mesh(new THREE.CylinderGeometry(.27, .29, .16, 24));
}

function movingFixture(kind: "wash-led" | "profile" | "wash-classic", color: THREE.Color, intensity: number, pan: number, tilt: number): BuiltInFixtureModel {
  const object = new THREE.Group();
  const base = movingBase(kind !== "wash-led");
  object.add(base);
  const panGroup = new THREE.Group();
  panGroup.rotation.y = pan;
  object.add(panGroup);
  const yoke = boxFrame(.48, .48, .075, .055);
  yoke.position.y = -.31;
  panGroup.add(yoke);
  const tiltGroup = new THREE.Group();
  tiltGroup.position.y = -.48;
  // The head pivots on the axle joining the left and right yoke arms (local X),
  // not through the plane of either arm.
  tiltGroup.rotation.x = tilt;
  panGroup.add(tiltGroup);
  let head: THREE.Mesh;
  if (kind === "wash-led") {
    head = mesh(new THREE.CylinderGeometry(.21, .21, .3, 24));
    head.rotation.z = Math.PI / 2;
  } else {
    head = mesh(new THREE.SphereGeometry(.25, 24, 16));
    head.scale.set(kind === "profile" ? .82 : .96, 1.18, .82);
  }
  tiltGroup.add(head);
  // The luminous source is a filled central disc. The surrounding lens/barrel is
  // intentionally not emissive, avoiding the false illuminated-ring appearance.
  const apertureRadius = kind === "profile" ? .075 : .095;
  const aperture = lightSource(new THREE.CircleGeometry(apertureRadius, 32), color, intensity);
  aperture.rotation.x = -Math.PI / 2;
  aperture.position.y = kind === "wash-led" ? -.218 : -.305;
  tiltGroup.add(aperture);
  const beamMount = new THREE.Group();
  beamMount.position.y = aperture.position.y - .01;
  tiltGroup.add(beamMount);
  return { object, beamMount };
}

function staticFixture(kind: "par" | "fresnel" | "strobe" | "sunstrip", color: THREE.Color, intensity: number): BuiltInFixtureModel {
  const object = new THREE.Group();
  const hanger = boxFrame(kind === "sunstrip" ? 1.25 : .55, .45, .055, .035);
  hanger.position.y = -.2;
  object.add(hanger);
  const body = new THREE.Group();
  body.position.y = -.42;
  object.add(body);
  let aperture: THREE.Mesh;
  if (kind === "par") {
    const can = mesh(new THREE.CylinderGeometry(.19, .28, .62, 24));
    can.rotation.z = Math.PI / 2;
    body.add(can);
    aperture = lightSource(new THREE.CircleGeometry(.185, 32), color, intensity);
    aperture.rotation.y = Math.PI / 2;
    aperture.position.x = .325;
  } else if (kind === "fresnel") {
    body.add(mesh(new THREE.BoxGeometry(.48, .42, .54)));
    const barrel = mesh(new THREE.CylinderGeometry(.23, .23, .18, 24));
    barrel.rotation.z = Math.PI / 2;
    barrel.position.x = .34;
    body.add(barrel);
    aperture = lightSource(new THREE.CircleGeometry(.21, 32), color, intensity);
    aperture.rotation.y = Math.PI / 2;
    aperture.position.x = .445;
    for (const [y, z, ry, rz] of [[.25, 0, 0, .2], [-.25, 0, 0, -.2], [0, .25, .2, 0], [0, -.25, -.2, 0]] as number[][]) {
      const shutter = mesh(new THREE.BoxGeometry(.04, .32, .42), black());
      shutter.position.set(.46, y, z);
      shutter.rotation.set(0, ry, rz);
      body.add(shutter);
    }
  } else if (kind === "strobe") {
    body.add(mesh(new THREE.BoxGeometry(.95, .48, .16)));
    aperture = lightSource(new THREE.PlaneGeometry(.82, .36), color, intensity);
    aperture.rotation.y = Math.PI / 2;
    aperture.position.x = .481;
  } else {
    body.add(mesh(new THREE.BoxGeometry(.16, .22, 1.45)));
    aperture = new THREE.Mesh(new THREE.BufferGeometry(), lightSourceMaterial(color, intensity));
    for (let index = 0; index < 10; index++) {
      const lamp = lightSource(new THREE.CircleGeometry(.052, 20), color, intensity);
      lamp.name = `light-emitting-surface cell-${index + 1}`;
      lamp.rotation.y = Math.PI / 2;
      lamp.position.set(.086, 0, -.61 + index * .136);
      body.add(lamp);
    }
  }
  body.add(aperture);
  const beamMount = new THREE.Group();
  beamMount.rotation.z = -Math.PI / 2;
  beamMount.position.set(kind === "fresnel" ? .45 : kind === "strobe" ? .49 : kind === "sunstrip" ? .1 : .33, 0, 0);
  body.add(beamMount);
  return { object, beamMount };
}

export function createBuiltInFixtureModel(fixture: PatchedFixture, color: THREE.Color, intensity: number, pan: number, tilt: number): BuiltInFixtureModel {
  const kind = inferBuiltInFixtureKind(fixture);
  return kind === "wash-led" || kind === "profile" || kind === "wash-classic"
    ? movingFixture(kind, color, intensity, pan, tilt)
    : staticFixture(kind, color, intensity);
}

function truss(length: number) {
  const group = new THREE.Group();
  const material = metal();
  for (const y of [-.145, .145]) for (const z of [-.145, .145]) {
    const chord = mesh(new THREE.CylinderGeometry(.018, .018, length, 10), material);
    chord.rotation.z = Math.PI / 2;
    chord.position.set(0, y, z);
    group.add(chord);
  }
  const bays = Math.max(1, Math.round(length / .25));
  for (let bay = 0; bay <= bays; bay++) {
    const x = -length / 2 + (bay / bays) * length;
    for (const z of [-.145, .145]) {
      const rung = mesh(new THREE.CylinderGeometry(.012, .012, .29, 8), material);
      rung.position.set(x, 0, z);
      group.add(rung);
    }
    for (const y of [-.145, .145]) {
      const rung = mesh(new THREE.CylinderGeometry(.012, .012, .29, 8), material);
      rung.rotation.x = Math.PI / 2;
      rung.position.set(x, y, 0);
      group.add(rung);
    }
    if (bay < bays) for (const z of [-.145, .145]) {
      const braceLength = Math.hypot(length / bays, .29);
      const brace = mesh(new THREE.CylinderGeometry(.009, .009, braceLength, 8), material);
      brace.rotation.z = Math.PI / 2 - Math.atan2(.29, length / bays);
      brace.position.set(x + length / bays / 2, 0, z);
      group.add(brace);
    }
  }
  return group;
}

export function createBuiltInStageAsset(id: BuiltInStageAssetId) {
  const trussLength = id.startsWith("truss-") ? Number(id.slice(6, -1)) : null;
  if (trussLength) return truss(trussLength);
  if (id === "stage-2x1m") {
    const group = new THREE.Group();
    const deck = mesh(new THREE.BoxGeometry(2, .16, 1), new THREE.MeshStandardMaterial({ color: 0x34383b, roughness: .9 }));
    for (const x of [-.92, .92]) for (const z of [-.42, .42]) {
      const leg = mesh(new THREE.BoxGeometry(.055, .42, .055), metal());
      leg.position.set(x, -.28, z);
      group.add(leg);
    }
    group.add(deck);
    return group;
  }
  const glow = new THREE.Color(0x55dfff);
  const material = emissiveMaterial(glow, 1);
  if (id === "emissive-round") return mesh(new THREE.CircleGeometry(.32, 32), material);
  if (id === "emissive-sphere") return mesh(new THREE.SphereGeometry(.3, 24, 16), material);
  if (id === "emissive-square") return mesh(new THREE.PlaneGeometry(.6, .6), material);
  if (id === "emissive-tube") return mesh(new THREE.CylinderGeometry(.06, .06, 1.2, 20), material);
  return mesh(new THREE.BoxGeometry(.55, .55, .55), material);
}
