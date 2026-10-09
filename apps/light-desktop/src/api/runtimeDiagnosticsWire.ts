import type { RuntimeOutputDeliveryStatus } from "./generated/light-wire";

/** Older servers cannot prove current delivery loss; absence is never a failure. */
export function decodeOutputDeliveryStatus(
	value: unknown,
): readonly RuntimeOutputDeliveryStatus[] | null {
	if (value === undefined || value === null) return null;
	if (!Array.isArray(value))
		throw new Error("output_delivery_status must be an array");
	return value.map((entry, index) => {
		const path = `output_delivery_status[${index}]`;
		if (!entry || typeof entry !== "object" || Array.isArray(entry))
			throw new Error(`${path} must be an object`);
		const item = entry as Record<string, unknown>;
		if (item.protocol !== "art_net" && item.protocol !== "sacn")
			throw new Error(`${path}.protocol is invalid`);
		if (
			!Number.isSafeInteger(item.universe) ||
			(item.universe as number) < 0 ||
			(item.universe as number) > 65535
		)
			throw new Error(`${path}.universe is invalid`);
		if (typeof item.destination !== "string" || !item.destination)
			throw new Error(`${path}.destination is invalid`);
		if (
			!["sending", "send_failed", "awaiting_first_send"].includes(
				String(item.delivery_state),
			)
		)
			throw new Error(`${path}.delivery_state is invalid`);
		if (item.current_error !== null && typeof item.current_error !== "string")
			throw new Error(`${path}.current_error is invalid`);
		if (
			item.delivery_state === "send_failed"
				? !item.current_error
				: item.current_error !== null
		)
			throw new Error(`${path}.current_error does not match delivery_state`);
		return {
			protocol: item.protocol,
			universe: item.universe as number,
			destination: item.destination,
			delivery_state:
				item.delivery_state as RuntimeOutputDeliveryStatus["delivery_state"],
			current_error: item.current_error as string | null,
		};
	});
}
