import { useEffect, useRef } from "react";
import * as THREE from "three";
import type { FixtureMode } from "../wire";
import {
	buildFixtureProfileGeometryPreview,
	visibleGeometryBounds,
} from "../stageGeometry";
import { useFixtureProfileEditorPorts } from "./ports";
import { NumberField } from "@tosklight/ui";

export function VectorFields({
	label,
	value,
	onChange,
}: {
	label: string;
	value: { x: number; y: number; z: number };
	onChange: (value: { x: number; y: number; z: number }) => void;
}) {
	return (
		<fieldset className="geometry-vector">
			<legend>{label}</legend>
			{(["x", "y", "z"] as const).map((axis) => (
				<NumberField
					key={axis}
					label={axis.toUpperCase()}
					allowDecimal
					value={value[axis]}
					onChange={(event) =>
						onChange({ ...value, [axis]: Number(event.target.value) })
					}
				/>
			))}
		</fieldset>
	);
}

/** Dark blue rather than black, so the dark lamp body still reads against it. */
export const GEOMETRY_PREVIEW_BACKGROUND = 0x14233a;

/** Point the camera at the lamp from above and to the front-right, with it filling most of the view. */
function frameLamp(camera: THREE.PerspectiveCamera, lamp: THREE.Object3D) {
	const bounds = visibleGeometryBounds(lamp);
	const center = bounds.isEmpty()
		? new THREE.Vector3()
		: bounds.getCenter(new THREE.Vector3());
	const radius = bounds.isEmpty()
		? 0.5
		: Math.max(0.05, bounds.getSize(new THREE.Vector3()).length() / 2);
	const distance = radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2));
	const direction = new THREE.Vector3(0.55, 0.4, 1).normalize();
	camera.near = Math.max(0.001, distance / 100);
	camera.far = distance * 100;
	camera.position.copy(center).addScaledVector(direction, distance * 1.15);
	camera.lookAt(center);
}

export function GeometryPreview({ mode }: { mode: FixtureMode }) {
	const host = useRef<HTMLDivElement>(null);
	const { disposeScene } = useFixtureProfileEditorPorts();
	useEffect(() => {
		const container = host.current;
		if (!container || typeof WebGLRenderingContext === "undefined") return;
		const scene = new THREE.Scene();
		scene.background = new THREE.Color(GEOMETRY_PREVIEW_BACKGROUND);
		scene.add(new THREE.HemisphereLight(0xdcefff, 0x1c2a3c, 2.4));
		const key = new THREE.DirectionalLight(0xffffff, 1.6);
		key.position.set(2, 3, 4);
		scene.add(key);
		const lamp = buildFixtureProfileGeometryPreview(mode);
		scene.add(lamp);
		const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 100);
		frameLamp(camera, lamp);
		let renderer: THREE.WebGLRenderer;
		try {
			renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
		} catch {
			disposeScene(scene);
			return;
		}
		renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
		renderer.outputColorSpace = THREE.SRGBColorSpace;
		container.replaceChildren(renderer.domElement);
		const render = () => {
			const width = Math.max(260, container.clientWidth);
			const height = Math.max(260, container.clientHeight);
			renderer.setSize(width, height, false);
			camera.aspect = width / height;
			camera.updateProjectionMatrix();
			renderer.render(scene, camera);
		};
		render();
		const observer =
			typeof ResizeObserver === "undefined" ? null : new ResizeObserver(render);
		observer?.observe(container);
		return () => {
			observer?.disconnect();
			disposeScene(scene);
			renderer.dispose();
			renderer.domElement.remove();
		};
	}, [mode, disposeScene]);
	return (
		<section
			className="geometry-live-preview"
			aria-label="Live geometry preview"
		>
			<h3>Live 3D preview</h3>
			<div
				ref={host}
				className="geometry-preview-stage"
				role="img"
				aria-label="Fixture geometry hierarchy in three dimensions"
			/>
			<small>
				{mode.geometry.nodes.length} parts · {mode.geometry.emitters.length}{" "}
				emitters. Preview shows the lamp with the Stage renderer's hierarchy,
				transforms, and emitter faces; beams are drawn on Stage.
			</small>
		</section>
	);
}
