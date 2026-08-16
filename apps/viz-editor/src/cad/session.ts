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
	replaceSelection: (expectedRevision: number, selectedIds: readonly string[]) =>
		invoke<SelectionDelta>("cad_replace_selection", {
			intent: { expectedRevision, selectedIds },
		}),
	transform: (
		expectedSceneRevision: number,
		entityIds: readonly string[],
		deltaMillimetres: readonly [number, number, number],
		snapToMounts: boolean,
	) =>
		invoke<CadTransformOutcome>("cad_transform", {
			intent: {
				expectedSceneRevision,
				entityIds,
				deltaMillimetres,
				snapToMounts,
			},
		}),
	undo: (expectedSceneRevision: number) =>
		invoke<CadTransformOutcome>("cad_undo", { expectedSceneRevision }),
	redo: (expectedSceneRevision: number) =>
		invoke<CadTransformOutcome>("cad_redo", { expectedSceneRevision }),
	onSceneDelta: (handler: (delta: CadSceneDelta) => void): Promise<UnlistenFn> =>
		listen<CadSceneDelta>("cad-scene-delta", (event) => handler(event.payload)),
	onSelectionDelta: (
		handler: (delta: SelectionDelta) => void,
	): Promise<UnlistenFn> =>
		listen<SelectionDelta>("cad-selection-delta", (event) =>
			handler(event.payload),
		),
};
