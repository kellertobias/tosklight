import type {
	DynamicLaneProjection,
	DynamicRandomGroupProjection,
} from "../../api/types";

export type ScalarDynamicLane = Extract<
	DynamicLaneProjection,
	{ attribute: string }
>;
export type ProgrammingDynamicLane = Exclude<
	DynamicLaneProjection,
	ScalarDynamicLane
>;
export type ScalarDynamicRandomGroup = Extract<
	DynamicRandomGroupProjection,
	{ low: unknown }
>;

export function isScalarDynamicLane(
	lane: DynamicLaneProjection,
): lane is ScalarDynamicLane {
	return "attribute" in lane;
}

export function isScalarDynamicRandomGroup(
	group: DynamicRandomGroupProjection,
): group is ScalarDynamicRandomGroup {
	return "low" in group;
}

export function dynamicLaneMode(lane: DynamicLaneProjection) {
	return isScalarDynamicLane(lane)
		? lane.mode
		: lane.programming.configuration.mode;
}

export function dynamicLaneOwner(lane: DynamicLaneProjection): string {
	if (isScalarDynamicLane(lane)) return lane.attribute;
	switch (lane.programming.address.representation.kind) {
		case "angles":
		case "target":
			return "position";
		case "semantic_color":
		case "direct_color":
			return "color";
		case "focus":
			return "focus";
		case "zoom":
			return "zoom";
	}
}

export function dynamicLaneLabel(lane: DynamicLaneProjection): string {
	if (isScalarDynamicLane(lane)) return lane.attribute;
	const { component } = lane.programming.address;
	if (!component) return `${dynamicLaneOwner(lane)} · complete intent`;
	if (component.kind === "native_color")
		return `Color · native ${component.component.channel_id.slice(0, 8)}`;
	return component.kind === "color"
		? `Color · ${component.component.replaceAll("_", " ")}`
		: component.kind.replaceAll("_", " ");
}
