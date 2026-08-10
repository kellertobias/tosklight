import { createContext, type PropsWithChildren, useContext } from "react";
import type { MacrosApiClient } from "../../api/client/macros";
import type { ShowObjectsApiClient } from "../../api/client/showObjects";
import type { ClientTransport } from "../../api/client/transport";
import { jsonRequest } from "../../api/client/transport";
import type {
	EventPayload,
	TimecodeTransportActionRequest,
	TimecodeTransportSnapshot,
} from "../../api/generated/light-wire";

export interface TimecodeRunningApi {
	runtime(showId: string): Promise<TimecodeTransportSnapshot[]>;
	stop(showId: string, timecodeId: string): Promise<TimecodeTransportSnapshot>;
}

export interface RunningRuntimeActions {
	macros: Pick<MacrosApiClient, "runtime" | "cancel">;
	timecodes: TimecodeRunningApi;
	showObjects: Pick<ShowObjectsApiClient, "objects">;
	events?: { onEvent(listener: (event: EventPayload) => void): () => unknown };
}

const RunningRuntimeActionsContext =
	createContext<RunningRuntimeActions | null>(null);

export function RunningRuntimeActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: RunningRuntimeActions }>) {
	return (
		<RunningRuntimeActionsContext.Provider value={actions}>
			{children}
		</RunningRuntimeActionsContext.Provider>
	);
}

export function useRunningRuntimeActions(): RunningRuntimeActions | null {
	return useContext(RunningRuntimeActionsContext);
}

export class TimecodeRunningApiClient implements TimecodeRunningApi {
	constructor(private readonly transport: ClientTransport) {}

	runtime(showId: string): Promise<TimecodeTransportSnapshot[]> {
		return this.transport.request("/api/v2/timecodes/runtime", {
			headers: showHeaders(showId),
		});
	}

	stop(showId: string, timecodeId: string): Promise<TimecodeTransportSnapshot> {
		const body: TimecodeTransportActionRequest = {
			timecode_id: timecodeId,
			action: { type: "stop" },
		};
		const request = jsonRequest("POST", body);
		return this.transport.request(
			`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/transport`,
			{
				...request,
				headers: { ...request.headers, ...showHeaders(showId) },
			},
		);
	}
}

function showHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}
