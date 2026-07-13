import { describe, expect, it } from "vitest";
import type { VisualizationSnapshot } from "../api/types";
import { cueVisualization, migrateStagePosition } from "./stage3dScene";
import { BUILT_IN_STAGE_ASSETS, createBuiltInStageAsset, createBuiltInFixtureModel, inferBuiltInFixtureKind, movingLightTiltRadians } from "./builtInStageModels";
import * as THREE from "three";
import type { PatchedFixture } from "../api/types";

describe("3D stage state", () => {
  it("migrates legacy percentage positions into the meter-based stage", () => {
    expect(migrateStagePosition({ x: 50, y: 25, rotation: 90 }, 0)).toEqual({
      x: 0, y: 2, z: 5, rotationX: 0, rotationY: 0, rotationZ: 90,
    });
  });

  it("tracks cue values and explicit releases for thumbnails", () => {
    const base: VisualizationSnapshot = { revision: 1, generated_at: "", grand_master: .5, blackout: true, values: [] };
    const first = cueVisualization(base, [{ fixture_id: "one", attribute: "intensity", value: { kind: "normalized", value: .8 } }]);
    expect(first.blackout).toBe(false);
    expect(first.grand_master).toBe(1);
    expect(first.values).toHaveLength(1);
    const released = cueVisualization(first, [{ fixture_id: "one", attribute: "intensity", value: null }]);
    expect(released.values).toHaveLength(0);
  });
});

describe("built-in 3D model library", () => {
  const fixture = (device_type: string, name: string) => ({
    fixture_id: "fixture", universe: 1, address: 1,
    definition: { device_type, name, manufacturer: "", model: name },
  }) as PatchedFixture;

  it("recognizes the requested fixture families", () => {
    expect(inferBuiltInFixtureKind(fixture("moving wash", "A7 LED Wash"))).toBe("wash-led");
    expect(inferBuiltInFixtureKind(fixture("moving profile", "Profile"))).toBe("profile");
    expect(inferBuiltInFixtureKind(fixture("wash", "Classic Wash"))).toBe("wash-classic");
    expect(inferBuiltInFixtureKind(fixture("conventional", "PAR Can"))).toBe("par");
    expect(inferBuiltInFixtureKind(fixture("conventional", "PC Fresnel"))).toBe("fresnel");
    expect(inferBuiltInFixtureKind(fixture("strobe", "Strobe"))).toBe("strobe");
    expect(inferBuiltInFixtureKind(fixture("strip light", "Sunstrip"))).toBe("sunstrip");
  });

  it("contains every requested truss length and emissive primitive", () => {
    expect(BUILT_IN_STAGE_ASSETS.map((asset) => asset.id)).toEqual(expect.arrayContaining([
      "truss-0.5m", "truss-1m", "truss-2m", "truss-3m", "stage-2x1m",
      "emissive-round", "emissive-sphere", "emissive-square", "emissive-cube", "emissive-tube",
    ]));
    for (const asset of BUILT_IN_STAGE_ASSETS) expect(createBuiltInStageAsset(asset.id)).toBeTruthy();
  });

  it("maps tilt symmetrically from minus 160 to plus 160 degrees", () => {
    expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(0))).toBeCloseTo(-160);
    expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(.5))).toBeCloseTo(0);
    expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(1))).toBeCloseTo(160);
  });

  it("tilts a moving head on the axle between the yoke arms", () => {
    const model = createBuiltInFixtureModel(fixture("moving profile", "Profile"), new THREE.Color("white"), 1, 0, movingLightTiltRadians(.75));
    const tiltGroup = model.beamMount.parent!;
    expect(tiltGroup.rotation.x).toBeCloseTo(THREE.MathUtils.degToRad(80));
    expect(tiltGroup.rotation.z).toBeCloseTo(0);
  });

  it("gives every fixture family a bright unlit emitting surface", () => {
    for (const [type, name] of [
      ["moving wash", "A7 LED Wash"], ["moving profile", "Profile"], ["wash", "Classic Wash"],
      ["scanner", "Mirror Mover"], ["conventional", "PAR Can"], ["conventional", "PC Fresnel"], ["strobe", "Strobe"], ["strip light", "Sunstrip"],
    ]) {
      const model = createBuiltInFixtureModel(fixture(type, name), new THREE.Color(0x55aaff), 1, 0, 0);
      const sources: THREE.Mesh[] = [];
      model.object.traverse((object) => { if (object instanceof THREE.Mesh && object.name.startsWith("light-emitting-surface")) sources.push(object); });
      expect(sources.length, name).toBeGreaterThan(0);
      expect(sources.every((source) => source.material instanceof THREE.MeshBasicMaterial), name).toBe(true);
    }
  });

  it("uses one filled central source for a wash mover instead of an LED ring", () => {
    const beamColor = new THREE.Color(0xff0000);
    const model = createBuiltInFixtureModel(fixture("moving wash", "A7 LED Wash"), beamColor, 1, 0, 0);
    const sources: THREE.Mesh[] = [];
    model.object.traverse((object) => { if (object instanceof THREE.Mesh && object.name === "light-emitting-surface") sources.push(object); });
    expect(sources).toHaveLength(1);
    expect(sources[0].geometry).toBeInstanceOf(THREE.CircleGeometry);
    const sourceColor = (sources[0].material as THREE.MeshBasicMaterial).color;
    expect(sourceColor.r).toBeGreaterThanOrEqual(sourceColor.g);
    expect(sourceColor.g).toBeGreaterThan(.7);
    expect(sourceColor.b).toBeGreaterThan(.7);
  });

  it("renders an off lens as nearly black neutral glass", () => {
    const model = createBuiltInFixtureModel(fixture("moving profile", "Profile"), new THREE.Color(0xff0000), 0, 0, 0);
    let source: THREE.Mesh | undefined;
    model.object.traverse((object) => { if (object instanceof THREE.Mesh && object.name === "light-emitting-surface") source = object; });
    const color = (source!.material as THREE.MeshBasicMaterial).color;
    expect(Math.max(color.r, color.g, color.b)).toBeLessThan(.05);
    expect(Math.max(color.r, color.g, color.b) - Math.min(color.r, color.g, color.b)).toBeLessThan(.02);
  });

  it("builds a scanner with a fixed source and animated 45-degree mirror", () => {
    const scanner = fixture("scanner", "Mirror Mover Scanner");
    expect(inferBuiltInFixtureKind(scanner)).toBe("mirror-scanner");
    const neutral = createBuiltInFixtureModel(scanner, new THREE.Color("white"), 1, 0, 0);
    const mirror = neutral.object.getObjectByName("moving-mirror")!;
    const chassis = neutral.object.getObjectByName("scanner-chassis") as THREE.Mesh;
    const chassisSize = new THREE.Box3().setFromObject(chassis).getSize(new THREE.Vector3());
    expect(chassisSize.z / chassisSize.x).toBeCloseTo(3);
    expect(mirror.parent!.rotation.x).toBeCloseTo(Math.PI / 4);
    const moved = createBuiltInFixtureModel(scanner, new THREE.Color("white"), 1, .4, movingLightTiltRadians(.75));
    const movedMirror = moved.object.getObjectByName("moving-mirror")!;
    expect(movedMirror.parent!.rotation.x).not.toBeCloseTo(Math.PI / 4);
    expect(moved.beamMount.parent!.rotation.y).toBeCloseTo(.4);
  });
});
