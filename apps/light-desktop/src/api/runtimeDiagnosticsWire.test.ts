import { describe, expect, it } from "vitest";
import { decodeOutputDeliveryStatus } from "./runtimeDiagnosticsWire";

const failed = {
	protocol: "art_net",
	universe: 50,
	destination: "127.0.0.1:16454",
	delivery_state: "send_failed",
	current_error: "Permission denied",
};

describe("current output delivery wire", () => {
	it("does not treat missing older-server telemetry as a failure", () => {
		expect(decodeOutputDeliveryStatus(undefined)).toBeNull();
		expect(decodeOutputDeliveryStatus(null)).toBeNull();
	});
	it("accepts typed failure and explicit successful recovery with unknown extensions", () => {
		expect(decodeOutputDeliveryStatus([{ ...failed, future: true }])).toEqual([
			failed,
		]);
		expect(
			decodeOutputDeliveryStatus([
				{ ...failed, delivery_state: "sending", current_error: null },
			])?.[0].delivery_state,
		).toBe("sending");
	});
	it.each([
		{ ...failed, delivery_state: "guess_failed" },
		{ ...failed, current_error: null },
		{ ...failed, universe: -1 },
		{ ...failed, delivery_state: "sending" },
	])("rejects malformed or contradictory current delivery state", (entry) => {
		expect(() => decodeOutputDeliveryStatus([entry])).toThrow(
			/output_delivery_status/,
		);
	});
});
