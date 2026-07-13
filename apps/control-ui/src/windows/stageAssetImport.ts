import JSZip from "jszip";
import * as THREE from "three";
import type { StageAsset } from "../api/ServerContext";

const supported = new Set(["glb", "stl", "3mf"]);
const position = { x: 0, y: 4, z: 0, rotationX: 0, rotationY: 0, rotationZ: 0 };

function dataUrl(bytes: Uint8Array, mime = "application/octet-stream") {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  return `data:${mime};base64,${btoa(binary)}`;
}

function asset(name: string, format: StageAsset["format"], bytes: Uint8Array, suffix = "", transform?: THREE.Matrix4): StageAsset {
  const translation = new THREE.Vector3();
  const rotation = new THREE.Quaternion();
  const scale = new THREE.Vector3(1, 1, 1);
  transform?.decompose(translation, rotation, scale);
  const euler = new THREE.Euler().setFromQuaternion(rotation, "XYZ");
  return { id: `${crypto.randomUUID()}${suffix}`, name, format, dataUrl: dataUrl(bytes), position: transform ? { x: translation.x / 1000, y: translation.y / 1000, z: translation.z / 1000, rotationX: THREE.MathUtils.radToDeg(euler.x), rotationY: THREE.MathUtils.radToDeg(euler.y), rotationZ: THREE.MathUtils.radToDeg(euler.z) } : { ...position }, scale: transform ? (scale.x + scale.y + scale.z) / 3 : 1 };
}

export async function importStageAssets(file: File): Promise<StageAsset[]> {
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (supported.has(extension)) return [asset(file.name, extension as StageAsset["format"], new Uint8Array(await file.arrayBuffer()))];
  if (extension !== "gdtf") throw new Error("Use GLB, STL, 3MF, or GDTF files. Import MVR from the Show dialog.");
  const zip = await JSZip.loadAsync(file);
  const modelFiles = Object.values(zip.files).filter((entry) => !entry.dir && entry.name.toLowerCase().endsWith(".glb"));
  if (!modelFiles.length) throw new Error(`${extension.toUpperCase()} archive contains no supported GLB geometry`);
  return Promise.all(modelFiles.map(async (entry, index) => asset(`${file.name} · ${entry.name}`, "glb", await entry.async("uint8array"), `-${index}`, new THREE.Matrix4().makeTranslation((index % 4) * 1500, Math.floor(index / 4) * 1500, 0))));
}
