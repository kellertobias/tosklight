import { createContext, useContext, type ComponentType } from "react";
import type * as THREE from "three";
import type { FixtureMode } from "../wire";

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
 * What the fixture-profile editor cannot own itself. Geometry preview needs the host's Stage
 * renderer so the preview and the real Stage agree about what a profile looks like.
 */
export type FixtureProfileEditorPorts = {
	buildGeometryPreview: (mode: FixtureMode) => THREE.Object3D;
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
