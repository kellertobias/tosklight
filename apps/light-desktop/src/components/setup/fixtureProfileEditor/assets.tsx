import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { disposeScene } from "../../../windows/stage3dScene";
import { Button } from "../../common";
import { RootConfinedFilePickerButton } from "../../files/RootConfinedFilePickerButton";

function fileAsDataUrl(file: File) {
	return new Promise<string>((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(String(reader.result));
		reader.onerror = () => reject(reader.error);
		reader.readAsDataURL(file);
	});
}

export function AssetField({
	label,
	value,
	extensions,
	preview,
	onChange,
}: {
	label: string;
	value: string | null;
	extensions: string[];
	preview?: "image" | "glb";
	onChange: (value: string | null) => void;
}) {
	return (
		<div className="fixture-asset-field">
			<span style={{ color: "var(--muted)", fontSize: 12 }}>{label}</span>
			{preview === "image" && value && (
				<img src={value} alt="Fixture photograph preview" />
			)}
			{preview === "glb" && value && <GlbAssetPreview value={value} />}
			<div>
				<RootConfinedFilePickerButton
					label={
						value
							? `Replace ${label.toLowerCase()}`
							: `Choose ${label.toLowerCase()}`
					}
					allowedExtensions={extensions}
					onFiles={(files) => {
						const file = files[0];
						if (file) return fileAsDataUrl(file).then(onChange);
					}}
				/>
				{value && (
					<Button
						aria-label={`Remove ${label.toLowerCase()}`}
						onClick={() => onChange(null)}
					>
						Remove
					</Button>
				)}
			</div>
			<small>
				{value ? `${label} assigned` : `No ${label.toLowerCase()} assigned`}
			</small>
		</div>
	);
}

function GlbAssetPreview({ value }: { value: string }) {
	const host = useRef<HTMLDivElement>(null);
	const [metadata, setMetadata] = useState("Inspecting GLB model…");
	const [error, setError] = useState<string | null>(null);
	useEffect(() => {
		let cancelled = false;
		let renderer: THREE.WebGLRenderer | null = null;
		let controls: OrbitControls | null = null;
		let resizeObserver: ResizeObserver | null = null;
		let loadedScene: THREE.Object3D | null = null;
		void fetch(value)
			.then((response) => response.arrayBuffer())
			.then(
				(buffer) =>
					new Promise<void>((resolve, reject) => {
						new GLTFLoader().parse(
							buffer,
							"",
							(gltf) => {
								if (cancelled) return resolve();
								loadedScene = gltf.scene;
								let nodes = 0;
								let meshes = 0;
								gltf.scene.traverse((node) => {
									nodes += 1;
									if ((node as THREE.Mesh).isMesh) meshes += 1;
								});
								setMetadata(
									`GLB 2.0 · ${buffer.byteLength} bytes · ${nodes} nodes · ${meshes} meshes`,
								);
								setError(null);
								if (
									host.current &&
									typeof WebGLRenderingContext !== "undefined"
								) {
									const scene = new THREE.Scene();
									scene.background = new THREE.Color(0x090d10);
									scene.add(gltf.scene);
									scene.add(new THREE.HemisphereLight(0xffffff, 0x27313a, 2.2));
									const bounds = new THREE.Box3().setFromObject(gltf.scene);
									const center = bounds.getCenter(new THREE.Vector3());
									const size = Math.max(
										bounds.getSize(new THREE.Vector3()).length(),
										0.1,
									);
									gltf.scene.position.sub(center);
									const camera = new THREE.PerspectiveCamera(
										38,
										3 / 2,
										0.01,
										size * 20,
									);
									camera.position.set(size * 0.8, size * 0.55, size * 1.4);
									camera.lookAt(0, 0, 0);
									renderer = new THREE.WebGLRenderer({ antialias: true });
									renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
									renderer.outputColorSpace = THREE.SRGBColorSpace;
									host.current.replaceChildren(renderer.domElement);
									controls = new OrbitControls(camera, renderer.domElement);
									controls.enablePan = false;
									controls.target.set(0, 0, 0);
									const render = () => renderer?.render(scene, camera);
									controls.addEventListener("change", render);
									controls.update();
									const resize = () => {
										if (!host.current || !renderer) return;
										const width = Math.max(host.current.clientWidth, 1);
										const height = Math.max(host.current.clientHeight, 1);
										camera.aspect = width / height;
										camera.updateProjectionMatrix();
										renderer.setSize(width, height, false);
										render();
									};
									resizeObserver = new ResizeObserver(resize);
									resizeObserver.observe(host.current);
									resize();
								}
								resolve();
							},
							reject,
						);
					}),
			)
			.catch((reason) => {
				if (cancelled) return;
				setMetadata("");
				setError(
					`GLB preview failed: ${reason instanceof Error ? reason.message : String(reason)}`,
				);
			});
		return () => {
			cancelled = true;
			resizeObserver?.disconnect();
			controls?.dispose();
			if (loadedScene) disposeScene(loadedScene as THREE.Scene);
			renderer?.dispose();
		};
	}, [value]);
	return (
		<div className="fixture-glb-preview">
			<div
				ref={host}
				role="img"
				aria-label="Visualizer GLB model preview"
				title="Drag to rotate; scroll to zoom"
			/>
			<small>Drag to rotate · Scroll to zoom</small>
			<small role={error ? "alert" : "status"}>{error ?? metadata}</small>
		</div>
	);
}
