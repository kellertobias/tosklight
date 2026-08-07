import type {
	CueThumbnailIndex as WireCueThumbnailIndex,
	CueThumbnailUpdateOutcome as WireCueThumbnailUpdateOutcome,
	CueThumbnailUpload as WireCueThumbnailUpload,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

/** What the show already holds for one cue, without the pixels. */
export interface CueThumbnailEntry {
	cueId: string;
	stateHash: string;
	updatedAt: string;
}

/**
 * One redrawn preview on its way to the show.
 *
 * `imageBase64` is the encoded picture without any `data:` prefix, exactly the payload the server
 * stores.
 */
export interface CueThumbnailUpload {
	cueId: string;
	stateHash: string;
	imageBase64: string;
	width: number;
	height: number;
}

export interface CueThumbnailUpdateOutcome {
	stored: number;
	skippedCueIds: string[];
}

/**
 * Persisted cue preview pictures.
 *
 * A desk draws a preview once, when the cue is recorded or edited, and stores it with the show.
 * Every later desk reads the stored picture instead of redrawing the whole cue list on open.
 */
export class CueThumbnailApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async index(showId: string): Promise<CueThumbnailEntry[]> {
		const snapshot = await this.transport.request<WireCueThumbnailIndex>(
			"/api/v2/cues/thumbnails",
			{ headers: { "x-tosk-show": showId } },
		);
		return snapshot.entries.map((entry) => ({
			cueId: entry.cue_id,
			stateHash: entry.state_hash,
			updatedAt: entry.updated_at,
		}));
	}

	/**
	 * Fetches one stored picture as an object URL usable directly as an `<img src>`.
	 *
	 * The picture is fetched rather than linked because the route is authenticated; the caller
	 * owns revoking the returned URL.
	 */
	async imageUrl(showId: string, cueId: string): Promise<string> {
		const image = await this.transport.blob(
			`/api/v2/cues/${encodeURIComponent(cueId)}/thumbnail`,
			{ headers: { "x-tosk-show": showId } },
		);
		return URL.createObjectURL(image);
	}

	/**
	 * Stores redrawn previews.
	 *
	 * Editing one cue restages every cue that tracks from it, so the whole affected run goes up as
	 * one request rather than one request per cue.
	 */
	async store(
		showId: string,
		uploads: CueThumbnailUpload[],
	): Promise<CueThumbnailUpdateOutcome> {
		const thumbnails: WireCueThumbnailUpload[] = uploads.map((upload) => ({
			cue_id: upload.cueId,
			state_hash: upload.stateHash,
			image_base64: upload.imageBase64,
			width: upload.width,
			height: upload.height,
		}));
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			thumbnails,
		});
		const outcome =
			await this.transport.request<WireCueThumbnailUpdateOutcome>(
				"/api/v2/cues/thumbnails/update",
				{
					...request,
					headers: { ...request.headers, "x-tosk-show": showId },
				},
			);
		return {
			stored: outcome.stored,
			skippedCueIds: outcome.skipped_cue_ids,
		};
	}
}
