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

function importPath(targetShowId: string, sourceShowId: string) {
	return `/api/v2/shows/${encodeURIComponent(targetShowId)}/selective-imports/${encodeURIComponent(sourceShowId)}`;
}

export class SelectiveImportApiClient {
	constructor(private readonly transport: ClientTransport) {}

	catalog(
		targetShowId: string,
		sourceShowId: string,
		signal?: AbortSignal,
	): Promise<SelectiveImportCatalog> {
		return this.transport.request<unknown>(
			`${importPath(targetShowId, sourceShowId)}/catalog`,
			{ signal },
		).then(selectiveImportCatalogFromWire);
	}

	preview(
		targetShowId: string,
		sourceShowId: string,
		selection: SelectiveImportSelection,
		signal?: AbortSignal,
	): Promise<SelectiveImportPreview> {
		return this.transport.request<unknown>(
			`${importPath(targetShowId, sourceShowId)}/preview`,
			{ ...jsonRequest("POST", selectiveImportSelectionToWire(selection)), signal },
		).then(selectiveImportPreviewFromWire);
	}

	apply(
		targetShowId: string,
		sourceShowId: string,
		request: SelectiveImportApplyRequest,
	): Promise<SelectiveImportOutcome> {
		const init = jsonRequest("POST", selectiveImportApplyToWire(request));
		const headers = new Headers(init.headers);
		headers.set("if-match", String(request.expectedTargetRevision));
		return this.transport.request<unknown>(
			`${importPath(targetShowId, sourceShowId)}/apply`,
			{ ...init, headers },
		).then(selectiveImportOutcomeFromWire);
	}
}
