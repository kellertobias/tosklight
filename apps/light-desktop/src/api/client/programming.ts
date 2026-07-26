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
import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
} from "../../features/programmerPriority/contracts";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesActionRequest,
} from "../../features/programmerValues/contracts";
import type {
	CommandLineProjection,
	ProgrammingSnapshot,
	SelectionActionOutcome,
	SelectionActionRequest,
} from "../../features/programmingInteraction/contracts";
import type { PresetFamily } from "../../presetFamilies";
import type {
	GenerateFixturePresetsOutcome,
	GenerateFixturePresetsRequest,
	LiveAction,
} from "../generated/light-wire";
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
	decodeProgrammerPriorityActionOutcome,
	encodeProgrammerPriorityActionRequest,
} from "../programmerPriorityWire";
import {
	decodeProgrammerValuesActionOutcome,
	encodeProgrammerValuesActionRequest,
} from "../programmerValuesWire";
import {
	decodeSelectionActionOutcome,
	encodeSelectionActionRequest,
} from "../programmingSelectionWire";
import {
	decodeProgrammingCommandLine,
	decodeProgrammingInteractionSnapshot,
} from "../programmingWire";
import type { GeneratedFixturePresetResult } from "../types";
import type { LiveClientTransport } from "./transport";

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
		const value = await this.transport.sendAction(
			{
				type: "command_line_replace",
				request: {
					expected_revision: expectedRevision,
					text,
				},
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
		const value = await this.transport.sendAction(
			{ type: "programming_selection", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeSelectionActionOutcome(value, request.requestId);
	}

	async programmerValuesLiveAction(
		userId: string,
		request: ProgrammerValuesActionRequest,
	): Promise<ProgrammerValuesActionOutcome> {
		const wireRequest = encodeProgrammerValuesActionRequest(request);
		const value = await this.transport.sendAction(
			{ type: "programming_values", request: wireRequest },
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
		const value = await this.transport.sendAction(
			{ type: "programmer_priority", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeProgrammerPriorityActionOutcome(value, userId, request);
	}

	async presetRecallLiveAction(
		scope: PresetRecallScope,
		request: PresetRecallRequest,
	): Promise<PresetRecallOutcome> {
		const wireRequest = encodePresetRecallRequest(request);
		const value = await this.transport.sendAction(
			{
				type: "preset_recall",
				request: {
					request_id: request.requestId,
					show_id: scope.showId,
					request: wireRequest,
				},
			},
			request.requestId,
		);
		return decodePresetRecallOutcome(value, scope.userId, request);
	}

	async programmerPreloadLifecycleLiveAction(
		userId: string,
		request: ProgrammerPreloadLifecycleRequest,
	): Promise<ProgrammerPreloadLifecycleOutcome> {
		const wireRequest = encodeProgrammerPreloadLifecycleRequest(request);
		const value = await this.transport.sendAction(
			{ type: "programmer_preload_lifecycle", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeProgrammerPreloadLifecycleOutcome(value, userId, request);
	}

	async programmerPreloadValuesLiveAction(
		userId: string,
		request: ProgrammerPreloadValuesActionRequest,
	): Promise<ProgrammerPreloadValuesActionOutcome> {
		const wireRequest = encodeProgrammerPreloadValuesActionRequest(request);
		const value = await this.transport.sendAction(
			{ type: "programmer_preload_values", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeProgrammerPreloadValuesActionOutcome(
			value,
			userId,
			request.requestId,
		);
	}

	align(
		attribute: string,
		mode: "left" | "right" | "center" | "out",
		from = 0,
		to = 1,
	) {
		const requestId = crypto.randomUUID();
		return this.transport.sendAction(
			{
				type: "programming_align",
				request: {
					request_id: requestId,
					attribute,
					mode,
					from,
					to,
				},
			},
			requestId,
		);
	}

	controlFixtureAction(fixtureId: string, actionId: string, active: boolean) {
		const requestId = crypto.randomUUID();
		return this.transport.sendAction(
			{
				type: "fixture_control",
				request: {
					request_id: requestId,
					fixture_id: fixtureId,
					action_id: actionId,
					active,
				},
			},
			requestId,
		);
	}

	generateFixturePresets(
		fixtureIds: string[],
		expectedShowRevision: number,
	): Promise<GeneratedFixturePresetResult & { showRevision: number }> {
		const requestId = crypto.randomUUID();
		const request = {
			request_id: requestId,
			expected_show_revision: expectedShowRevision,
			fixture_ids: fixtureIds,
		} satisfies GenerateFixturePresetsRequest;
		return this.transport
			.request<GenerateFixturePresetsOutcome>(
				"/api/v2/preset-profile-generation/update",
				{
					method: "POST",
					headers: { "content-type": "application/json" },
					body: JSON.stringify(request),
				},
			)
			.then((outcome) => {
				if (outcome.request_id !== requestId)
					throw new Error("Preset generation returned a mismatched request ID");
				return {
					created: outcome.created.map((preset) => ({
						...preset,
						address: {
							...preset.address,
							family: legacyPresetFamily(preset.address.family),
						},
					})),
					showRevision: outcome.show_revision,
				};
			});
	}

	setCommandLine(value: string) {
		return this.send({
			type: "command_line_set",
			request: { value },
		});
	}

	setCommandTarget(value: "FIXTURE" | "GROUP") {
		return this.send({
			type: "command_target",
			request: { value },
		});
	}

	executeCommandLine(value: string) {
		return this.send({
			type: "command_line_execute",
			request: { value },
		});
	}

	undoProgrammer() {
		return this.send({ type: "programmer_undo" });
	}

	private send(action: LiveAction) {
		return this.transport.sendAction(action);
	}
}

function legacyPresetFamily(
	family: "mixed" | "intensity" | "color" | "position" | "beam",
): PresetFamily {
	switch (family) {
		case "mixed":
			return "Mixed";
		case "intensity":
			return "Intensity";
		case "color":
			return "Color";
		case "position":
			return "Position";
		case "beam":
			return "Beam";
	}
}
