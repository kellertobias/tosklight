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

	refreshMediaThumbnails(
		fixtureId: string,
		elements: number[],
		width = 128,
		height = 72,
	): Promise<{ fixture_id: string; count: number }> {
		const request = {
			library_type: 1,
			library_level: 0,
			library_1: 0,
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
