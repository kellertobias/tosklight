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
	FileInputClaimRequest,
	FileInputReleaseRequest,
	ShowObjectActionOutcome,
	MacroDefinition as WireMacroDefinition,
	MacroValidation as WireMacroValidation,
} from "../generated/light-wire";
import {
	type MacroExecution,
	type MacroRuntime,
	macroExecutionFromWire,
} from "../runtimeModels";
import { jsonRequest, type LiveClientTransport } from "./transport";

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
			| "text"
			| "definition";
		expansion?: string | null;
	}>;
}

export interface MacroSuggestion {
	label: string;
	insertText: string;
	detail: string;
	replaceStart: number;
	replaceEnd: number;
}

export interface MacroValidation {
	valid: boolean;
	diagnostics: MacroLineDiagnostic[];
	suggestions: MacroSuggestion[];
}

export class MacrosApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	validate(
		showId: string,
		source: string,
		cursor?: number,
	): Promise<MacroValidation> {
		const body: MacroValidationRequest = { source, cursor };
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
			suggestions: (validation.suggestions ?? []).map((suggestion) => ({
				label: suggestion.label,
				insertText: suggestion.insert_text,
				detail: suggestion.detail,
				replaceStart: suggestion.replace_start,
				replaceEnd: suggestion.replace_end,
			})),
		}));
	}

	claimEditorInput(instanceId: string): Promise<void> {
		const body: FileInputClaimRequest = {
			request_id: crypto.randomUUID(),
			instance_id: instanceId,
			action: "macro_edit",
			origin: "editor",
		};
		return this.transport.request(
			"/api/v2/files/input-context/claim",
			jsonRequest("POST", body),
		);
	}

	releaseEditorInput(instanceId: string): Promise<void> {
		const body: FileInputReleaseRequest = {
			request_id: crypto.randomUUID(),
			instance_id: instanceId,
		};
		return this.transport.request(
			"/api/v2/files/input-context/release",
			jsonRequest("POST", body),
		);
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

	copy(
		showId: string,
		sourceMacroId: string,
		expectedRevision: number,
		poolNumber: number,
	) {
		return this.mutate(showId, {
			type: "copy",
			source_macro_id: sourceMacroId,
			expected_revision: expectedRevision,
			pool_number: poolNumber,
		});
	}

	run(
		showId: string,
		macroId: string,
		request: MacroRunActionRequest = {},
	): Promise<MacroExecution> {
		void showId;
		return this.transport
			.sendAction({
				type: "macro",
				request: {
					type: "run",
					macro_id: macroId,
					source_revision: request.source_revision,
					trigger: request.trigger ?? { type: "web_socket" },
				},
			})
			.then((snapshot) => macroExecutionFromWire(snapshot as MacroExecutionSnapshot));
	}

	runLine(
		showId: string,
		macroId: string,
		request: MacroRunLineActionRequest,
	): Promise<MacroExecution> {
		void showId;
		return this.transport
			.sendAction({
				type: "macro",
				request: {
					type: "run_line",
					macro_id: macroId,
					source_revision: request.source_revision,
					line: request.line,
				},
			})
			.then((snapshot) => macroExecutionFromWire(snapshot as MacroExecutionSnapshot));
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
