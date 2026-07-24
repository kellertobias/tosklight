import {
	type SelectiveImportApplyRequest,
	type SelectiveImportCatalog,
	type SelectiveImportOutcome,
	type SelectiveImportPreview,
	type SelectiveImportSelection,
} from "../selectiveImportModels";
import {
	selectiveImportApplyToWire,
	selectiveImportCatalogFromWire,
	selectiveImportOutcomeFromWire,
	selectiveImportPreviewFromWire,
	selectiveImportSelectionToWire,
} from "../selectiveImportWire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

function importPath(sourceShowId: string) {
	return `/api/v2/selective-imports/${encodeURIComponent(sourceShowId)}`;
}

function showHeaders(targetShowId: string, headers?: HeadersInit) {
	const result = new Headers(headers);
	result.set("x-tosk-show", targetShowId);
	return result;
}

export class SelectiveImportApiClient {
	constructor(private readonly transport: ClientTransport) {}

	catalog(
		targetShowId: string,
		sourceShowId: string,
		signal?: AbortSignal,
	): Promise<SelectiveImportCatalog> {
		return this.transport.request<unknown>(
			`${importPath(sourceShowId)}/catalog`,
			{ signal, headers: showHeaders(targetShowId) },
		).then(selectiveImportCatalogFromWire);
	}

	preview(
		targetShowId: string,
		sourceShowId: string,
		selection: SelectiveImportSelection,
		signal?: AbortSignal,
	): Promise<SelectiveImportPreview> {
		const init = jsonRequest("POST", selectiveImportSelectionToWire(selection));
		return this.transport
			.request<unknown>(`${importPath(sourceShowId)}/preview`, {
				...init,
				headers: showHeaders(targetShowId, init.headers),
				signal,
			})
			.then(selectiveImportPreviewFromWire);
	}

	apply(
		targetShowId: string,
		sourceShowId: string,
		request: SelectiveImportApplyRequest,
	): Promise<SelectiveImportOutcome> {
		const init = jsonRequest("POST", selectiveImportApplyToWire(request));
		const headers = showHeaders(targetShowId, init.headers);
		headers.set("if-match", String(request.expectedTargetRevision));
		return this.transport.request<unknown>(
			`${importPath(sourceShowId)}/apply`,
			{ ...init, headers },
		).then(selectiveImportOutcomeFromWire);
	}
}
