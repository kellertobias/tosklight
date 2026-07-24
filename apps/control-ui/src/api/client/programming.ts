import type {
	CommandLineProjection,
	ProgrammingSnapshot,
	SelectionActionOutcome,
	SelectionActionRequest,
} from "../../features/programmingInteraction/contracts";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "../../features/programmerValues/contracts";
import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
} from "../../features/programmerPriority/contracts";
import type {
	PresetRecallOutcome,
	PresetRecallRequest,
	PresetRecallScope,
} from "../../features/presetRecall/contracts";
import type {
	ProgrammerPreloadLifecycleOutcome,
	ProgrammerPreloadLifecycleRequest,
} from "../../features/programmerPreloadLifecycle/contracts";
import type {
	ProgrammerPreloadValuesActionOutcome,
	ProgrammerPreloadValuesActionRequest,
} from "../../features/programmerPreloadValues/contracts";
import {
	decodeProgrammerPriorityActionOutcome,
	encodeProgrammerPriorityActionRequest,
} from "../programmerPriorityWire";
import {
	decodePresetRecallOutcome,
	encodePresetRecallRequest,
} from "../presetRecallWire";
import {
	decodeProgrammerPreloadLifecycleOutcome,
	encodeProgrammerPreloadLifecycleRequest,
} from "../programmerPreloadLifecycleWire";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	encodeProgrammerPreloadValuesActionRequest,
} from "../programmerPreloadValuesWire";
import {
	decodeSelectionActionOutcome,
	encodeSelectionActionRequest,
} from "../programmingSelectionWire";
import {
	decodeProgrammerValuesActionOutcome,
	encodeProgrammerValuesActionRequest,
} from "../programmerValuesWire";
import {
	decodeProgrammingCommandLine,
	decodeProgrammingInteractionSnapshot,
} from "../programmingWire";
import type { GeneratedFixturePresetResult } from "../types";
import type { LiveClientTransport } from "./transport";

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
			"/api/v2/programming-interaction/snapshot",
			{ headers: { "x-tosk-desk": deskId } },
		);
		return decodeProgrammingInteractionSnapshot(value, deskId);
	}

	async replaceProgrammingCommandLine(
		_deskId: string,
		text: string,
		expectedRevision: number,
	): Promise<CommandLineProjection> {
		const requestId = crypto.randomUUID();
		const value = await this.transport.commandWithRequestId(
			"programmer.command_line.replace",
			{
				request_id: requestId,
				expected_revision: expectedRevision,
				text,
			},
			requestId,
		);
		return decodeProgrammingCommandLine(value);
	}

	async applyProgrammingSelection(
		_deskId: string,
		request: SelectionActionRequest,
	): Promise<SelectionActionOutcome> {
		const wireRequest = encodeSelectionActionRequest(request);
		const value = await this.transport.commandWithRequestId(
			"programmer.selection.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeSelectionActionOutcome(value, request.requestId);
	}

	async programmerValuesLiveAction(
		userId: string,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome> {
		const wireRequest = encodeProgrammerValuesActionRequest(request);
		const value = await this.transport.commandWithRequestId(
			"programmer.values.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeProgrammerValuesActionOutcome(
			value,
			userId,
			request.requestId,
		);
	}

	async programmerPriorityLiveAction(
		userId: string,
		request: ProgrammerPriorityActionRequest,
	): Promise<ProgrammerPriorityActionOutcome> {
		const wireRequest = encodeProgrammerPriorityActionRequest(request);
		const value = await this.transport.commandWithRequestId(
			"programmer.priority.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeProgrammerPriorityActionOutcome(value, userId, request);
	}

	async presetRecallLiveAction(
		scope: PresetRecallScope,
		request: PresetRecallRequest,
	): Promise<PresetRecallOutcome> {
		const wireRequest = encodePresetRecallRequest(request);
		const value = await this.transport.commandWithRequestId(
			"preset.recall.action",
			{ show_id: scope.showId, request: wireRequest },
			wireRequest.request_id,
		);
		return decodePresetRecallOutcome(value, scope.userId, request);
	}

	async programmerPreloadLifecycleLiveAction(
		userId: string,
		request: ProgrammerPreloadLifecycleRequest,
	): Promise<ProgrammerPreloadLifecycleOutcome> {
		const wireRequest = encodeProgrammerPreloadLifecycleRequest(request);
		const value = await this.transport.commandWithRequestId(
			"programmer.preload.lifecycle.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeProgrammerPreloadLifecycleOutcome(value, userId, request);
	}

	async programmerPreloadValuesLiveAction(
		userId: string,
		request: ProgrammerPreloadValuesActionRequest,
	): Promise<ProgrammerPreloadValuesActionOutcome> {
		const wireRequest = encodeProgrammerPreloadValuesActionRequest(request);
		const value = await this.transport.commandWithRequestId(
			"programmer.preload.values.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeProgrammerPreloadValuesActionOutcome(
			value,
			userId,
			request.requestId,
		);
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
