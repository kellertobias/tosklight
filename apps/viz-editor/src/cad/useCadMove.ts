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
import { cadSession } from "./session";
import type { CadSceneSnapshot, CadTransformPreview } from "./types";

export function useCadMove({
	sceneRef,
	applyScene,
	onError,
	snapToMounts,
	blocked,
}: {
	sceneRef: RefObject<CadSceneSnapshot | null>;
	applyScene(scene: CadSceneSnapshot): void;
	onError(reason: string): void;
	snapToMounts: boolean;
	/** While print pages are open the rig does not move. */
	blocked: boolean;
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

	return { previewStore, onPreview, move };
}
