import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
	CadSceneDelta,
	CadSceneSnapshot,
	CadTransformOutcome,
	SelectionDelta,
} from "./types";

export const cadSession = {
	snapshot: () => invoke<CadSceneSnapshot>("cad_scene_snapshot"),
	replaceSelection: (
		expectedRevision: number,
		selectedIds: readonly string[],
	) =>
		invoke<SelectionDelta>("cad_replace_selection", {
			intent: { expectedRevision, selectedIds },
		}),
	transform: (
		expectedSceneRevision: number,
		entityIds: readonly string[],
		deltaMillimetres: readonly [number, number, number],
		snapToMounts: boolean,
		spread: boolean,
	) =>
		invoke<CadTransformOutcome>("cad_transform", {
			intent: {
				expectedSceneRevision,
				entityIds,
				deltaMillimetres,
				snapToMounts,
				spread,
			},
		}),
	/** Sets where fixtures stand and how they are turned, as one step that `undo` puts back. */
	setTransforms: (
		expectedSceneRevision: number,
		transforms: ReadonlyArray<{
			id: string;
			positionMillimetres: [number, number, number];
			rotationDegrees: [number, number, number];
		}>,
	) =>
		invoke<CadTransformOutcome>("cad_set_transforms", { expectedSceneRevision, transforms }),
	/** Adds whole fixtures, such as copies, as one step that `undo` takes away again. */
	add: (expectedSceneRevision: number, fixtures: readonly unknown[]) =>
		invoke<{ sceneRevision: number; addedIds: string[] }>("cad_add", {
			expectedSceneRevision,
			fixtures,
		}),
	/** Deletes fixtures from the show as one step that `undo` brings back. */
	delete: (expectedSceneRevision: number, fixtureIds: readonly string[]) =>
		invoke<{ sceneRevision: number; deletedIds: string[] }>("cad_delete", {
			expectedSceneRevision,
			fixtureIds,
		}),
	undo: (expectedSceneRevision: number) =>
		invoke<CadTransformOutcome>("cad_undo", { expectedSceneRevision }),
	redo: (expectedSceneRevision: number) =>
		invoke<CadTransformOutcome>("cad_redo", { expectedSceneRevision }),
	exportPdf: (path: string, bytes: Uint8Array) =>
		invoke<void>("cad_export_pdf", {
			path,
			bytesBase64: bytesToBase64(bytes),
		}),
	onSceneDelta: (
		handler: (delta: CadSceneDelta) => void,
	): Promise<UnlistenFn> =>
		listen<CadSceneDelta>("cad-scene-delta", (event) => handler(event.payload)),
	onSelectionDelta: (
		handler: (delta: SelectionDelta) => void,
	): Promise<UnlistenFn> =>
		listen<SelectionDelta>("cad-selection-delta", (event) =>
			handler(event.payload),
		),
};

function bytesToBase64(bytes: Uint8Array): string {
	let value = "";
	for (let offset = 0; offset < bytes.length; offset += 0x8000)
		value += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
	return btoa(value);
}
