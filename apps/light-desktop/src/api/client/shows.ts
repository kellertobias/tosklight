import type {
	MvrApplyResult,
	MvrImportPreview,
	NetworkShowCatalog,
	ShowEntry,
	ShowRevision,
} from "../types";
import type {
	MvrImportResolution,
	MvrImportResolutionAction,
	MvrExportSummary,
	NetworkShowCatalog as WireNetworkShowCatalog,
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

export interface ShowSaveTarget { rootId: string; path: string; instance?: string; }
export interface ShowSaveFolders { roots: import("../types").FileRoot[]; entries: import("../types").FileEntry[]; root_id?: string | null; path?: string; }
/** An MVR archive the server wrote, with the summary of exactly what that archive carries. */
export interface ExportedMvrFile { root_id: string; path: string; summary: MvrExportSummary; }

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

    networkSaveFolders(instance: string, rootId: string, path: string): Promise<ShowSaveFolders> {
        const query = new URLSearchParams({path});
        if (rootId) query.set("root_id", rootId);
        return this.transport.request(`/api/v2/shows/network/${encodeURIComponent(instance)}/folders?${query}`);
    }
    saveShowCopy(sourceId: string, name: string, target: ShowSaveTarget, baseShow: boolean): Promise<ShowEntry> {
        return this.showAction(target.instance
            ? {type:"save_copy_to_peer",instance:target.instance,source_show_id:sourceId,name,root_id:target.rootId,path:target.path,is_base_show:baseShow}
            : {type:"save_copy",source_show_id:sourceId,data_base64:null,name,root_id:target.rootId,path:target.path,is_base_show:baseShow});
    }
    async exportMvrFile(showId: string, name: string, target: ShowSaveTarget): Promise<ExportedMvrFile> {
        const outcome = await this.action(target.instance
            ? {type:"export_mvr_to_peer",instance:target.instance,show_id:showId,name,root_id:target.rootId,path:target.path}
            : {type:"export_mvr_file",show_id:showId,data_base64:null,name,root_id:target.rootId,path:target.path});
        if (outcome.type !== "mvr_exported") throw new Error("The export did not return an MVR export summary");
        return {root_id:outcome.root_id,path:outcome.path,summary:outcome.summary};
    }
	async networkShows(): Promise<NetworkShowCatalog> {
		const catalog = await this.transport.request<WireNetworkShowCatalog>(
			"/api/v2/shows/network",
		);
		return {
			browsing: catalog.browsing,
			peers: catalog.peers.map((peer) => ({
				instance: peer.instance,
				name: peer.name,
				address: peer.address,
				role: peer.role,
				error: peer.error,
				shows: peer.shows.map((show) => ({
					id: show.id,
					name: show.name,
					updated_at: show.updated_at,
					revisions: show.revisions.map((revision) => ({
						show_id: revision.show_id,
						revision: revision.revision,
						name: revision.name,
						created_at: revision.created_at,
					})),
				})),
			})),
		};
	}
    prepareRevision(id: string, revision: number): Promise<ShowEntry> {
        return this.showAction({type:"prepare_revision",show_id:id,revision});
    }
    importRemoteShow(instance: string, id: string | null, revision: number | null, open: boolean): Promise<ShowEntry> {
        return this.showAction(id ? {type:"import_from_desk",instance,show_id:id,revision,open} : {type:"import_from_visualizer",instance,open});
    }
	setBaseShow(id: string, isBaseShow: boolean): Promise<ShowEntry> {
        return this.showAction({ type: "set_base_show", show_id: id, is_base_show: isBaseShow });
    }
    setDescription(id: string, description: string): Promise<ShowEntry> {
        return this.showAction({type:"set_description",show_id:id,description});
    }
    createFromBase(id: string, name: string): Promise<ShowEntry> {
        return this.showAction({ type: "create_from_base", show_id: id, name });
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

	previewMvr(file: File, showId?: string, signal?: AbortSignal): Promise<MvrImportPreview> {
		const query = showId ? `?show_id=${encodeURIComponent(showId)}` : "";
		return this.transport.request(`/api/v2/mvr/imports/preview${query}`, {
			method: "POST",
			headers: { "content-type": "application/octet-stream" },
			body: file,
            signal,
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
