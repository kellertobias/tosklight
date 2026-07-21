import type {
	CommandLineProjection,
	ProgrammingSnapshot,
	SelectionActionOutcome,
	SelectionActionRequest,
} from "../../features/programmingInteraction/contracts";
import {
	decodeSelectionActionOutcome,
	encodeSelectionActionRequest,
} from "../programmingSelectionWire";
import {
	decodeProgrammingCommandLine,
	decodeProgrammingInteractionSnapshot,
} from "../programmingWire";
import type { GeneratedFixturePresetResult, ProgrammerState } from "../types";
import type { LiveClientTransport } from "./transport";
import { jsonRequest } from "./transport";

type SelectionGestureSource =
	| { type: "fixture"; fixture_id: string }
	| { type: "live_group"; group_id: string }
	| { type: "dereferenced_group"; group_id: string };

export class ProgrammingApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	async programmingInteractionSnapshot(
		deskId: string,
	): Promise<ProgrammingSnapshot> {
		const value = await this.transport.request<unknown>(
			`/api/v2/desks/${encodeURIComponent(deskId)}/programming-interaction/snapshot`,
		);
		return decodeProgrammingInteractionSnapshot(value, deskId);
	}

	async replaceProgrammingCommandLine(
		deskId: string,
		text: string,
		expectedRevision: number,
	): Promise<CommandLineProjection> {
		const init = jsonRequest("PUT", { text });
		const headers = new Headers(init.headers);
		headers.set("if-match", String(expectedRevision));
		const value = await this.transport.request<unknown>(
			`/api/v2/desks/${encodeURIComponent(deskId)}/command-line`,
			{ ...init, headers },
		);
		return decodeProgrammingCommandLine(value);
	}

	async applyProgrammingSelection(
		deskId: string,
		request: SelectionActionRequest,
	): Promise<SelectionActionOutcome> {
		const value = await this.transport.request<unknown>(
			`/api/v2/desks/${encodeURIComponent(deskId)}/programming-selection/actions`,
			jsonRequest("POST", encodeSelectionActionRequest(request)),
		);
		return decodeSelectionActionOutcome(value, request.requestId);
	}

	programmers(): Promise<ProgrammerState[]> {
		return this.transport.request("/api/v1/programmers");
	}

	clearProgrammer(sessionId: string) {
		return this.transport.request(`/api/v1/programmers/${sessionId}/clear`, {
			method: "POST",
		});
	}

	selectGroup(
		groupId: string,
		frozen = false,
		rule: Record<string, unknown> = { type: "all" },
	) {
		return this.transport.command("group.select", {
			group_id: groupId,
			frozen,
			rule,
		});
	}

	selectionMacro(rule: Record<string, unknown>) {
		return this.transport.command("selection.macro", { rule });
	}

	align(
		attribute: string,
		mode: "left" | "right" | "center" | "out",
		from = 0,
		to = 1,
	) {
		return this.transport.command("programmer.align", {
			attribute,
			mode,
			from,
			to,
		});
	}

	controlFixtureAction(fixtureId: string, actionId: string, active: boolean) {
		return this.transport.command("programmer.control_action", {
			fixture_id: fixtureId,
			action_id: actionId,
			active,
		});
	}

	generateFixturePresets(
		fixtureIds: string[],
	): Promise<GeneratedFixturePresetResult> {
		return this.transport.command("preset.generate_fixture_values", {
			fixture_ids: fixtureIds,
		}) as Promise<GeneratedFixturePresetResult>;
	}

	setGroupMaster(groupId: string, value: number) {
		return this.transport.command("group.master.set", {
			group_id: groupId,
			value,
		});
	}

	setGroupMasterFlash(groupId: string, value: number) {
		return this.transport.command("group.master.flash", {
			group_id: groupId,
			value,
		});
	}

	setSelection(fixtures: string[]) {
		return this.transport.command("selection.set", { fixtures });
	}

	selectionGesture(source: SelectionGestureSource, remove = false) {
		return this.transport.command("selection.gesture", { source, remove });
	}

	setCommandLine(value: string) {
		return this.transport.command("programmer.command_line", { value });
	}

	setCommandTarget(value: "FIXTURE" | "GROUP") {
		return this.transport.command("programmer.command_target", { value });
	}

	executeCommandLine(value: string) {
		return this.transport.command("programmer.execute", { value });
	}

	undoProgrammer() {
		return this.transport.command("programmer.undo", {});
	}
}
