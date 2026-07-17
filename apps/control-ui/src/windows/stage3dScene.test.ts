import { describe, expect, it } from "vitest";
import type { VisualizationSnapshot } from "../api/types";
import { buildStageScene, cueVisualization, migrateStagePosition, mountFixtureModel } from "./stage3dScene";
import { BUILT_IN_STAGE_ASSETS, createBuiltInStageAsset, createBuiltInFixtureModel, inferBuiltInFixtureKind, movingLightTiltRadians } from "./builtInStageModels";
import * as THREE from "three";
import type { PatchedFixture } from "../api/types";
import { blankChannel, blankFixtureProfile, blankHead, fixtureDefinitionFromProfileMode, geometryTemplate } from "../components/setup/fixtureProfileModel";

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

  it("consumes schema-v2 hierarchy motion, logical-head values, multiple emitters, and source layouts", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Acme";
    profile.name = "Twin Beam";
    profile.revision = 1;
    const mode = profile.modes[0];
    const second = { ...blankHead(1, 1), master_shared: false };
    mode.heads.push(second);
    mode.channels = [
      { ...blankChannel(mode), head_id: mode.heads[0].id, attribute: "intensity" },
      { ...blankChannel(mode), head_id: second.id, attribute: "intensity" },
    ];
    mode.geometry = geometryTemplate("shared_pan_multi_head", mode.heads.map((head) => head.id));
    mode.geometry.emitters[0].layout = { type: "matrix", columns: 2, rows: 2, spacing: { x: 40, y: 40, z: 0 } };
    mode.geometry.emitters[0].feather = .35;
    mode.geometry.emitters[0].focus = .7;
    const definition = fixtureDefinitionFromProfileMode(profile, mode);
    const fixture = {
      fixture_id: profile.id,
      universe: 1,
      address: 1,
      definition,
      logical_heads: [{ head_index: 1, fixture_id: "head-two" }],
    } as PatchedFixture;
    const snapshot: VisualizationSnapshot = {
      revision: 1,
      generated_at: "",
      grand_master: 1,
      blackout: false,
      values: [
        { fixture_id: profile.id, attribute: "pan", value: { kind: "normalized", value: .75 } },
        { fixture_id: profile.id, attribute: "tilt", value: { kind: "normalized", value: .25 } },
        { fixture_id: profile.id, attribute: "intensity", value: { kind: "normalized", value: .4 } },
        { fixture_id: profile.id, attribute: "beam.focus", value: { kind: "normalized", value: .2 } },
        { fixture_id: profile.id, attribute: "beam.zoom", value: { kind: "normalized", value: .75 } },
        { fixture_id: "head-two", attribute: "tilt", value: { kind: "normalized", value: .75 } },
        { fixture_id: "head-two", attribute: "intensity", value: { kind: "normalized", value: .8 } },
      ],
    };
    const { scene } = buildStageScene([{ fixture, index: 0, position: { x: 0, y: 0, z: 3, rotationX: 0, rotationY: 0, rotationZ: 0 } }], snapshot);
    const pan = mode.geometry.nodes.find((node) => node.motion?.attribute === "pan")!;
    const tilts = mode.geometry.nodes.filter((node) => node.motion?.attribute === "tilt");
    expect(scene.getObjectByName(`geometry-node:${pan.id}`)?.rotation.y).toBeCloseTo(THREE.MathUtils.degToRad(135));
    expect(scene.getObjectByName(`geometry-node:${tilts[0].id}`)?.rotation.x).toBeCloseTo(THREE.MathUtils.degToRad(-67.5));
    expect(scene.getObjectByName(`geometry-node:${tilts[1].id}`)?.rotation.x).toBeCloseTo(THREE.MathUtils.degToRad(67.5));
    const sources: THREE.Object3D[] = [];
    scene.traverse((object) => { if (object.name.startsWith("geometry-source:")) sources.push(object); });
    expect(sources).toHaveLength(5);
    expect(sources.filter((source) => source.userData.layout === "matrix")).toHaveLength(4);
    const emitter = scene.getObjectByName(`geometry-emitter:${mode.geometry.emitters[0].id}`)!;
    expect(emitter.userData.sourceCount).toBe(4);
    expect(emitter.userData.beamAngleDegrees).toBeLessThan(emitter.userData.fieldAngleDegrees);
    expect(emitter.userData.feather).toBe(.35);
    expect(emitter.userData.focus).toBe(.2);
    const cores: THREE.Object3D[] = [];
    scene.traverse((object) => { if (object.name === "beam-core") cores.push(object); });
    expect(cores).toHaveLength(5);
  });

  it("places point, ring, strip, matrix, and explicit-pixel beam sources", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Acme";
    profile.name = "Pixel Lamp";
    profile.revision = 1;
    const mode = profile.modes[0];
    mode.channels = [{ ...blankChannel(mode), attribute: "intensity" }];
    const nodeId = mode.geometry.nodes[0].id;
    const headId = mode.heads[0].id;
    const emitter = mode.geometry.emitters[0];
    mode.geometry.emitters = [
      { ...emitter, id: "point", node_id: nodeId, head_id: headId, layout: { type: "point" } },
      { ...emitter, id: "ring", node_id: nodeId, head_id: headId, layout: { type: "ring", count: 4, radius_millimetres: 100 } },
      { ...emitter, id: "strip", node_id: nodeId, head_id: headId, layout: { type: "strip", count: 3, spacing_millimetres: 50 } },
      { ...emitter, id: "matrix", node_id: nodeId, head_id: headId, layout: { type: "matrix", columns: 2, rows: 2, spacing: { x: 40, y: 30, z: 10 } } },
      { ...emitter, id: "pixels", node_id: nodeId, head_id: headId, layout: { type: "explicit_pixels", positions: [{ x: 0, y: 0, z: 0 }, { x: 100, y: 200, z: 300 }] } },
    ];
    const fixture = {
      fixture_id: profile.id,
      universe: 1,
      address: 1,
      definition: fixtureDefinitionFromProfileMode(profile, mode),
      logical_heads: [],
    } as PatchedFixture;
    const { scene } = buildStageScene([{
      fixture,
      index: 0,
      position: { x: 0, y: 0, z: 3, rotationX: 0, rotationY: 0, rotationZ: 0 },
    }], null);
    const sources: THREE.Object3D[] = [];
    scene.traverse((object) => { if (object.name.startsWith("geometry-source:")) sources.push(object); });

    expect(sources).toHaveLength(14);
    expect(Object.fromEntries(["point", "ring", "strip", "matrix", "explicit_pixels"].map((layout) => [
      layout,
      sources.filter((source) => source.userData.layout === layout).length,
    ]))).toEqual({ point: 1, ring: 4, strip: 3, matrix: 4, explicit_pixels: 2 });
    expect(scene.getObjectByName("geometry-source:pixels:1")?.position.toArray()).toEqual([.1, .2, .3]);
  });

  it("mounts named GLB parts on their profile geometry anchors", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Acme";
    profile.name = "Bound Mover";
    profile.revision = 1;
    const mode = profile.modes[0];
    mode.geometry = geometryTemplate("moving_head", [mode.heads[0].id]);
    const pan = mode.geometry.nodes.find((node) => node.motion?.attribute === "pan")!;
    const tilt = mode.geometry.nodes.find((node) => node.motion?.attribute === "tilt")!;
    pan.glb_node = "PanVisual";
    tilt.glb_node = "TiltVisual";
    const fixture = {
      fixture_id: profile.id,
      universe: 1,
      address: 1,
      definition: fixtureDefinitionFromProfileMode(profile, mode),
      logical_heads: [],
    } as PatchedFixture;
    const { scene, fixtureObjects } = buildStageScene([{
      fixture,
      index: 0,
      position: { x: 0, y: 0, z: 3, rotationX: 0, rotationY: 0, rotationZ: 0 },
    }], {
      revision: 1,
      generated_at: "",
      grand_master: 1,
      blackout: false,
      values: [{ fixture_id: profile.id, attribute: "pan", value: { kind: "normalized", value: .75 } }],
    });
    const model = new THREE.Group();
    const panVisual = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1));
    panVisual.name = "PanVisual";
    const tiltVisual = new THREE.Mesh(new THREE.SphereGeometry(.5));
    tiltVisual.name = "TiltVisual";
    panVisual.add(tiltVisual);
    model.add(panVisual);
    const root = fixtureObjects.get(profile.id)!;

    expect(mountFixtureModel(root, model, fixture)).toBe(2);
    const panPart = scene.getObjectByName(`fixture-model-part:${pan.id}`)!;
    const tiltPart = scene.getObjectByName(`fixture-model-part:${tilt.id}`)!;
    expect(panPart.parent?.name).toBe(`geometry-node-anchor:${pan.id}`);
    expect(tiltPart.parent?.name).toBe(`geometry-node-anchor:${tilt.id}`);
    expect(panPart.getObjectByName("PanVisual")).toBeTruthy();
    expect(panPart.getObjectByName("TiltVisual")).toBeUndefined();
    expect(tiltPart.getObjectByName("TiltVisual")).toBeTruthy();
    expect(scene.getObjectByName(`geometry-part:${pan.id}`)).toBeUndefined();
    expect(scene.getObjectByName(`geometry-node:${pan.id}`)?.rotation.y).toBeCloseTo(THREE.MathUtils.degToRad(135));
  });

  it("uses post-profile calibrated color and mastered intensity without applying desk masters twice", () => {
    const profile = blankFixtureProfile();
    profile.manufacturer = "Acme";
    profile.name = "Projected Lamp";
    profile.revision = 1;
    const mode = profile.modes[0];
    mode.channels = [{ ...blankChannel(mode), attribute: "intensity" }];
    const fixture = {
      fixture_id: profile.id,
      universe: 1,
      address: 1,
      definition: fixtureDefinitionFromProfileMode(profile, mode),
      logical_heads: [],
    } as PatchedFixture;
    const { scene } = buildStageScene([{
      fixture,
      index: 0,
      position: { x: 0, y: 0, z: 3, rotationX: 0, rotationY: 0, rotationZ: 0 },
    }], {
      revision: 1,
      generated_at: "",
      grand_master: .1,
      blackout: true,
      values: [
        { fixture_id: profile.id, attribute: "intensity", value: { kind: "normalized", value: 1 } },
        { fixture_id: profile.id, attribute: "color", value: { kind: "color_xyz", value: { x: .4124564, y: .2126729, z: .0193339 } } },
      ],
      profile_output_values: [
        { fixture_id: profile.id, attribute: "intensity", value: { kind: "normalized", value: .25 } },
        { fixture_id: profile.id, attribute: "color", value: { kind: "color_xyz", value: { x: .1804375, y: .072175, z: .9503041 } } },
      ],
    });
    const emitter = scene.getObjectByName(`geometry-emitter:${mode.geometry.emitters[0].id}`)!;
    expect(emitter.userData.intensity).toBe(.25);
    expect(emitter.userData.color).toBe("#0000ff");
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

  it("builds a selected Sunstrip scene without invalid outline geometry", () => {
    const sunstrip = fixture("strip light", "Sunstrip");
    expect(() => buildStageScene([{
      fixture: sunstrip,
      index: 0,
      position: { x: 0, y: 0, z: 3, rotationX: 0, rotationY: 0, rotationZ: 0 },
    }], null, new Set([sunstrip.fixture_id]))).not.toThrow();
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
