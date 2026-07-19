import type {
	PresetAddress,
	PresetFamily,
} from "../../presetFamilies";
import { presetStorageKey } from "../../presetFamilies";
import type { ShowObjectMutationResponse } from "../../features/showObjects/contracts";
import { ApiRequestError } from "../ApiRequestError";
import type { StoredPreset, VersionedObject } from "../types";
import type { ClientTransport } from "./transport";

interface PreloadStoreInput {
	target: "preset" | "cue";
	target_id: string;
	cue_number?: number;
	name?: string;
	mode?: "merge" | "overwrite" | "add_missing_fixtures";
	family?: PresetFamily;
}

interface PresetStoreBody {
	name: string;
	family: PresetFamily;
	number: number;
	values: Record<string, Record<string, unknown>>;
	group_values?: Record<string, Record<string, unknown>>;
}

interface PresetStoreResult {
	revision: number;
	event_sequence: number | null;
	preset: StoredPreset;
	source_session: string;
}

export class ShowObjectsApiClient {
	constructor(private readonly transport: ClientTransport) {}

	objects<T>(showId: string, kind: string): Promise<VersionedObject<T>[]> {
		return this.transport.request(
			`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}`,
			{},
			false,
		);
	}

	object<T>(showId: string, kind: string, id: string) {
		return this.transport.request<VersionedObject<T>>(
			encodedShowObjectPath(showId, kind, id),
		);
	}

	objectOrNull<T>(showId: string, kind: string, id: string) {
		return this.object<T>(showId, kind, id).catch((reason) => {
			if (reason instanceof ApiRequestError && reason.status === 404) return null;
			throw reason;
		});
	}

	putObject<T>(
		showId: string,
		kind: string,
		id: string,
		body: T,
		revision: number,
	): Promise<ShowObjectMutationResponse> {
		return this.transport.request(showObjectPath(showId, kind, id), {
			method: "PUT",
			headers: revisionHeaders(revision, true),
			body: JSON.stringify(body),
		});
	}

	deleteObject(
		showId: string,
		kind: string,
		id: string,
		revision: number,
	): Promise<void> {
		return this.transport.request(showObjectPath(showId, kind, id), {
			method: "DELETE",
			headers: revisionHeaders(revision),
		});
	}

	storePreload(
		showId: string,
		input: PreloadStoreInput,
		revision: number,
	): Promise<ShowObjectMutationResponse> {
		return this.transport.request(`/api/v1/shows/${showId}/preload/store`, {
			method: "POST",
			headers: revisionHeaders(revision, true),
			body: JSON.stringify(input),
		});
	}

	undoObject(
		showId: string,
		kind: string,
		id: string,
		revision: number,
	): Promise<ShowObjectMutationResponse> {
		return this.transport.request(`${showObjectPath(showId, kind, id)}/undo`, {
			method: "POST",
			headers: revisionHeaders(revision),
		});
	}

	storePreset(
		showId: string,
		address: PresetAddress,
		preset: PresetStoreBody,
		mode: "merge" | "overwrite" | "add_missing_fixtures",
		revision: number,
	): Promise<PresetStoreResult> {
		const storageKey = presetStorageKey(address);
		return this.transport.request(
			`/api/v1/shows/${showId}/presets/${encodeURIComponent(storageKey)}/store`,
			{
				method: "POST",
				headers: revisionHeaders(revision, true),
				body: JSON.stringify({ mode, preset }),
			},
		);
	}
}

function showObjectPath(showId: string, kind: string, id: string): string {
	return `/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`;
}

function encodedShowObjectPath(
	showId: string,
	kind: string,
	id: string,
): string {
	return `/api/v1/shows/${encodeURIComponent(showId)}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`;
}

function revisionHeaders(revision: number, json = false): HeadersInit {
	return {
		...(json ? { "content-type": "application/json" } : {}),
		"if-match": String(revision),
	};
}
