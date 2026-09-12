import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CadViewDirection } from "./types";

/** One run of straight segments of a placed drawing, in the drawing's own millimetres. */
export interface UnderlayPolyline {
	points: [number, number][];
	closed: boolean;
	/** The source layer, kept for a later per-layer switch. */
	layer: string;
}

export interface UnderlayGeometry {
	polylines: UnderlayPolyline[];
	/** `[minX, minY, maxX, maxY]` in millimetres. */
	extentsMillimetres: [number, number, number, number];
	units: string;
}

/** A venue drawing placed on one CAD view. */
export interface CadUnderlay {
	id: string;
	name: string;
	sourceFormat: "dxf" | "svg";
	view: CadViewDirection;
	originMillimetres: [number, number];
	scale: number;
	rotationDegrees: number;
	visible: boolean;
	/** What the file said its units were, for the panel to report. */
	units: string;
	geometry: UnderlayGeometry;
}

/** What an import would add, reported before the show is written to. */
export interface CadUnderlayPreview {
	name: string;
	sourceFormat: "dxf" | "svg";
	units: string;
	polylineCount: number;
	pointCount: number;
	extentsMillimetres: [number, number, number, number];
}

export const underlaySession = {
	preview: (path: string) =>
		invoke<CadUnderlayPreview>("preview_cad_underlay", { path }),
	import: (path: string, view: CadViewDirection) =>
		invoke<CadUnderlay>("import_cad_underlay", { path, view }),
	all: () => invoke<CadUnderlay[]>("cad_underlays"),
	save: (underlay: CadUnderlay) =>
		invoke<CadUnderlay>("save_cad_underlay", { underlay }),
	remove: (id: string) => invoke<void>("delete_cad_underlay", { id }),
	/** Every window redraws when a drawing is added, placed or removed anywhere. */
	onDelta: (handler: (underlays: CadUnderlay[]) => void): Promise<UnlistenFn> =>
		listen<CadUnderlay[]>("cad-underlay-delta", (event) =>
			handler(event.payload),
		),
};
