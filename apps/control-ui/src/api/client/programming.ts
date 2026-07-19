import type { PresetAddress } from "../../presetFamilies";
import type {
	AttributeValue,
	GeneratedFixturePresetResult,
	ProgrammerState,
} from "../types";
import type { LiveClientTransport } from "./transport";

type SelectionGestureSource =
	| { type: "fixture"; fixture_id: string }
	| { type: "live_group"; group_id: string }
	| { type: "dereferenced_group"; group_id: string };

interface ProgrammerAssignment {
	fixtureId: string;
	attribute: string;
	value: number;
}

export class ProgrammingApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	programmers(): Promise<ProgrammerState[]> {
		return this.transport.request("/api/v1/programmers", {}, false);
	}

	clearProgrammer(sessionId: string) {
		return this.transport.request(`/api/v1/programmers/${sessionId}/clear`, {
			method: "POST",
		});
	}

	clearProgrammerValues() {
		return this.transport.command("programmer.clear", {});
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

	preload(action: "enter" | "go" | "clear" | "release") {
		return this.transport.command(`preload.${action}`, {});
	}

	setPreloadGroup(groupId: string, attribute: string, value: number) {
		return this.transport.command("preload.group.set", {
			group_id: groupId,
			attribute,
			value,
		});
	}

	setProgrammer(fixtureId: string, attribute: string, value: number) {
		return this.transport.command("programmer.set", {
			fixture_id: fixtureId,
			attribute,
			value,
		});
	}

	setProgrammerMany(assignments: ProgrammerAssignment[]) {
		return this.transport.command("programmer.set_many", {
			assignments: assignments.map(({ fixtureId, attribute, value }) => ({
				fixture_id: fixtureId,
				attribute,
				value,
			})),
		});
	}

	setProgrammerValue(
		fixtureId: string,
		attribute: string,
		value: AttributeValue,
	) {
		return this.transport.command("programmer.set_value", {
			fixture_id: fixtureId,
			attribute,
			value,
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

	releaseProgrammer(fixtureId: string, attribute: string) {
		return this.transport.command("programmer.release", {
			fixture_id: fixtureId,
			attribute,
		});
	}

	setGroupProgrammer(
		groupId: string,
		attribute: string,
		value: number | AttributeValue,
	) {
		return this.transport.command("programmer.group.set", {
			group_id: groupId,
			attribute,
			value,
		});
	}

	releaseGroupProgrammer(groupId: string, attribute: string) {
		return this.transport.command("programmer.group.release", {
			group_id: groupId,
			attribute,
		});
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

	applyPreset(address: PresetAddress) {
		return this.transport.command("preset.apply", address);
	}
}
