/**
 * Moving the rig from a CAD viewport: the live preview while dragging, and the commit on release.
 *
 * On release the preview stays where the operator let go until the show has answered. The
 * committed scene and the cleared preview then land in one render, so no frame draws the element
 * back at its old position. A refused move restores the show's positions and says why.
 */
import { type RefObject, useState } from "react";
import { flushSync } from "react-dom";
import { createPreviewStore } from "./cadPreviewStore";
import { duplicateSelection } from "./cadDuplicate";
import { cadSession } from "./session";
import { selectionStep } from "./duplicateStep";
import type { CadSceneSnapshot, CadTransformPreview, ViewportTile } from "./types";

export function useCadMove({
	sceneRef,
	applyScene,
	onError,
	snapToMounts,
	blocked,
	onCopied,
}: {
	sceneRef: RefObject<CadSceneSnapshot | null>;
	applyScene(scene: CadSceneSnapshot): void;
	onError(reason: string): void;
	snapToMounts: boolean;
	/** While print pages are open the rig does not move. */
	blocked: boolean;
	/** The copies a duplicate made, which become the selection so the next move is theirs. */
	onCopied(ids: string[]): void;
}) {
	const [previewStore] = useState(createPreviewStore);

	function onPreview(preview: CadTransformPreview | null) {
		if (!preview) previewStore.clear();
		else previewStore.set(preview, sceneRef.current?.sceneRevision ?? -1);
	}

	/** Swaps the show's answer in and the preview out in the same synchronous render. */
	function settle(next: CadSceneSnapshot | null, error?: string) {
		flushSync(() => {
			if (error) onError(error);
			if (next) applyScene(next);
			previewStore.clear();
		});
	}

	async function move(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
		spread: boolean,
		snap = true,
	) {
		const scene = sceneRef.current;
		if (!scene || !entityIds.length || blocked) return settle(null);
		try {
			await cadSession.transform(
				scene.sceneRevision,
				entityIds,
				deltaMillimetres.map(Math.round) as [number, number, number],
				snapToMounts && snap,
				spread,
			);
			settle(await cadSession.snapshot());
		} catch (reason) {
			const refreshed = await cadSession.snapshot().catch(() => null);
			settle(refreshed, String(reason));
		}
	}

	/** Commits a turn: each fixture's new place and rotation, as the preview showed them. */
	async function turn(placements: NonNullable<CadTransformPreview["placements"]>) {
		const scene = sceneRef.current;
		if (!scene || !placements.length || blocked) return settle(null);
		try {
			await cadSession.setTransforms(scene.sceneRevision, placements);
			settle(await cadSession.snapshot());
		} catch (reason) {
			const refreshed = await cadSession.snapshot().catch(() => null);
			settle(refreshed, String(reason));
		}
	}

	/**
	 * Commits a move made with the duplicate modifier: copies moved by `delta`, the originals left
	 * where they are, as one step Undo takes away. The copies become the selection.
	 */
	async function copy(
		deltaMillimetres: [number, number, number],
		entityIds: readonly string[],
	): Promise<void> {
		const scene = sceneRef.current;
		if (!scene || !entityIds.length || blocked) return settle(null);
		try {
			const offset = deltaMillimetres.map(Math.round) as [number, number, number];
			const copies = await duplicateSelection(entityIds, offset, scene.sceneRevision);
			settle(await cadSession.snapshot());
			if (copies.length) onCopied(copies);
		} catch (reason) {
			const refreshed = await cadSession.snapshot().catch(() => null);
			settle(refreshed, String(reason));
		}
	}

	/** ⌘D: copies of the selection beside it on `tile`, as the object menu's Duplicate places them. */
	function duplicate(tile: Pick<ViewportTile, "view" | "rotationQuarterTurns"> | null | undefined) {
		const scene = sceneRef.current;
		const step = scene && selectionStep(scene.entities, scene.selectedIds, tile);
		return step ? copy(step, scene?.selectedIds ?? []) : Promise.resolve();
	}

	return { previewStore, onPreview, move, turn, copy, duplicate };
}
