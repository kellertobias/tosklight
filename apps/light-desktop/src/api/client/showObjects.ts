import type { PresetFamily } from "../../presetFamilies";
import { ApiRequestError } from "../ApiRequestError";
import type {
	DynamicCreateActionRequest,
	DynamicDeleteActionRequest,
	DynamicPoolActionRequest,
	DynamicSpatialPreviewRequest,
	DynamicSpatialPreviewResponse,
	DynamicUpdateActionRequest,
	DynamicUpdateIntent,
	OutputRoute,
	OutputRouteAction,
	OutputRouteActionOutcome,
	PreloadRecordAction,
	ShowObjectActionOutcome,
	ShowObjectCollectionSnapshot,
	ShowObjectExactSnapshot,
	UserLayoutPatch,
} from "../generated/light-wire";
import type { OutputRouteRangeIntent, VersionedObject } from "../types";
import { type ClientTransport, jsonRequest } from "./transport";

interface PreloadStoreInput {
	target: "preset" | "cue";
	target_id: string;
	cue_number?: string;
	name?: string;
	mode?: "merge" | "overwrite" | "add_missing_fixtures";
	family?: PresetFamily;
}

interface PatchLayerSaveInput {
	id: string;
	name: string;
	order: number;
}

export class ShowObjectsApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async objects<T>(
		showId: string,
		kind: string,
	): Promise<VersionedObject<T>[]> {
		return (await this.collectionSnapshot<T>(showId, kind)).objects;
	}

	async collectionSnapshot<T>(showId: string, kind: string) {
		const snapshot = await this.transport.request<ShowObjectCollectionSnapshot>(
			`/api/v2/objects/${encodeURIComponent(kind)}`,
			{ headers: showScopeHeaders(showId) },
		);
		return {
			objects: snapshot.objects as VersionedObject<T>[],
			showRevision: snapshot.show_revision,
		};
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

	createDynamic(
		showId: string,
		definition: import("../generated/light-wire").DynamicDefinitionProjection,
	): Promise<ShowObjectActionOutcome> {
		const request: DynamicCreateActionRequest = {
			request_id: crypto.randomUUID(),
			definition,
		};
		return this.dynamicAction("/api/v2/dynamics/create", showId, request);
	}

	moveDynamic(
		showId: string,
		id: string,
		expectedRevision: number,
		poolNumber: number,
	): Promise<ShowObjectActionOutcome> {
		const request: DynamicPoolActionRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
			pool_number: poolNumber,
		};
		return this.dynamicAction(
			`/api/v2/dynamics/${encodeURIComponent(id)}/move`,
			showId,
			request,
		);
	}

	copyDynamic(
		showId: string,
		id: string,
		expectedRevision: number,
		poolNumber: number,
	): Promise<ShowObjectActionOutcome> {
		const request: DynamicPoolActionRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
			pool_number: poolNumber,
		};
		return this.dynamicAction(
			`/api/v2/dynamics/${encodeURIComponent(id)}/copy`,
			showId,
			request,
		);
	}

	deleteDynamic(
		showId: string,
		id: string,
		expectedRevision: number,
	): Promise<ShowObjectActionOutcome> {
		const request: DynamicDeleteActionRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
		};
		return this.dynamicAction(
			`/api/v2/dynamics/${encodeURIComponent(id)}/delete`,
			showId,
			request,
		);
	}

	updateDynamic(
		showId: string,
		id: string,
		expectedRevision: number,
		intent: DynamicUpdateIntent,
		mutationGroup?: string,
	): Promise<ShowObjectActionOutcome> {
		const request: DynamicUpdateActionRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
			mutation_group: mutationGroup ?? null,
			intent,
		};
		return this.dynamicAction(
			`/api/v2/dynamics/${encodeURIComponent(id)}/update`,
			showId,
			request,
		);
	}

	previewDynamicSpatialMapping(
		showId: string,
		id: string,
		requestBody: DynamicSpatialPreviewRequest,
	): Promise<DynamicSpatialPreviewResponse> {
		const request = jsonRequest("POST", requestBody);
		return this.transport.request(
			`/api/v2/dynamics/${encodeURIComponent(id)}/spatial-preview`,
			{
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			},
		);
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

	createOutputRouteRange(
		showId: string,
		intent: OutputRouteRangeIntent,
	): Promise<OutputRouteActionOutcome> {
		return this.outputRouteAction(showId, {
			type: "create_range",
			range_id: crypto.randomUUID(),
			route: {
				...intent.route,
				logical_universe: intent.logical_start,
				destination_universe: intent.destination_start,
			},
			logical_universe_end: intent.logical_end,
			destination_universe_end: intent.destination_end,
		});
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

	updateUserLayout(
		showId: string,
		sessionId: string,
		patch: {
			desks: unknown[];
			activeDeskId: string;
			windowSettings?: unknown;
		},
		revision: number,
	): Promise<ShowObjectActionOutcome> {
		const wirePatch: UserLayoutPatch = {
			desks: patch.desks,
			active_desk_id: patch.activeDeskId,
			...(patch.windowSettings === undefined
				? {}
				: { window_settings: patch.windowSettings }),
		};
		return this.showObjectAction(
			`/api/v2/user-layouts/${encodeURIComponent(sessionId)}/update`,
			showId,
			{
				type: "update",
				expected_revision: revision,
				patch: wirePatch,
			},
		);
	}

	savePatchLayer(
		showId: string,
		layer: PatchLayerSaveInput,
		revision: number,
	): Promise<ShowObjectActionOutcome> {
		return this.showObjectAction(
			`/api/v2/patch/layers/${encodeURIComponent(layer.id)}/update`,
			showId,
			{
				type: "save",
				expected_revision: revision,
				layer: { name: layer.name, order: layer.order },
			},
		);
	}

	storePreload(
		showId: string,
		input: PreloadStoreInput,
		revision: number,
	): Promise<ShowObjectActionOutcome> {
		let action: PreloadRecordAction;
		if (input.target === "preset") {
			action = {
				type: "preset",
				target_id: input.target_id,
				expected_revision: revision,
				name: input.name ?? `Preset ${input.target_id}`,
				mode: input.mode ?? "merge",
				family: preloadFamily(input.family),
			};
		} else {
			const cueNumber = input.cue_number;
			if (
				cueNumber === undefined ||
				!/^(?:0|[1-9]\d*)(?:\.(?:0|[1-9]\d*))*$/.test(cueNumber)
			) {
				throw new Error(
					"A canonical dotted cue path is required when storing to a cue",
				);
			}
			action = {
				type: "cue",
				cue_list_id: input.target_id,
				expected_revision: revision,
				cue_number: cueNumber,
				name: input.name ?? null,
			};
		}
		return this.showObjectAction("/api/v2/preload/record", showId, action);
	}

	private outputRouteAction(
		showId: string,
		action: OutputRouteAction,
	): Promise<OutputRouteActionOutcome> {
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			action,
		});
		return this.transport
			.request("/api/v2/output-routes/actions", {
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			})
			.then((outcome) => outcome as OutputRouteActionOutcome);
	}

	private dynamicAction(
		path: string,
		showId: string,
		requestBody:
			| DynamicCreateActionRequest
			| DynamicPoolActionRequest
			| DynamicDeleteActionRequest
			| DynamicUpdateActionRequest,
	): Promise<ShowObjectActionOutcome> {
		const request = jsonRequest("POST", requestBody);
		return this.transport
			.request(path, {
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			})
			.then((outcome) => outcome as ShowObjectActionOutcome);
	}

	private showObjectAction(
		path: string,
		showId: string,
		action:
			| import("../generated/light-wire").UserLayoutAction
			| import("../generated/light-wire").PatchLayerAction
			| PreloadRecordAction,
	): Promise<ShowObjectActionOutcome> {
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			action,
		});
		return this.transport
			.request(path, {
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			})
			.then((outcome) => outcome as ShowObjectActionOutcome);
	}
}

function v2ShowObjectPath(kind: string, id: string): string {
	return `/api/v2/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`;
}

function showScopeHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}

function preloadFamily(family: PresetFamily | undefined) {
	switch (family) {
		case "Intensity":
			return "intensity" as const;
		case "Color":
			return "color" as const;
		case "Position":
			return "position" as const;
		case "Beam":
			return "beam" as const;
		default:
			return "mixed" as const;
	}
}
