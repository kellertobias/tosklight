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

function directMatrix(element: Element) {
  const node = Array.from(element.children).find((child) => child.tagName.toLowerCase().endsWith("matrix"));
  if (!node?.textContent) return new THREE.Matrix4();
  const values = node.textContent.match(/[-+]?\d*\.?\d+(?:e[-+]?\d+)?/gi)?.map(Number) ?? [];
  if (values.length !== 12) return new THREE.Matrix4();
  return new THREE.Matrix4().set(values[0], values[1], values[2], values[9], values[3], values[4], values[5], values[10], values[6], values[7], values[8], values[11], 0, 0, 0, 1);
}

async function importMvr(file: File, zip: JSZip) {
  const description = Object.values(zip.files).find((entry) => entry.name.toLowerCase().endsWith("generalscenedescription.xml"));
  if (!description) throw new Error("MVR archive is missing GeneralSceneDescription.xml");
  const document = new DOMParser().parseFromString(await description.async("text"), "application/xml");
  if (document.querySelector("parsererror")) throw new Error("MVR scene description is invalid XML");
  const files = new Map(Object.values(zip.files).filter((entry) => !entry.dir).map((entry) => [entry.name.toLowerCase(), entry]));
  const results: StageAsset[] = [];
  const visit = async (element: Element, parent: THREE.Matrix4) => {
    const local = directMatrix(element);
    const transform = parent.clone().multiply(local);
    if (element.tagName.toLowerCase().endsWith("geometry3d")) {
      const name = element.getAttribute("fileName") ?? element.getAttribute("filename") ?? "";
      const entry = files.get(name.toLowerCase()) ?? [...files.entries()].find(([path]) => path.endsWith(`/${name.toLowerCase()}`))?.[1];
      if (entry && name.toLowerCase().endsWith(".glb")) results.push(asset(`${file.name} · ${name}`, "glb", await entry.async("uint8array"), `-${results.length}`, transform));
    }
    for (const child of Array.from(element.children)) if (!child.tagName.toLowerCase().endsWith("matrix")) await visit(child, transform);
  };
  await visit(document.documentElement, new THREE.Matrix4());
  return results;
}

export async function importStageAssets(file: File): Promise<StageAsset[]> {
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (supported.has(extension)) return [asset(file.name, extension as StageAsset["format"], new Uint8Array(await file.arrayBuffer()))];
  if (extension !== "mvr" && extension !== "gdtf") throw new Error("Use GLB, STL, 3MF, MVR, or GDTF files");
  const zip = await JSZip.loadAsync(file);
  if (extension === "mvr") {
    const imported = await importMvr(file, zip);
    if (imported.length) return imported;
  }
  const modelFiles = Object.values(zip.files).filter((entry) => !entry.dir && entry.name.toLowerCase().endsWith(".glb"));
  if (!modelFiles.length) throw new Error(`${extension.toUpperCase()} archive contains no supported GLB geometry`);
  return Promise.all(modelFiles.map(async (entry, index) => asset(`${file.name} · ${entry.name}`, "glb", await entry.async("uint8array"), `-${index}`, new THREE.Matrix4().makeTranslation((index % 4) * 1500, Math.floor(index / 4) * 1500, 0))));
}
