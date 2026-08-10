import type {
	MacroCancelActionRequest,
	MacroDefinition,
	MacroExecutionSnapshot,
	MacroObjectAction,
	MacroObjectActionRequest,
	MacroRunActionRequest,
	MacroRunLineActionRequest,
	MacroRuntimeSnapshot,
	MacroValidation,
	MacroValidationRequest,
	ShowObjectActionOutcome,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export class MacrosApiClient {
	constructor(private readonly transport: ClientTransport) {}

	validate(showId: string, source: string): Promise<MacroValidation> {
		const body: MacroValidationRequest = { source };
		return this.request("/api/v2/macros/validate", showId, body);
	}

	mutate(
		showId: string,
		action: MacroObjectAction,
	): Promise<ShowObjectActionOutcome> {
		const body: MacroObjectActionRequest = {
			request_id: crypto.randomUUID(),
			action,
		};
		return this.request("/api/v2/macros/actions", showId, body);
	}

	create(showId: string, definition: MacroDefinition) {
		return this.mutate(showId, { type: "create", definition });
	}

	update(
		showId: string,
		macroId: string,
		expectedRevision: number,
		patch: Extract<MacroObjectAction, { type: "update" }>["patch"],
	) {
		return this.mutate(showId, {
			type: "update",
			macro_id: macroId,
			expected_revision: expectedRevision,
			patch,
		});
	}

	delete(showId: string, macroId: string, expectedRevision: number) {
		return this.mutate(showId, {
			type: "delete",
			macro_id: macroId,
			expected_revision: expectedRevision,
		});
	}

	run(
		showId: string,
		macroId: string,
		request: MacroRunActionRequest = {},
	): Promise<MacroExecutionSnapshot> {
		return this.request(
			`/api/v2/macros/${encodeURIComponent(macroId)}/run`,
			showId,
			request,
		);
	}

	runLine(
		showId: string,
		macroId: string,
		request: MacroRunLineActionRequest,
	): Promise<MacroExecutionSnapshot> {
		return this.request(
			`/api/v2/macros/${encodeURIComponent(macroId)}/run-line`,
			showId,
			request,
		);
	}

	runtime(showId: string): Promise<MacroRuntimeSnapshot> {
		return this.transport.request("/api/v2/macros/runtime", {
			headers: showHeaders(showId),
		});
	}

	execution(showId: string, executionId: string) {
		return this.transport.request<MacroExecutionSnapshot>(
			`/api/v2/macros/executions/${encodeURIComponent(executionId)}`,
			{ headers: showHeaders(showId) },
		);
	}

	cancel(showId: string, executionId: string) {
		const body: MacroCancelActionRequest = { execution_id: executionId };
		return this.request("/api/v2/macros/executions/cancel", showId, body);
	}

	private request<T>(path: string, showId: string, body: unknown): Promise<T> {
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
