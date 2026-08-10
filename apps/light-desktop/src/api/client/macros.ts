import type {
	MacroCancelActionRequest,
	MacroExecutionSnapshot,
	MacroObjectAction,
	MacroObjectActionRequest,
	MacroRunActionRequest,
	MacroRunLineActionRequest,
	MacroRunLineUndoOutcome,
	MacroRuntimeSnapshot,
	MacroValidationRequest,
	ShowObjectActionOutcome,
	MacroDefinition as WireMacroDefinition,
	MacroValidation as WireMacroValidation,
} from "../generated/light-wire";
import {
	type MacroExecution,
	type MacroRuntime,
	macroExecutionFromWire,
} from "../runtimeModels";
import { type ClientTransport, jsonRequest } from "./transport";

export interface MacroDefinition {
	id: string;
	number: number;
	name: string;
	source: string;
	presentation: { color?: string | null; icon?: string | null };
}

export interface MacroLineDiagnostic {
	line: number;
	status: "valid" | "invalid" | "interaction_required";
	message: string;
	tokens: Array<{
		start: number;
		end: number;
		kind:
			| "keyword"
			| "target"
			| "operator"
			| "address"
			| "number"
			| "timing"
			| "comment"
			| "text";
	}>;
}

export interface MacroValidation {
	valid: boolean;
	diagnostics: MacroLineDiagnostic[];
}

export class MacrosApiClient {
	constructor(private readonly transport: ClientTransport) {}

	validate(showId: string, source: string): Promise<MacroValidation> {
		const body: MacroValidationRequest = { source };
		return this.request<WireMacroValidation>(
			"/api/v2/macros/validate",
			showId,
			body,
		).then((validation) => ({
			valid: validation.valid,
			diagnostics: validation.diagnostics.map((diagnostic) => ({
				...diagnostic,
				tokens: diagnostic.tokens.map((token) => ({ ...token })),
			})),
		}));
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
		return this.mutate(showId, {
			type: "create",
			definition: definition as WireMacroDefinition,
		});
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
	): Promise<MacroExecution> {
		return this.request<MacroExecutionSnapshot>(
			`/api/v2/macros/${encodeURIComponent(macroId)}/run`,
			showId,
			request,
		).then(macroExecutionFromWire);
	}

	runLine(
		showId: string,
		macroId: string,
		request: MacroRunLineActionRequest,
	): Promise<MacroExecution> {
		return this.request<MacroExecutionSnapshot>(
			`/api/v2/macros/${encodeURIComponent(macroId)}/run-line`,
			showId,
			request,
		).then(macroExecutionFromWire);
	}

	runtime(showId: string): Promise<MacroRuntime> {
		return this.transport
			.request<MacroRuntimeSnapshot>("/api/v2/macros/runtime", {
				headers: showHeaders(showId),
			})
			.then((snapshot) => ({
				desk_id: snapshot.desk_id,
				active: snapshot.active.map(macroExecutionFromWire),
				recent: snapshot.recent.map(macroExecutionFromWire),
			}));
	}

	execution(showId: string, executionId: string): Promise<MacroExecution> {
		return this.transport
			.request<MacroExecutionSnapshot>(
				`/api/v2/macros/executions/${encodeURIComponent(executionId)}`,
				{ headers: showHeaders(showId) },
			)
			.then(macroExecutionFromWire);
	}

	cancel(showId: string, executionId: string): Promise<MacroExecution> {
		const body: MacroCancelActionRequest = { execution_id: executionId };
		return this.request<MacroExecutionSnapshot>(
			"/api/v2/macros/executions/cancel",
			showId,
			body,
		).then(macroExecutionFromWire);
	}

	undoRunLine(
		showId: string,
		executionId: string,
	): Promise<MacroRunLineUndoOutcome> {
		return this.request(
			`/api/v2/macros/executions/${encodeURIComponent(executionId)}/undo-line`,
			showId,
			{},
		);
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
