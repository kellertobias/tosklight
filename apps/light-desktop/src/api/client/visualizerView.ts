import type {
	VisualizerRenderQuality as WireVisualizerRenderQuality,
	VisualizerViewMode as WireVisualizerViewMode,
	VisualizerViewProjection as WireVisualizerViewProjection,
	VisualizerViewSnapshot as WireVisualizerViewSnapshot,
	VisualizerViewUpdateOutcome as WireVisualizerViewUpdateOutcome,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export type VisualizerViewMode =
	| "top_down"
	| "left_to_right"
	| "right_to_left"
	| "front_to_back"
	| "back_to_front"
	| "lines_3d"
	| "simple_3d"
	| "full_3d";

export type VisualizerRenderQuality = "draft" | "standard" | "high" | "ultra";

/** What one connected renderer is being told to look at. */
export interface VisualizerView {
	target: string;
	mode: VisualizerViewMode;
	quality: VisualizerRenderQuality;
	exposure: number;
	ambient: number;
	revision: number;
}

/** Only the fields being changed: selecting a view never resubmits a camera. */
export interface VisualizerViewPatch {
	mode?: VisualizerViewMode;
	quality?: VisualizerRenderQuality;
	exposure?: number;
	ambient?: number;
}

/** The desk-owned view every connected visualizer follows, by renderer target. */
export class VisualizerViewApiClient {
	constructor(private readonly transport: ClientTransport) {}

	views(): Promise<VisualizerView[]> {
		return this.transport
			.request<WireVisualizerViewSnapshot>("/api/v2/visualizer-views")
			.then((snapshot) => snapshot.views.map(mapView));
	}

	update(target: string, patch: VisualizerViewPatch): Promise<VisualizerView> {
		return this.transport
			.request<WireVisualizerViewUpdateOutcome>(
				`/api/v2/visualizer-views/${encodeURIComponent(target)}/update`,
				jsonRequest("POST", {
					request_id: crypto.randomUUID(),
					patch: {
						mode: patch.mode satisfies WireVisualizerViewMode | undefined,
						quality: patch.quality satisfies
							| WireVisualizerRenderQuality
							| undefined,
						exposure: patch.exposure,
						ambient: patch.ambient,
					},
				}),
			)
			.then((outcome) => mapView(outcome.view));
	}
}

function mapView(view: WireVisualizerViewProjection): VisualizerView {
	return {
		target: view.target,
		mode: view.mode,
		quality: view.quality,
		exposure: view.exposure,
		ambient: view.ambient,
		revision: view.revision,
	};
}
