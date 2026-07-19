import { useEffect, useRef } from "react";
import * as THREE from "three";
import type { FixtureMode } from "../../../api/types";
import {
	buildFixtureProfileGeometryPreview,
	disposeScene,
} from "../../../windows/stage3dScene";
import { NumberField } from "../../common";

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

export function GeometryPreview({ mode }: { mode: FixtureMode }) {
	const host = useRef<HTMLDivElement>(null);
	useEffect(() => {
		const container = host.current;
		if (!container || typeof WebGLRenderingContext === "undefined") return;
		const scene = new THREE.Scene();
		scene.background = new THREE.Color(0x080b0e);
		scene.add(new THREE.HemisphereLight(0xbfe9ff, 0x101820, 2));
		scene.add(buildFixtureProfileGeometryPreview(mode));
		const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 100);
		camera.position.set(3.5, 2.5, 6.5);
		camera.lookAt(0, -1.5, 0);
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
	}, [mode]);
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
				aria-label="Fixture geometry hierarchy and beams in three dimensions"
			/>
			<small>
				{mode.geometry.nodes.length} parts · {mode.geometry.emitters.length}{" "}
				emitters. Preview uses the Stage renderer's hierarchy, transforms,
				source layouts, and beam angles.
			</small>
		</section>
	);
}
