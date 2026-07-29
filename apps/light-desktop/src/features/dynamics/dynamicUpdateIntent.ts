import type {
	DynamicActivationPolicyProjection,
	DynamicDefinitionProjection,
	DynamicLaneProjection,
	DynamicPhaseDistributionProjection,
	DynamicRandomGroupProjection,
	DynamicSpeedProjection,
	DynamicTargetBindingProjection,
	DynamicUpdateIntent,
} from "../../api/types";

/**
 * Projects a Dynamic update exactly as the server will apply it so encoder
 * feedback can be immediate while the authoritative write is in flight.
 */
export function applyDynamicUpdateIntent(
	definition: DynamicDefinitionProjection,
	intent: DynamicUpdateIntent,
): DynamicDefinitionProjection {
	switch (intent.type) {
		case "set_name":
			return { ...definition, name: intent.name };
		case "set_color":
			return { ...definition, color: intent.color };
		case "set_icon":
			return { ...definition, icon: intent.icon };
		case "set_target_binding":
			return {
				...definition,
				target_binding:
					intent.target_binding as DynamicTargetBindingProjection,
			};
		case "add_lane": {
			const lanes = [...definition.lanes];
			const index = intent.index ?? lanes.length;
			lanes.splice(index, 0, intent.lane as DynamicLaneProjection);
			return { ...definition, lanes };
		}
		case "replace_lane":
			return {
				...definition,
				lanes: definition.lanes.map((lane) =>
					lane.id === intent.lane_id
						? (intent.lane as DynamicLaneProjection)
						: lane,
				),
			};
		case "delete_lane":
			return {
				...definition,
				lanes: definition.lanes.filter((lane) => lane.id !== intent.lane_id),
			};
		case "move_lane": {
			const lanes = [...definition.lanes];
			const current = lanes.findIndex((lane) => lane.id === intent.lane_id);
			if (current < 0) return definition;
			const [lane] = lanes.splice(current, 1);
			lanes.splice(Math.max(0, Math.min(intent.index, lanes.length)), 0, lane);
			return { ...definition, lanes };
		}
		case "set_phase":
			return {
				...definition,
				phase: intent.phase as DynamicPhaseDistributionProjection,
			};
		case "set_phase_mode":
			return {
				...definition,
				phase_mode: intent.phase_mode,
			};
		case "set_speed":
			return {
				...definition,
				speed: intent.speed as DynamicSpeedProjection,
			};
		case "set_overall_speed_multiplier":
			return { ...definition, overall_speed_multiplier: intent.multiplier };
		case "set_run_mode":
			return { ...definition, run_mode: intent.run_mode };
		case "set_activation":
			return {
				...definition,
				default_activation:
					intent.activation as DynamicActivationPolicyProjection,
			};
		case "set_activation_boundary":
			return { ...definition, activation_boundary: intent.boundary };
		case "add_random_group":
			return {
				...definition,
				random_groups: [
					...definition.random_groups,
					intent.group as DynamicRandomGroupProjection,
				],
			};
		case "replace_random_group":
			return {
				...definition,
				random_groups: definition.random_groups.map((group) =>
					group.id === intent.group_id
						? (intent.group as DynamicRandomGroupProjection)
						: group,
				),
			};
		case "delete_random_group":
			return {
				...definition,
				random_groups: definition.random_groups.filter(
					(group) => group.id !== intent.group_id,
				),
			};
	}
}
