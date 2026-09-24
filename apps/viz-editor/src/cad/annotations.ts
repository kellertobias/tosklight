import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CadViewDirection } from "./types";

export type CadAnnotationKind = "polyline" | "box" | "text" | "measure";

/**
 * A line, box, text or measurement drawn on one CAD view.
 *
 * Points are plan millimetres on that view before a top-down tile's rotation: a polyline's
 * vertices, a box's two opposite corners, text's anchor, or a measurement's two ends.
 */
export interface CadAnnotation {
	/** Empty for an item not stored yet; the show assigns it. */
	id: string;
	view: CadViewDirection;
	kind: CadAnnotationKind;
	points: [number, number][];
	closed: boolean;
	text: string;
	textHeightMillimetres: number;
	/** The typeface text is set in, by its ID in `CAD_FONTS`; absent or empty for the screen's own. */
	font?: string;
}

export const annotationSession = {
	all: () => invoke<CadAnnotation[]>("cad_annotations"),
	save: (annotation: CadAnnotation) =>
		invoke<CadAnnotation>("save_cad_annotation", { annotation }),
	remove: (id: string) => invoke<void>("delete_cad_annotation", { id }),
	/** Changes an item already drawn — moves or rewords text — as one step Undo puts back. */
	change: (annotation: CadAnnotation) =>
		invoke<CadAnnotation>("cad_change_annotation", { annotation }),
	/** Every window redraws when an item is drawn or erased anywhere. */
	onDelta: (
		handler: (annotations: CadAnnotation[]) => void,
	): Promise<UnlistenFn> =>
		listen<CadAnnotation[]>("cad-annotation-delta", (event) =>
			handler(event.payload),
		),
};
