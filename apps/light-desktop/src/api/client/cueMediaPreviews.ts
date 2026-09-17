import type {
	CueMediaPreviewFailure as WireCueMediaPreviewFailure,
	CueMediaPreviewIndex as WireCueMediaPreviewIndex,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";

/** A Cue whose preview the addressed Media Server draws, and exactly which picture it is. */
export interface CueMediaPreviewEntry {
	cueId: string;
	cueListId: string;
	serverFixtureId: string;
	outputId: string | null;
	scope: "program" | "layer";
	/** Zero-based layer of a layer preview. */
	layer: number | null;
	layerFixtureId: string | null;
	/** Changes whenever the picture would; used as the image identity. */
	previewKey: string;
}

export type CueMediaPreviewFailureState = "offline" | "loading" | "missing";

export type CueMediaPreviewImage =
	| { kind: "picture"; blob: Blob; empty: boolean }
	| {
			kind: "failed";
			state: CueMediaPreviewFailureState;
			error: string;
			retryable: boolean;
	  };

/**
 * Cue previews drawn by the Media Server that plays the Cue.
 *
 * The desk server decides which Cues are media-only and computes the state each one leaves; this
 * client only reads the index and fetches the picture it names.
 */
export class CueMediaPreviewApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async index(showId: string): Promise<CueMediaPreviewEntry[]> {
		const snapshot = await this.transport.request<WireCueMediaPreviewIndex>(
			"/api/v2/cues/media-previews",
			{ headers: { "x-tosk-show": showId } },
		);
		return snapshot.entries.map((entry) => ({
			cueId: entry.cue_id,
			cueListId: entry.cue_list_id,
			serverFixtureId: entry.server_fixture_id,
			outputId: entry.output_id ?? null,
			scope: entry.scope,
			layer: entry.layer ?? null,
			layerFixtureId: entry.layer_fixture_id ?? null,
			previewKey: entry.preview_key,
		}));
	}

	/**
	 * Fetches one Cue's picture, or the named reason it has none.
	 *
	 * `previewKey` travels in the query so a changed Cue is a different request, never a cached
	 * picture of the previous state.
	 */
	async image(
		showId: string,
		cueId: string,
		previewKey: string,
		size: { width: number; height: number },
	): Promise<CueMediaPreviewImage> {
		const path = `/api/v2/cues/${encodeURIComponent(cueId)}/media-preview?width=${size.width}&height=${size.height}&key=${encodeURIComponent(previewKey)}`;
		const init = { headers: { "x-tosk-show": showId } };
		if (!this.transport.response) {
			const blob = await this.transport.blob(path, init);
			return { kind: "picture", blob, empty: false };
		}
		const response = await this.transport.response(path, init);
		if (response.ok)
			return {
				kind: "picture",
				blob: await response.blob(),
				empty: response.headers.get("x-light-media-preview") === "empty",
			};
		const failure = (await response
			.json()
			.catch(() => null)) as WireCueMediaPreviewFailure | null;
		if (failure && typeof failure.state === "string")
			return {
				kind: "failed",
				state: failure.state,
				error: failure.error,
				retryable: failure.retryable,
			};
		return {
			kind: "failed",
			state: response.status === 404 ? "missing" : "offline",
			error: `The desk answered ${response.status}.`,
			retryable: response.status !== 404,
		};
	}
}
