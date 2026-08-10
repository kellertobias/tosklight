import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
} from "../../features/outputRuntime/contracts";
import type {
	DmxOverrideRequest,
	HighlightActionRequest,
	MediaPreviewRefreshRequest,
	MediaThumbnailRefreshRequest,
	PatchPreviewHighlightRequest,
} from "../generated/light-wire";
import {
	decodeOutputRuntimeActionOutcome,
	encodeOutputRuntimeActionRequest,
} from "../outputRuntimeWire";
import type {
	DmxSnapshot,
	HighlightAction,
	HighlightState,
	MediaServerFixture,
	VisualizationSnapshot,
} from "../types";
import type { LiveClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export interface MediaPreviewRefresh {
	fixture_id: string;
	source: number;
	format: string;
	width: number;
	height: number;
}

export interface MediaServerInspection {
	server: { name: string; layer_count: number };
	folders: Array<{ id: number; name: string; element_count: number }>;
	files: Array<{
		folder_id: number;
		id: number;
		name: string;
		width: number;
		height: number;
		length_frames: number;
		fps: number;
	}>;
	preview_sources: Array<{
		id: number;
		name: string;
		physical_output: number;
		layer: number | null;
		width: number;
		height: number;
	}>;
	layers: Array<{
		layer: number;
		physical_output: number;
		folder: number;
		file: number;
		name: string;
		position_frames: number;
		length_frames: number;
		fps: number;
		flags: number;
	}>;
}

export class MediaOutputApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	visualization(preload = false): Promise<VisualizationSnapshot> {
		const query = preload ? "?preload=true" : "";
		return this.transport.request(`/api/v2/output/visualization${query}`);
	}

	dmx(): Promise<DmxSnapshot> {
		return this.transport.request("/api/v2/output/dmx", {}, false);
	}

	mediaServers(): Promise<{ fixtures: MediaServerFixture[] }> {
		return this.transport.request("/api/v2/media-servers");
	}

	inspectMediaServer(fixtureId: string): Promise<MediaServerInspection> {
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/inspect`,
		);
	}

	refreshMediaPreview(
		fixtureId: string,
		source = 0,
		width = 320,
		height = 180,
	): Promise<MediaPreviewRefresh> {
		const request = {
			source,
			width,
			height,
		} satisfies MediaPreviewRefreshRequest;
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/preview/refresh`,
			jsonRequest("POST", request),
		);
	}

	mediaPreview(fixtureId: string, source = 0): Promise<Blob> {
		return this.transport.blob(
			`/api/v2/media-servers/${fixtureId}/preview/${source}`,
		);
	}

	mediaThumbnail(
		fixtureId: string,
		folder: number,
		element: number,
	): Promise<Blob> {
		return this.transport.blob(
			`/api/v2/media-servers/${fixtureId}/thumbnails/${folder}/${element}`,
		);
	}

	refreshMediaThumbnails(
		fixtureId: string,
		folder: number,
		elements: number[],
		width = 128,
		height = 72,
	): Promise<{ fixture_id: string; count: number }> {
		const request = {
			library_type: 1,
			library_level: 1,
			library_1: folder,
			library_2: 0,
			library_3: 0,
			elements,
			width,
			height,
		} satisfies MediaThumbnailRefreshRequest;
		return this.transport.request(
			`/api/v2/media-servers/${fixtureId}/thumbnails/refresh`,
			jsonRequest("POST", request),
		);
	}

	async outputRuntimeLiveAction(
		showId: string,
		request: OutputRuntimeActionRequest,
	): Promise<OutputRuntimeActionOutcome> {
		const wireRequest = encodeOutputRuntimeActionRequest(request);
		const value = await this.transport.sendAction(
			{ type: "output_runtime", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeOutputRuntimeActionOutcome(value, showId, request);
	}

	setDmxOverride(universe: number, address: number, value: number | null) {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			universe,
			address,
			value,
		} satisfies DmxOverrideRequest;
		return this.transport.sendAction(
			{ type: "dmx_override", request },
			requestId,
		);
	}

	highlight(): Promise<HighlightState> {
		return this.transport.request("/api/v2/output/highlight");
	}

	highlightAction(action: HighlightAction): Promise<HighlightState> {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			action,
		} satisfies HighlightActionRequest;
		return this.transport.sendAction(
			{ type: "highlight", request },
			requestId,
		) as Promise<HighlightState>;
	}

	setPatchPreviewHighlight(active: boolean, fixtureIds: string[] = []) {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			active,
			fixture_ids: fixtureIds,
		} satisfies PatchPreviewHighlightRequest;
		return this.transport.sendAction(
			{ type: "patch_preview_highlight", request },
			requestId,
		) as Promise<{ active: boolean; allowed: boolean }>;
	}
}
