import type {
	ShowObjectActionOutcome,
	TimecodeDefinition,
	TimecodeObjectAction,
	TimecodePatch,
	TimecodeTransportAction,
	TimecodeTransportSnapshot,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export interface TimecodeObjectRecord {
	revision: number;
	definition: TimecodeDefinition;
}

export interface TimecodeCollectionSnapshot {
	show_revision: number;
	objects: TimecodeObjectRecord[];
}

export class TimecodesApiClient {
	constructor(private readonly transport: ClientTransport) {}

	objects(showId: string): Promise<TimecodeCollectionSnapshot> {
		return this.transport.request("/api/v2/timecodes", {
			headers: showHeaders(showId),
		});
	}

	mutate(showId: string, action: TimecodeObjectAction) {
		return this.post<ShowObjectActionOutcome>("/api/v2/timecodes/actions", showId, {
			request_id: crypto.randomUUID(),
			action,
		});
	}

	create(showId: string, definition: TimecodeDefinition) {
		return this.mutate(showId, { type: "create", definition });
	}

	update(showId: string, timecodeId: string, expectedRevision: number, patch: TimecodePatch) {
		return this.mutate(showId, {
			type: "update",
			timecode_id: timecodeId,
			expected_revision: expectedRevision,
			patch,
		});
	}

	delete(showId: string, timecodeId: string, expectedRevision: number) {
		return this.mutate(showId, {
			type: "delete",
			timecode_id: timecodeId,
			expected_revision: expectedRevision,
		});
	}

	runtime(showId: string): Promise<TimecodeTransportSnapshot[]> {
		return this.transport.request("/api/v2/timecodes/runtime", { headers: showHeaders(showId) });
	}

	snapshot(showId: string, timecodeId: string): Promise<TimecodeTransportSnapshot> {
		return this.transport.request(`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/runtime`, {
			headers: showHeaders(showId),
		});
	}

	transportAction(showId: string, timecodeId: string, action: TimecodeTransportAction) {
		return this.post<TimecodeTransportSnapshot>(
			`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/transport`,
			showId,
			{ timecode_id: timecodeId, action },
		);
	}

	private post<T>(path: string, showId: string, body: unknown): Promise<T> {
		const request = jsonRequest("POST", body);
		return this.transport.request(path, {
			...request,
			headers: { ...request.headers, ...showHeaders(showId) },
		});
	}
}

function showHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}
