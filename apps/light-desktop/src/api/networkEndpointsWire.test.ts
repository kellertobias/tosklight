import { describe, expect, it } from "vitest";
import { decodeNetworkEndpointsSnapshot } from "./networkEndpointsWire";

describe("network endpoints wire mapping", () => {
	it("maps the wire snapshot into the DMX diagnostics model", () => {
		expect(
			decodeNetworkEndpointsSnapshot({
				output_bind_ip: "10.0.0.2",
				network_output_available: false,
				endpoints: [
					{
						id: "send-1",
						protocol: "art_net",
						direction: "send",
						origin: "configured",
						role: "DMX output",
						software: null,
						endpoint: "10.0.0.255:6454",
						name: null,
						delivery_mode: "broadcast",
						logical_universe: 1,
						universes: [0],
						status: "unavailable",
						detail: "Network output could not start",
						errors: 2,
						last_activity_millis_ago: null,
					},
				],
			}),
		).toEqual({
			outputBindIp: "10.0.0.2",
			networkOutputAvailable: false,
			endpoints: [
				{
					id: "send-1",
					protocol: "art_net",
					direction: "send",
					origin: "configured",
					role: "DMX output",
					endpoint: "10.0.0.255:6454",
					name: null,
					software: null,
					deliveryMode: "broadcast",
					logicalUniverse: 1,
					universes: [0],
					status: "unavailable",
					detail: "Network output could not start",
					errors: 2,
					lastActivityMillisAgo: null,
				},
			],
		});
	});
});
