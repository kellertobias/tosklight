import * as THREE from "three";
import type { AttributeValue, PatchedFixture, VisualizationSnapshot } from "../api/types";
import type { StagePosition3d } from "../api/ServerContext";

export interface Stage3dFixture {
  fixture: PatchedFixture;
  position: StagePosition3d;
  index: number;
}

function normalized(value: AttributeValue | undefined, fallback: number) {
  return value?.kind === "normalized" ? value.value : fallback;
}

function discrete(value: AttributeValue | undefined) {
  return value?.kind === "discrete" ? value.value : null;
}

function parameterDefault(fixture: PatchedFixture, attribute: string, fallback: number) {
  return fixture.definition.heads?.flatMap((head) => head.parameters).find((parameter) => parameter.attribute === attribute)?.default ?? fallback;
}

function capabilityName(fixture: PatchedFixture, attribute: string, value: AttributeValue | undefined) {
  const named = discrete(value);
  if (named) return named;
  if (value?.kind !== "normalized") return null;
  const raw = Math.round(value.value * 255);
  return fixture.definition.heads?.flatMap((head) => head.parameters).find((parameter) => parameter.attribute === attribute)?.capabilities?.find((capability) => raw >= capability.dmx_from && raw <= capability.dmx_to)?.name ?? null;
}

function xyzToColor(value: AttributeValue | undefined, attributes: Map<string, AttributeValue>) {
  if (value?.kind === "color_xyz") {
    const { x, y, z } = value.value;
    const linear = [3.2406 * x - 1.5372 * y - .4986 * z, -.9689 * x + 1.8758 * y + .0415 * z, .0557 * x - .204 * y + 1.057 * z];
    const gamma = (channel: number) => channel <= .0031308 ? 12.92 * channel : 1.055 * Math.pow(channel, 1 / 2.4) - .055;
    return new THREE.Color(gamma(linear[0]), gamma(linear[1]), gamma(linear[2]));
  }
  return new THREE.Color(
    normalized(attributes.get("color.red"), 1),
    normalized(attributes.get("color.green"), 1),
    normalized(attributes.get("color.blue"), 1),
  );
}

export function defaultStagePosition(index: number): StagePosition3d {
  return { x: -5.25 + (index % 8) * 1.5, y: 1 + Math.floor(index / 8) * 1.6, z: 5, rotationX: 0, rotationY: 0, rotationZ: 0 };
}

export function migrateStagePosition(position: { x: number; y: number; rotation: number } | undefined, index: number): StagePosition3d {
  if (!position) return defaultStagePosition(index);
  return { x: (position.x / 100 - .5) * 12, y: position.y / 100 * 8, z: 5, rotationX: 0, rotationY: 0, rotationZ: position.rotation };
}

function valuesByFixture(snapshot: VisualizationSnapshot | null) {
  const result = new Map<string, Map<string, AttributeValue>>();
  for (const entry of snapshot?.values ?? []) {
    const attributes = result.get(entry.fixture_id) ?? new Map<string, AttributeValue>();
    attributes.set(entry.attribute, entry.value);
    result.set(entry.fixture_id, attributes);
  }
  return result;
}

function fixtureBody(selected: boolean) {
  const group = new THREE.Group();
  group.name = "fixture-placeholder";
  const dark = new THREE.MeshStandardMaterial({ color: selected ? 0x136f80 : 0x252c33, roughness: .55, metalness: .35 });
  const base = new THREE.Mesh(new THREE.CylinderGeometry(.22, .27, .18, 16), dark);
  const yoke = new THREE.Mesh(new THREE.BoxGeometry(.46, .42, .12), dark);
  yoke.position.y = -.25;
  const head = new THREE.Mesh(new THREE.CylinderGeometry(.2, .24, .42, 16), dark);
  head.rotation.z = Math.PI / 2;
  head.position.y = -.52;
  group.add(base, yoke, head);
  if (selected) for (const mesh of [base, yoke, head]) {
    const outline = new THREE.LineSegments(
      new THREE.EdgesGeometry(mesh.geometry),
      new THREE.LineBasicMaterial({ color: 0x378eff }),
    );
    outline.position.copy(mesh.position);
    outline.rotation.copy(mesh.rotation);
    outline.scale.setScalar(1.035);
    outline.name = "selection-outline";
    group.add(outline);
  }
  return group;
}

export function buildStageScene(
  fixtures: Stage3dFixture[],
  snapshot: VisualizationSnapshot | null,
  selected: Set<string> = new Set(),
) {
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x080b0f);
  scene.add(new THREE.HemisphereLight(0xa9c8dc, 0x11151a, 1.5));
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(12, 8), new THREE.MeshStandardMaterial({ color: 0x151b20, roughness: .9 }));
  floor.rotation.x = -Math.PI / 2;
  floor.position.set(0, 0, -4);
  scene.add(floor);
  const grid = new THREE.GridHelper(12, 24, 0x24798a, 0x263039);
  grid.position.z = -4;
  scene.add(grid);
  const byFixture = valuesByFixture(snapshot);
  const fixtureObjects = new Map<string, THREE.Object3D>();

  for (const item of fixtures) {
    const id = item.fixture.fixture_id;
    const attributes = byFixture.get(id) ?? new Map<string, AttributeValue>();
    const root = new THREE.Group();
    root.name = `fixture:${id}`;
    root.userData.fixtureId = id;
    root.position.set(item.position.x, item.position.z, -item.position.y);
    root.rotation.set(
      THREE.MathUtils.degToRad(item.position.rotationX),
      THREE.MathUtils.degToRad(item.position.rotationZ),
      THREE.MathUtils.degToRad(item.position.rotationY),
    );
    root.add(fixtureBody(selected.has(id)));

    const intensity = (snapshot?.blackout ? 0 : normalized(attributes.get("intensity"), parameterDefault(item.fixture, "intensity", 0))) * (snapshot?.grand_master ?? 1);
    const pan = (normalized(attributes.get("pan"), parameterDefault(item.fixture, "pan", .5)) - .5) * Math.PI * 2;
    const tilt = normalized(attributes.get("tilt"), parameterDefault(item.fixture, "tilt", .2)) * Math.PI * .92;
    const zoom = normalized(attributes.get("zoom"), parameterDefault(item.fixture, "zoom", .35));
    const focus = normalized(attributes.get("focus"), parameterDefault(item.fixture, "focus", .65));
    const color = xyzToColor(attributes.get("color"), attributes);
    const distance = 7;
    const radius = Math.tan(THREE.MathUtils.degToRad(4 + zoom * 23)) * distance;
    const direction = new THREE.Vector3(Math.sin(pan) * Math.sin(tilt), -Math.cos(tilt), Math.cos(pan) * Math.sin(tilt)).normalize();
    const beam = new THREE.Group();
    beam.position.y = -.62;
    beam.quaternion.setFromUnitVectors(new THREE.Vector3(0, -1, 0), direction);
    const coneGeometry = new THREE.ConeGeometry(radius, distance, 32, 1, true);
    coneGeometry.translate(0, -distance / 2, 0);
    const volume = new THREE.Mesh(coneGeometry, new THREE.MeshBasicMaterial({ color, transparent: true, opacity: intensity * (.035 + focus * .055), side: THREE.DoubleSide, depthWrite: false, blending: THREE.AdditiveBlending }));
    const outline = new THREE.LineSegments(new THREE.EdgesGeometry(coneGeometry, 28), new THREE.LineBasicMaterial({ color, transparent: true, opacity: .28 + intensity * .55 }));
    const centerGeometry = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3(0, -distance, 0)]);
    const center = new THREE.Line(centerGeometry, new THREE.LineBasicMaterial({ color, transparent: true, opacity: .55 }));
    beam.add(volume, outline, center);
    const gobo = capabilityName(item.fixture, "gobo", attributes.get("gobo"));
    if (gobo && gobo.toLowerCase() !== "open") {
      for (let spoke = 0; spoke < 6; spoke++) {
        const angle = spoke / 6 * Math.PI * 2;
        const line = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3(Math.cos(angle) * radius, -distance, Math.sin(angle) * radius)]);
        beam.add(new THREE.Line(line, new THREE.LineBasicMaterial({ color, transparent: true, opacity: intensity * .45 })));
      }
    }
    root.add(beam);
    scene.add(root);
    fixtureObjects.set(id, root);
  }
  return { scene, fixtureObjects };
}

export function disposeScene(scene: THREE.Scene) {
  scene.traverse((object) => {
    const mesh = object as THREE.Mesh;
    mesh.geometry?.dispose();
    const materials = Array.isArray(mesh.material) ? mesh.material : mesh.material ? [mesh.material] : [];
    for (const material of materials) material.dispose();
  });
}

export function cueVisualization(base: VisualizationSnapshot | null, changes: Array<{ fixture_id: string; attribute: string; value: AttributeValue | null }>) {
  const entries = new Map((base?.values ?? []).map((entry) => [`${entry.fixture_id}\0${entry.attribute}`, entry]));
  for (const change of changes) {
    const key = `${change.fixture_id}\0${change.attribute}`;
    if (change.value) entries.set(key, { ...change, value: change.value }); else entries.delete(key);
  }
  return { revision: base?.revision ?? 0, generated_at: new Date().toISOString(), grand_master: 1, blackout: false, values: [...entries.values()] } satisfies VisualizationSnapshot;
}

export function renderStageThumbnail(fixtures: Stage3dFixture[], snapshot: VisualizationSnapshot, width = 240, height = 135) {
  const { scene } = buildStageScene(fixtures, snapshot);
  const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true, alpha: false });
  renderer.setSize(width, height, false);
  renderer.setPixelRatio(1);
  const camera = new THREE.PerspectiveCamera(48, width / height, .1, 100);
  camera.position.set(10, 8, 11);
  camera.lookAt(0, 1.8, -4);
  renderer.render(scene, camera);
  const result = renderer.domElement.toDataURL("image/webp", .8);
  disposeScene(scene);
  renderer.forceContextLoss();
  renderer.dispose();
  return result;
}
