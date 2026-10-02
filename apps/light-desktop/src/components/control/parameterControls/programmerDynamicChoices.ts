import type {
	DynamicDefinitionProjection,
	DynamicRuntimeControllerProjection,
	DynamicRuntimeInstanceProjection,
	DynamicRuntimeSnapshotProjection,
} from "../../../api/types";
import type { ProgrammerDynamicValue } from "../../../features/programmerValues/contracts";
import type { DynamicControllerChoice } from "./ProgrammerDynamicsInstanceContent";

export function dynamicChoices(
	runtime: DynamicRuntimeSnapshotProjection,
	definitions: readonly DynamicDefinitionProjection[],
	selectedFixtureIds: readonly string[],
	stagedValues: readonly ProgrammerDynamicValue[],
): DynamicControllerChoice[] {
	const selected = new Set(selectedFixtureIds);
	const running = runtime.instances
		.filter(
			(instance) =>
				selected.size === 0 ||
				instance.targets.some((target) => selected.has(target)),
		)
		.flatMap((instance) =>
			instance.controllers.map((controller) => ({
				instance,
				controller,
				definition:
					definitions.find(
						(definition) => definition.id === instance.dynamic_id,
					) ?? null,
			})),
		);
	const runningProgrammerLinks = new Set(
		running.filter(
			(choice) =>
				runtime.programmer_id !== undefined &&
				choice.controller.programmer_id === runtime.programmer_id,
		).map(
			(choice) =>
				choice.controller.programmer_instance_link ??
				choice.controller.controller_id,
		),
	);
	const stagedByInstance = new Map<
		string,
		{
			values: ProgrammerDynamicValue[];
			on: Extract<ProgrammerDynamicValue["value"], { type: "dynamic_on" }>;
		}
	>();
	for (const value of stagedValues) {
		if (value.value.type !== "dynamic_on") continue;
		if (selected.size > 0 && !selected.has(value.fixtureId)) continue;
		if (runningProgrammerLinks.has(value.value.instance_link)) continue;
		const group = stagedByInstance.get(value.value.instance_link);
		if (group) group.values.push(value);
		else
			stagedByInstance.set(value.value.instance_link, {
				values: [value],
				on: value.value,
			});
	}
	const staged = [...stagedByInstance.entries()].map(
		([instanceLink, { values, on }]) => {
			const definition =
				definitions.find(
					(candidate) =>
						candidate.id ===
						(on.dynamic.dynamic_id ?? on.dynamic.embedded_fallback.id),
				) ?? on.dynamic.embedded_fallback;
			const targets = [...new Set(values.map((value) => value.fixtureId))];
			const controllerProjection: DynamicRuntimeControllerProjection = {
				controller_id: instanceLink,
				...(runtime.programmer_id === undefined
					? {}
					: { programmer_id: runtime.programmer_id }),
				programmer_instance_link: instanceLink,
				source: "Programmer staged",
				priority: 0,
				size: on.overrides.size,
				speed_multiplier:
					on.overrides.speed_multiplier.numerator /
					on.overrides.speed_multiplier.denominator,
				phase_offset_degrees: on.overrides.phase_offset_degrees,
				paused: false,
				winning: false,
				releasing: false,
				activation_mix: 0,
			};
			const instanceProjection: DynamicRuntimeInstanceProjection = {
				instance_id: instanceLink,
				dynamic_id: definition.id,
				pool_number: on.dynamic.last_known_pool_number,
				name: definition.name,
				targets,
				pending: true,
				pending_until_millis: null,
				paused: false,
				speed_source: "Staged",
				activation_boundary: "beat",
				effective_cycle_millis: 0n,
				effective_bpm: null,
				beat_phase: null,
				phase_advancing: false,
				aliasing_warning: null,
				controllers: [controllerProjection],
			};
			return {
				instance: instanceProjection,
				controller: controllerProjection,
				definition,
			};
		},
	);
	return [...running, ...staged].sort(
		(left, right) =>
			Number(right.controller.winning) - Number(left.controller.winning) ||
			left.instance.pool_number - right.instance.pool_number ||
			left.controller.source.localeCompare(right.controller.source),
	);
}
