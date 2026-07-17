import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { STLLoader } from "three/examples/jsm/loaders/STLLoader.js";
import { ThreeMFLoader } from "three/examples/jsm/loaders/3MFLoader.js";
import type { VisualizationSnapshot } from "../api/types";
import type { StageAsset, StagePosition3d } from "../api/ServerContext";
import {
  buildStageScene,
  disposeScene,
  mountFixtureModel,
  type Stage3dFixture,
} from "./stage3dScene";
import { useApp } from "../state/AppContext";
import { createBuiltInStageAsset } from "./builtInStageModels";

interface Props {
  fixtures: Stage3dFixture[];
  assets: StageAsset[];
  visualization: VisualizationSnapshot | null;
  selected: string[];
  setup: boolean;
  showSelection: boolean;
  environmentBrightness: number;
  onSelect: (fixtureId: string, additive: boolean) => void;
  onMove: (fixtureId: string, position: StagePosition3d) => void;
  onMoveEnd: (fixtureId: string, position: StagePosition3d) => void;
}

export function Stage3dCanvas({
  fixtures,
  assets,
  visualization,
  selected,
  setup,
  showSelection,
  environmentBrightness,
  onSelect,
  onMove,
  onMoveEnd,
}: Props) {
  const { state, dispatch } = useApp();
  const host = useRef<HTMLDivElement>(null);
  const cameraPosition = useRef(new THREE.Vector3(0, 3.2, 12));
  const cameraTarget = useRef(new THREE.Vector3(0, 1.8, -4));
  const latestVisualization = useRef(visualization);
  const interacting = useRef(false);
  const [renderVisualization, setRenderVisualization] = useState(visualization);
  const callbacks = useRef({ onSelect, onMove, onMoveEnd });
  callbacks.current = { onSelect, onMove, onMoveEnd };
  useEffect(() => {
    latestVisualization.current = visualization;
    if (!interacting.current) setRenderVisualization(visualization);
  }, [visualization]);

  useEffect(() => {
    const container = host.current;
    if (!container) return;
    const { scene, fixtureObjects } = buildStageScene(
      fixtures,
      renderVisualization,
      showSelection ? new Set(selected) : new Set(),
      environmentBrightness,
    );
    let assetCancelled = false;
    const decode = (value: string) =>
      fetch(value).then((response) => response.arrayBuffer());
    for (const item of fixtures) {
      const source = item.fixture.definition.model_asset;
      if (!source) continue;
      void decode(source).then((buffer) => {
        if (assetCancelled) return;
        new GLTFLoader().parse(buffer, "", (gltf) => {
          if (assetCancelled) return;
          const root = fixtureObjects.get(item.instanceId ?? item.fixture.fixture_id);
          if (!root) return;
          const placeholder = root.getObjectByName("fixture-placeholder");
          placeholder?.parent?.remove(placeholder);
          mountFixtureModel(
            root,
            gltf.scene,
            item.fixture,
            showSelection && selected.includes(item.fixture.fixture_id),
          );
        });
      }).catch(() => undefined);
    }
    for (const asset of assets)
      asset.format === "builtin" && asset.builtinId
        ? (() => {
            const object = createBuiltInStageAsset(asset.builtinId);
            object.name = `asset:${asset.id}`;
            object.position.set(asset.position.x, asset.position.z, -asset.position.y);
            object.rotation.set(
              THREE.MathUtils.degToRad(asset.position.rotationX),
              THREE.MathUtils.degToRad(asset.position.rotationZ),
              THREE.MathUtils.degToRad(asset.position.rotationY),
            );
            object.scale.setScalar(asset.scale);
            scene.add(object);
          })()
        : asset.dataUrl && void decode(asset.dataUrl)
        .then((buffer) => {
          if (assetCancelled) return;
          const apply = (object: THREE.Object3D) => {
            object.name = `asset:${asset.id}`;
            object.position.set(
              asset.position.x,
              asset.position.z,
              -asset.position.y,
            );
            object.rotation.set(
              THREE.MathUtils.degToRad(asset.position.rotationX),
              THREE.MathUtils.degToRad(asset.position.rotationZ),
              THREE.MathUtils.degToRad(asset.position.rotationY),
            );
            object.scale.setScalar(asset.scale);
            scene.add(object);
          };
          if (asset.format === "glb")
            new GLTFLoader().parse(buffer, "", (gltf) => apply(gltf.scene));
          else if (asset.format === "stl")
            apply(
              new THREE.Mesh(
                new STLLoader().parse(buffer),
                new THREE.MeshStandardMaterial({
                  color: 0x76818a,
                  roughness: 0.75,
                }),
              ),
            );
          else apply(new ThreeMFLoader().parse(buffer));
        })
        .catch(() => undefined);
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    container.replaceChildren(renderer.domElement);
    const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
    const orbitRadius = Math.max(2, 12 / Math.max(.2, state.stageZoom));
    const azimuth = THREE.MathUtils.degToRad(state.stageOrbitX);
    const elevation = THREE.MathUtils.degToRad(18 + state.stageOrbitY);
    camera.position.set(Math.sin(azimuth) * orbitRadius, 1.8 + Math.sin(elevation) * orbitRadius, -4 + Math.cos(azimuth) * Math.cos(elevation) * orbitRadius);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.copy(cameraTarget.current);
    controls.enableDamping = true;
    const rememberCamera = () => {
      cameraPosition.current.copy(camera.position);
      cameraTarget.current.copy(controls.target);
    };
    const publishCamera = () => {
      const offset = camera.position.clone().sub(controls.target);
      dispatch({ type: "SET_STAGE_NAVIGATION", zoom: 12 / Math.max(2, offset.length()), orbitX: THREE.MathUtils.radToDeg(Math.atan2(offset.x, offset.z)), orbitY: THREE.MathUtils.radToDeg(Math.asin(offset.y / offset.length())) - 18 });
    };
    controls.addEventListener("change", rememberCamera);
    controls.addEventListener("end", publishCamera);
    const raycaster = new THREE.Raycaster();
    const pointer = new THREE.Vector2();
    let dragging: {
      fixtureId: string;
      instanceId: string;
      root: THREE.Object3D;
      y: number;
      offset: THREE.Vector3;
      pending: StagePosition3d;
      additive: boolean;
    } | null = null;
    const positionById = new Map(
      fixtures.map((item) => [item.instanceId ?? item.fixture.fixture_id, item.position]),
    );
    const updatePointer = (event: PointerEvent) => {
      const box = renderer.domElement.getBoundingClientRect();
      pointer.set(
        ((event.clientX - box.left) / box.width) * 2 - 1,
        (-(event.clientY - box.top) / box.height) * 2 + 1,
      );
      raycaster.setFromCamera(pointer, camera);
    };
    const down = (event: PointerEvent) => {
      interacting.current = true;
      updatePointer(event);
      const hit = raycaster
        .intersectObjects([...fixtureObjects.values()], true)
        .find((entry) => {
          let node: THREE.Object3D | null = entry.object;
          while (node && !node.userData.fixtureId) node = node.parent;
          return Boolean(node?.userData.fixtureId);
        });
      if (!hit) return;
      let root: THREE.Object3D | null = hit.object;
      while (root && !root.userData.fixtureId) root = root.parent;
      const id = root?.userData.fixtureId as string;
      const instanceId = (root?.userData.instanceId as string) || id;
      const additive = event.metaKey || event.ctrlKey;
      if (!setup) callbacks.current.onSelect(id, additive);
      if (setup && root) {
        dragging = {
          fixtureId: id,
          instanceId,
          root,
          y: root.position.y,
          offset: root.position.clone().sub(hit.point),
          pending: positionById.get(instanceId)!,
          additive,
        };
        controls.enabled = false;
        renderer.domElement.setPointerCapture(event.pointerId);
      }
    };
    const move = (event: PointerEvent) => {
      if (!dragging) return;
      updatePointer(event);
      const plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), -dragging.y);
      const point = new THREE.Vector3();
      if (!raycaster.ray.intersectPlane(plane, point)) return;
      point.add(dragging.offset);
      const current = positionById.get(dragging.instanceId)!;
      dragging.root.position.copy(point);
      dragging.pending = { ...current, x: point.x, y: -point.z, z: point.y };
    };
    const up = () => {
      if (dragging) {
        callbacks.current.onSelect(dragging.fixtureId, dragging.additive);
        callbacks.current.onMove(dragging.instanceId, dragging.pending);
        callbacks.current.onMoveEnd(dragging.instanceId, dragging.pending);
      }
      dragging = null;
      controls.enabled = true;
      interacting.current = false;
      setRenderVisualization(latestVisualization.current);
    };
    renderer.domElement.addEventListener("pointerdown", down);
    renderer.domElement.addEventListener("pointermove", move);
    renderer.domElement.addEventListener("pointerup", up);
    renderer.domElement.addEventListener("pointercancel", up);
    const resize = () => {
      const { width, height } = container.getBoundingClientRect();
      renderer.setSize(width, height, false);
      camera.aspect = width / Math.max(height, 1);
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(container);
    resize();
    let frame = 0;
    const animate = () => {
      controls.update();
      renderer.render(scene, camera);
      frame = requestAnimationFrame(animate);
    };
    animate();
    return () => {
      assetCancelled = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      controls.dispose();
      controls.removeEventListener("change", rememberCamera);
      controls.removeEventListener("end", publishCamera);
      renderer.domElement.removeEventListener("pointerdown", down);
      renderer.domElement.removeEventListener("pointermove", move);
      renderer.domElement.removeEventListener("pointerup", up);
      renderer.domElement.removeEventListener("pointercancel", up);
      disposeScene(scene);
      renderer.forceContextLoss();
      renderer.dispose();
    };
  }, [fixtures, assets, renderVisualization, selected, setup, showSelection, environmentBrightness, state.stageZoom, state.stageOrbitX, state.stageOrbitY, dispatch]);

  return <div className="stage-3d-canvas" ref={host} />;
}
