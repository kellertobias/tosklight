import type {
	ScheduleCreateDefinition,
	ScheduleDeleteRequest,
	ScheduleDuplicateRequest,
	ScheduleMutationOutcome,
	SchedulePreview,
	SchedulePreviewRequest,
	ScheduleSnapshot,
	ScheduleUpdateRequest,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export type {
	ScheduleOccurrenceProjection,
	ScheduleProjection,
	ScheduleRuntimeChange,
	ScheduleTarget,
	ScheduleTrigger,
} from "../generated/light-wire";

export class SchedulesApiClient {
	constructor(private readonly transport: ClientTransport) {}

	snapshot(showId: string): Promise<ScheduleSnapshot> {
		return this.transport.request("/api/v2/schedules", {
			headers: showHeaders(showId),
		});
	}

	preview(
		showId: string,
		request: SchedulePreviewRequest,
		signal?: AbortSignal,
	): Promise<SchedulePreview> {
		return this.post("/api/v2/schedules/preview", showId, request, signal);
	}

	create(
		showId: string,
		definition: ScheduleCreateDefinition,
	): Promise<ScheduleMutationOutcome> {
		return this.post("/api/v2/schedules/create", showId, {
			request_id: crypto.randomUUID(),
			definition,
		});
	}

	update(
		showId: string,
		id: string,
		request: Omit<ScheduleUpdateRequest, "request_id">,
	): Promise<ScheduleMutationOutcome> {
		return this.post(
			`/api/v2/schedules/${encodeURIComponent(id)}/update`,
			showId,
			{ ...request, request_id: crypto.randomUUID() },
		);
	}

	duplicate(
		showId: string,
		id: string,
		expectedRevision: number,
	): Promise<ScheduleMutationOutcome> {
		const request: ScheduleDuplicateRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
		};
		return this.post(
			`/api/v2/schedules/${encodeURIComponent(id)}/duplicate`,
			showId,
			request,
		);
	}

	delete(
		showId: string,
		id: string,
		expectedRevision: number,
	): Promise<ScheduleMutationOutcome> {
		const request: ScheduleDeleteRequest = {
			request_id: crypto.randomUUID(),
			expected_revision: expectedRevision,
		};
		return this.post(
			`/api/v2/schedules/${encodeURIComponent(id)}/delete`,
			showId,
			request,
		);
	}

	private post<T>(
		path: string,
		showId: string,
		body: unknown,
		signal?: AbortSignal,
	): Promise<T> {
		const request = jsonRequest("POST", body);
		return this.transport.request<T>(path, {
			...request,
			headers: { ...request.headers, ...showHeaders(showId) },
			signal,
		});
	}
}

function showHeaders(showId: string) {
	return { "x-tosk-show": showId };
}
