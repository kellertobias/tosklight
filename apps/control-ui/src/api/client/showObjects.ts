import type { PresetFamily } from "../../presetFamilies";
import type { ShowObjectMutationResponse } from "../../features/showObjects/contracts";
import type {
	OutputRoute,
	OutputRouteAction,
	OutputRouteActionOutcome,
	ShowObjectCollectionSnapshot,
	ShowObjectExactSnapshot,
} from "../generated/light-wire";
import { ApiRequestError } from "../ApiRequestError";
import type { VersionedObject } from "../types";
import { type ClientTransport, jsonRequest } from "./transport";

interface PreloadStoreInput {
	target: "preset" | "cue";
	target_id: string;
	cue_number?: number;
	name?: string;
	mode?: "merge" | "overwrite" | "add_missing_fixtures";
	family?: PresetFamily;
}

export class ShowObjectsApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async objects<T>(
		showId: string,
		kind: string,
	): Promise<VersionedObject<T>[]> {
		const snapshot =
			await this.transport.request<ShowObjectCollectionSnapshot>(
				`/api/v2/objects/${encodeURIComponent(kind)}`,
				{ headers: showScopeHeaders(showId) },
			);
		return snapshot.objects as VersionedObject<T>[];
	}

	async object<T>(showId: string, kind: string, id: string) {
		const object = await this.objectOrNull<T>(showId, kind, id);
		if (!object) throw new ApiRequestError("show object not found", 404);
		return object;
	}

	async objectOrNull<T>(showId: string, kind: string, id: string) {
		const snapshot = await this.transport.request<ShowObjectExactSnapshot>(
			v2ShowObjectPath(kind, id),
			{ headers: showScopeHeaders(showId) },
		);
		return snapshot.object as VersionedObject<T> | null;
	}

	saveOutputRoute(
		showId: string,
		id: string,
		route: OutputRoute,
		revision: number,
	): Promise<OutputRouteActionOutcome> {
		const action: OutputRouteAction =
			revision === 0
				? { type: "create", route_id: id, route }
				: {
						type: "update",
						route_id: id,
						expected_revision: revision,
						patch: route,
					};
		return this.outputRouteAction(showId, action);
	}

	deleteOutputRoute(
		showId: string,
		id: string,
		revision: number,
	): Promise<OutputRouteActionOutcome> {
		return this.outputRouteAction(showId, {
			type: "delete",
			route_id: id,
			expected_revision: revision,
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

	private outputRouteAction(
		showId: string,
		action: OutputRouteAction,
	): Promise<OutputRouteActionOutcome> {
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			action,
		});
		return this.transport.request(
			"/api/v2/output-routes/actions",
			{
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			},
		).then((outcome) => outcome as OutputRouteActionOutcome);
	}
}

function showObjectPath(showId: string, kind: string, id: string): string {
	return `/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`;
}

function v2ShowObjectPath(kind: string, id: string): string {
	return `/api/v2/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`;
}

function showScopeHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}

function revisionHeaders(revision: number, json = false): HeadersInit {
	return {
		...(json ? { "content-type": "application/json" } : {}),
		"if-match": String(revision),
	};
}
