import { createContext, useContext, type ComponentType } from "react";
import type * as THREE from "three";

/**
 * Picking a photograph, icon, or GLB is the host's business: the desk confines the chooser to its
 * configured file roots, while the Architect reads the operator's own filesystem. The editor only
 * needs the chosen files back.
 */
export type ProfileAssetPickerProps = {
	label: string;
	allowedExtensions?: string[];
	onFiles: (files: File[]) => void | Promise<void>;
};

/**
 * What the fixture-profile editor cannot own itself.
 *
 * The geometry preview is not one of them: it builds from `@tosklight/patch/stage-geometry`, the
 * same code the desk's Stage draws a profile with, so every host previews it the same way.
 */
export type FixtureProfileEditorPorts = {
	disposeScene: (scene: THREE.Object3D) => void;
	AssetPicker: ComponentType<ProfileAssetPickerProps>;
};

const FixtureProfileEditorPortsContext =
	createContext<FixtureProfileEditorPorts | null>(null);

export function FixtureProfileEditorPortsProvider({
	ports,
	children,
}: {
	ports: FixtureProfileEditorPorts;
	children: React.ReactNode;
}) {
	return (
		<FixtureProfileEditorPortsContext.Provider value={ports}>
			{children}
		</FixtureProfileEditorPortsContext.Provider>
	);
}

export function useFixtureProfileEditorPorts() {
	const ports = useContext(FixtureProfileEditorPortsContext);
	if (!ports)
		throw new Error(
			"The fixture-profile editor needs FixtureProfileEditorPortsProvider above it.",
		);
	return ports;
}
