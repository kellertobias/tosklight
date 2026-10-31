import type { ShowEntry } from "../types";
import type {
	ShowLibraryActionOutcome,
	ShowMergeApplyResult,
	ShowMergePreview,
	ShowMergeResolution,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { showEntry } from "./shows";
import { jsonRequest } from "./transport";

export type {
	ShowMergeApplyResult,
	ShowMergeConflict,
	ShowMergePreview,
	ShowMergeResolution,
} from "../generated/light-wire";

function mergePath(copyShowId: string) {
	return `/api/v2/show-merges/${encodeURIComponent(copyShowId)}`;
}

function withShow(targetShowId: string, init: RequestInit) {
	const headers = new Headers(init.headers);
	headers.set("x-tosk-show", targetShowId);
	return { ...init, headers };
}

/** Offline show copies and merging them back into the open show (TL-858). */
export class ShowMergeApiClient {
	constructor(private readonly transport: ClientTransport) {}

	/** Saves a restore point of the open show and a copy of it that remembers where it started. */
	async takeOfflineCopy(
		name: string,
		rootId: string,
		path: string,
	): Promise<ShowEntry> {
		const outcome = await this.transport.request<ShowLibraryActionOutcome>(
			"/api/v2/shows",
			jsonRequest("POST", {
				request_id: crypto.randomUUID(),
				action: { type: "take_offline_copy", name, root_id: rootId, path },
			}),
		);
		if (outcome.result.type !== "show")
			throw new Error("The desk did not return the offline copy");
		return showEntry(outcome.result.show);
	}

	preview(
		targetShowId: string,
		copyShowId: string,
		resolutions: ShowMergeResolution[],
		signal?: AbortSignal,
	): Promise<ShowMergePreview> {
		return this.transport.request<ShowMergePreview>(
			`${mergePath(copyShowId)}/preview`,
			{ ...withShow(targetShowId, jsonRequest("POST", { resolutions })), signal },
		);
	}

	apply(
		targetShowId: string,
		copyShowId: string,
		expectedTargetRevision: number,
		resolutions: ShowMergeResolution[],
	): Promise<ShowMergeApplyResult> {
		return this.transport.request<ShowMergeApplyResult>(
			`${mergePath(copyShowId)}/apply`,
			withShow(
				targetShowId,
				jsonRequest("POST", {
					request_id: crypto.randomUUID(),
					expected_target_revision: expectedTargetRevision,
					resolutions,
				}),
			),
		);
	}
}
