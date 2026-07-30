import type {
	StageLayoutAction,
	StageLayoutActionOutcome as WireStageLayoutActionOutcome,
	StageProjection2d as WireStageProjection2d,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export type StageProjection2d =
	| "top_to_bottom"
	| "bottom_to_top"
	| "front_to_back"
	| "back_to_front"
	| "left_to_right"
	| "right_to_left";

export interface StageLayoutActionOutcome {
	request_id: string;
	revision: number;
	moved_fixture_ids: string[];
	replayed: boolean;
	changed: boolean;
}

export class StageLayoutApiClient {
	constructor(private readonly transport: ClientTransport) {}

	private action(
		showId: string,
		action: StageLayoutAction,
	): Promise<StageLayoutActionOutcome> {
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			action,
		});
		return this.transport
			.request<WireStageLayoutActionOutcome>("/api/v2/stage-layout/actions", {
				...request,
				headers: {
					...request.headers,
					"x-tosk-show": showId,
				},
			})
			.then(mapOutcome);
	}

	regenerate2d(
		showId: string,
		projection: StageProjection2d,
	): Promise<StageLayoutActionOutcome> {
		return this.action(showId, {
			type: "regenerate_2d",
			projection: projection satisfies WireStageProjection2d,
		});
	}
}

function mapOutcome(
	outcome: WireStageLayoutActionOutcome,
): StageLayoutActionOutcome {
	return {
		request_id: outcome.request_id,
		revision: outcome.revision,
		moved_fixture_ids: [...outcome.moved_fixture_ids],
		replayed: outcome.replayed,
		changed: outcome.changed,
	};
}
