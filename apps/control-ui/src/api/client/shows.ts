import type {
	MvrApplyResult,
	MvrExportPreview,
	MvrImportPreview,
	ShowEntry,
	ShowRevision,
} from "../types";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export type ShowOpenTransition =
	| "hold_current"
	| "timed_fade"
	| "safe_blackout";

export interface MvrApplyInput {
	new_show?: { name: string; open_after_import: boolean };
	existing_show_id?: string;
	resolutions?: Record<
		string,
		{ action: string; universe?: number; address?: number }
	>;
}

export class ShowApiClient {
	constructor(private readonly transport: ClientTransport) {}

	shows(): Promise<ShowEntry[]> {
		return this.transport.request("/api/v1/shows", {}, false);
	}

	createShow(name: string, dataBase64: string | null, overwrite: boolean) {
		return this.transport.request<ShowEntry>(
			"/api/v1/shows",
			jsonRequest("POST", { name, data_base64: dataBase64, overwrite }),
		);
	}

	openShow(
		id: string,
		transition: ShowOpenTransition,
		transitionMillis?: number,
	) {
		return this.transport.request<ShowEntry>(
			`/api/v1/shows/${id}/open`,
			jsonRequest("POST", { transition, transition_millis: transitionMillis }),
		);
	}

	openCleanDefaultShow(): Promise<ShowEntry> {
		return this.transport.request(
			"/api/v1/shows/default/open",
			jsonRequest("POST", { transition: "safe_blackout" }),
		);
	}

	renameShow(id: string, name: string): Promise<ShowEntry> {
		return this.transport.request(
			`/api/v1/shows/${id}/rename`,
			jsonRequest("PUT", { name }),
		);
	}

	overwriteShow(sourceId: string, destinationId: string): Promise<ShowEntry> {
		const path = `/api/v1/shows/${sourceId}/overwrite/${destinationId}`;
		return this.transport.request(path, { method: "POST" });
	}

	showRevisions(id: string): Promise<ShowRevision[]> {
		return this.transport.request(`/api/v1/shows/${id}/revisions`);
	}

	saveShowRevision(id: string, name: string): Promise<ShowRevision> {
		return this.transport.request(
			`/api/v1/shows/${id}/revisions`,
			jsonRequest("POST", { name }),
		);
	}

	openShowRevision(id: string, revision: number): Promise<ShowEntry> {
		const path = `/api/v1/shows/${id}/revisions/${revision}/open`;
		return this.transport.request(
			path,
			jsonRequest("POST", { transition: "safe_blackout" }),
		);
	}

	rollbackShow(): Promise<ShowEntry> {
		return this.transport.request(
			"/api/v1/shows/rollback",
			jsonRequest("POST", { transition: "safe_blackout" }),
		);
	}

	downloadShow(id: string): Promise<Blob> {
		return this.transport.blob(`/api/v1/shows/${id}/download`);
	}

	previewMvr(file: File, showId?: string): Promise<MvrImportPreview> {
		const query = showId ? `?show_id=${encodeURIComponent(showId)}` : "";
		return this.transport.request(`/api/v1/mvr/imports/preview${query}`, {
			method: "POST",
			headers: { "content-type": "application/octet-stream" },
			body: file,
		});
	}

	applyMvr(token: string, input: MvrApplyInput): Promise<MvrApplyResult> {
		return this.transport.request(
			`/api/v1/mvr/imports/${token}/apply`,
			jsonRequest("POST", input),
		);
	}

	mvrExportPreview(id: string): Promise<MvrExportPreview> {
		return this.transport.request(`/api/v1/shows/${id}/mvr/preview`);
	}

	downloadMvr(id: string): Promise<Blob> {
		return this.transport.blob(`/api/v1/shows/${id}/mvr`);
	}
}
