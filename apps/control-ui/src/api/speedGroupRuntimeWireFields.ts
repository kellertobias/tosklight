import type { SpeedGroupEventMessage } from "../features/speedGroupRuntime/contracts";

export function outcomeFields(
	response: Record<string, unknown>,
	status: "changed" | "no_change",
) {
	return presentFields(response, [
		"request_id",
		"correlation_id",
		"authority_id",
		"revision",
		"applied_at_millis",
		"groups",
		"status",
		...(status === "changed" ? ["event_sequence"] : []),
		"replayed",
		"durability",
		"warning",
	]);
}

export function messageFields(type: SpeedGroupEventMessage["type"]) {
	if (type === "ready" || type === "repaired") return ["type", "cursor"];
	if (type === "error") return ["type", "error"];
	if (type === "gap") return ["type", "gap"];
	return ["type", "event"];
}

export function presentFields(
	value: Record<string, unknown>,
	fields: readonly string[],
) {
	return fields.filter((field) => field in value);
}
