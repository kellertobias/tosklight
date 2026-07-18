import type {
	DmxSnapshot,
	MediaServerFixture,
	VisualizationSnapshot,
} from "../types";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export interface MediaPreviewRefresh {
	fixture_id: string;
	source: number;
	format: string;
	width: number;
	height: number;
}

export class MediaApiClient {
	constructor(private readonly transport: ClientTransport) {}

	visualization(preload = false): Promise<VisualizationSnapshot> {
		const query = preload ? "?preload=true" : "";
		return this.transport.request(`/api/v1/visualization${query}`);
	}

	dmx(): Promise<DmxSnapshot> {
		return this.transport.request("/api/v1/dmx", {}, false);
	}

	mediaServers(): Promise<{ fixtures: MediaServerFixture[] }> {
		return this.transport.request("/api/v1/media");
	}

	refreshMediaPreview(
		fixtureId: string,
		source: number,
		width: number,
		height: number,
	): Promise<MediaPreviewRefresh> {
		return this.transport.request(
			`/api/v1/media/${fixtureId}/preview/refresh`,
			jsonRequest("POST", { source, width, height }),
		);
	}

	mediaPreview(fixtureId: string, source: number): Promise<Blob> {
		return this.transport.blob(`/api/v1/media/${fixtureId}/preview/${source}`);
	}

	refreshMediaThumbnails(
		fixtureId: string,
		elements: number[],
		width: number,
		height: number,
	): Promise<{ fixture_id: string; count: number }> {
		return this.transport.request(
			`/api/v1/media/${fixtureId}/thumbnails/refresh`,
			jsonRequest("POST", { library_type: 1, elements, width, height }),
		);
	}
}
