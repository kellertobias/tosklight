import type {
	MvrApplyResult,
	MvrExportPreview,
	MvrImportPreview,
	ShowEntry,
	ShowRevision,
} from "../types";
import type {
	MvrImportResolution,
	MvrImportResolutionAction,
	RuntimeShowEntry,
	ShowLibraryAction,
	ShowLibraryActionOutcome,
	ShowLibraryActionResult,
	ShowLibrarySnapshot,
} from "../generated/light-wire";
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

	async shows(): Promise<ShowEntry[]> {
		const snapshot = await this.transport.request<ShowLibrarySnapshot>(
			"/api/v2/shows",
		);
		return snapshot.shows.map(showEntry);
	}

	createShow(
		name: string,
		dataBase64: string | null = null,
		overwrite = false,
	) {
		return this.showAction({
			type: "create",
			name,
			data_base64: dataBase64,
			overwrite,
		});
	}

	openShow(
		id: string,
		transition: ShowOpenTransition = "safe_blackout",
		transitionMillis?: number,
	) {
		return this.showAction({
			type: "open",
			show_id: id,
			transition,
			transition_millis: transitionMillis ?? null,
		} as ShowLibraryAction);
	}

	openCleanDefaultShow(): Promise<ShowEntry> {
		return this.showAction({
			type: "open_default",
			transition: "safe_blackout",
			transition_millis: null,
		});
	}

	renameShow(id: string, name: string): Promise<ShowEntry> {
		return this.showAction({ type: "rename", show_id: id, name });
	}

	overwriteShow(sourceId: string, destinationId: string): Promise<ShowEntry> {
		return this.showAction({
			type: "overwrite",
			source_show_id: sourceId,
			destination_show_id: destinationId,
		});
	}

	showRevisions(id: string): Promise<ShowRevision[]> {
		return this.transport
			.request<ShowLibrarySnapshot>("/api/v2/shows")
			.then(
				(snapshot) =>
					snapshot.shows.find((show) => show.id === id)?.revisions ?? [],
			);
	}

	saveShowRevision(id: string, name: string): Promise<ShowRevision> {
		return this.revisionAction({
			type: "save_revision",
			show_id: id,
			name,
		});
	}

	openShowRevision(id: string, revision: number): Promise<ShowEntry> {
		return this.showAction({
			type: "open_revision",
			show_id: id,
			revision,
			transition: "safe_blackout",
			transition_millis: null,
		});
	}

	rollbackShow(): Promise<ShowEntry> {
		return this.showAction({
			type: "rollback",
			transition: "safe_blackout",
			transition_millis: null,
		});
	}

	/**
	 * Load the document a Viz editor on the network has open.
	 *
	 * The desk fetches it, imports it as an ordinary show, and opens it — a copy, so patching
	 * either side afterwards leaves the other alone.
	 */
	importFromVisualizer(instance: string): Promise<ShowEntry> {
		return this.showAction({
			type: "import_from_visualizer",
			instance,
			open: true,
		});
	}

	downloadShow(id: string): Promise<Blob> {
		return this.transport.blob(`/api/v2/shows/${id}/download`);
	}

	previewMvr(file: File, showId?: string): Promise<MvrImportPreview> {
		const query = showId ? `?show_id=${encodeURIComponent(showId)}` : "";
		return this.transport.request(`/api/v2/mvr/imports/preview${query}`, {
			method: "POST",
			headers: { "content-type": "application/octet-stream" },
			body: file,
		});
	}

	applyMvr(token: string, input: MvrApplyInput): Promise<MvrApplyResult> {
		const destination = input.new_show
			? { type: "new_show" as const, ...input.new_show }
			: {
					type: "existing_show" as const,
					show_id: input.existing_show_id as string,
				};
		const resolutions = Object.entries(input.resolutions ?? {}).map(
			([fixture_id, resolution]): MvrImportResolution => ({
				fixture_id,
				action: mvrResolutionAction(resolution),
			}),
		);
		return this.action({
			type: "apply_mvr",
			token,
			destination,
			resolutions,
		}).then((result) => {
			if (result.type !== "mvr_apply") {
				throw new Error("show-library action returned an unexpected result");
			}
			return { ...result.result, show: showEntry(result.result.show) };
		});
	}

	mvrExportPreview(id: string): Promise<MvrExportPreview> {
		return this.transport.request(`/api/v2/shows/${id}/mvr/preview`);
	}

	downloadMvr(id: string): Promise<Blob> {
		return this.transport.blob(`/api/v2/shows/${id}/mvr`);
	}

	private async showAction(action: ShowLibraryAction): Promise<ShowEntry> {
		const result = await this.action(action);
		if (result.type !== "show") {
			throw new Error("show-library action returned an unexpected result");
		}
		return showEntry(result.show);
	}

	private async revisionAction(action: ShowLibraryAction): Promise<ShowRevision> {
		const result = await this.action(action);
		if (result.type !== "revision") {
			throw new Error("show-library action returned an unexpected result");
		}
		return result.revision;
	}

	private async action(
		action: ShowLibraryAction,
	): Promise<ShowLibraryActionResult> {
		const outcome = await this.transport.request<ShowLibraryActionOutcome>(
			"/api/v2/shows",
			jsonRequest("POST", { request_id: crypto.randomUUID(), action }),
		);
		return outcome.result;
	}
}

function showEntry(show: RuntimeShowEntry): ShowEntry {
	return {
		...show,
		revision_copy: show.revision_copy ?? undefined,
	};
}

function mvrResolutionAction(input: {
	action: string;
	universe?: number;
	address?: number;
}): MvrImportResolutionAction {
	if (input.action === "address") {
		return {
			type: "address",
			universe: input.universe ?? 1,
			address: input.address ?? 1,
		};
	}
	if (
		input.action === "import" ||
		input.action === "skip" ||
		input.action === "import_unpatched" ||
		input.action === "replace"
	) {
		return { type: input.action };
	}
	throw new Error(`unsupported MVR resolution action: ${input.action}`);
}
