import type { PresetFamily } from "../../presetFamilies";
import type {
	OutputRoute,
	OutputRouteAction,
	OutputRouteActionOutcome,
	PreloadRecordAction,
	ShowObjectCollectionSnapshot,
	ShowObjectExactSnapshot,
	ShowObjectActionOutcome,
	UserLayoutPatch,
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

interface DynamicRecordInput {
	speed: number;
	width: number;
	direction: string;
	fixtureIds: string[];
	groupIds: string[];
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

	updateUserLayout(
		showId: string,
		userId: string,
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
			`/api/v2/user-layouts/${encodeURIComponent(userId)}/update`,
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

	recordDynamic(
		showId: string,
		cueListId: string,
		revision: number,
		input: DynamicRecordInput,
	): Promise<ShowObjectActionOutcome> {
		return this.showObjectAction(
			`/api/v2/cue-lists/${encodeURIComponent(cueListId)}/dynamics/record`,
			showId,
			{
				type: "append",
				expected_revision: revision,
				speed: input.speed,
				width: input.width,
				direction: input.direction === "Reverse" ? "reverse" : "forward",
				fixture_ids: input.fixtureIds,
				group_ids: input.groupIds,
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
			if (cueNumber === undefined || !Number.isFinite(cueNumber)) {
				throw new Error(
					"A finite cue number is required when storing to a cue",
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
		return this.transport.request(
			"/api/v2/output-routes/actions",
			{
				...request,
				headers: { ...request.headers, ...showScopeHeaders(showId) },
			},
		).then((outcome) => outcome as OutputRouteActionOutcome);
	}

	private showObjectAction(
		path: string,
		showId: string,
		action:
			| import("../generated/light-wire").UserLayoutAction
			| import("../generated/light-wire").PatchLayerAction
			| import("../generated/light-wire").DynamicRecordAction
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
