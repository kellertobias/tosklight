import { describe, expect, it } from "vitest";
import { currentOutputDiagnostics } from "./DeskStateDiagnosticsState";

describe("current desk output diagnostics", () => {
	it.each(["art_net", "sacn"])("keeps only actual current enabled %s send failure critical and retires on send recovery", (protocol) => {
		const route = {
			protocol,
			universe: 50,
			destination: "127.0.0.1:16454",
			enabled: true,
		};
		const failed = {
			...route,
			delivery_state: "send_failed" as const,
			current_error: "Permission denied",
		};
		expect(
			currentOutputDiagnostics({
				outputRoutes: [route],
				outputDeliveryStatus: [failed],
			})[0]?.capabilityLoss,
		).toBe("dmx_output");
		expect(
			currentOutputDiagnostics({
				outputRoutes: [route],
				outputDeliveryStatus: [
					{ ...failed, delivery_state: "sending", current_error: null },
				],
			}),
		).toEqual([]);
		expect(
			currentOutputDiagnostics({
				outputRoutes: [{ ...route, enabled: false }],
				outputDeliveryStatus: [failed],
			}),
		).toEqual([]);
		expect(
			currentOutputDiagnostics({
				outputRoutes: [],
				outputDeliveryStatus: [failed],
			}),
		).toEqual([]);
	});
	it("does not escalate historical errors without explicit current delivery failure", () => {
		const route = {
			protocol: "art_net",
			universe: 50,
			destination: "127.0.0.1:16454",
			enabled: true,
			send_errors: 99,
		};
		expect(currentOutputDiagnostics({ outputRoutes: [route] })).toEqual([]);
	});
	it("retires a cached failure when current route configuration disables or removes its target", () => {
		const route = {
			protocol: "art_net",
			universe: 50,
			destination: "127.0.0.1:16454",
			enabled: true,
			delivery_mode: "unicast",
		};
		const health = {
			outputRoutes: [route],
			outputDeliveryStatus: [
				{
					...route,
					delivery_state: "send_failed" as const,
					current_error: "Permission denied",
				},
			],
		};
		const configured = {
			kind: "output_route",
			id: "test-route",
			revision: 1,
			updated_at: "2026-10-08T16:00:00Z",
			body: {
				protocol: "art_net" as const,
				logical_universe: 1,
				destination_universe: 50,
				destination: route.destination,
				enabled: false,
				delivery_mode: "unicast" as const,
				minimum_slots: 512,
			},
		};
		expect(currentOutputDiagnostics(health, [configured])).toEqual([]);
		expect(currentOutputDiagnostics(health, [])).toEqual([]);
		expect(
			currentOutputDiagnostics(health, [
				{
					...configured,
					body: {
						...configured.body,
						enabled: true,
						destination: "127.0.0.1:16455",
					},
				},
			]),
		).toEqual([]);
	});
	it("reports only a currently enabled duplicate output target and address", () => {
		const route = {
			protocol: "art_net",
			universe: 7,
			delivery_mode: "unicast",
			destination: "10.0.0.17:6454",
			enabled: true,
		};
		const diagnostics = currentOutputDiagnostics({
			outputRoutes: [route, { ...route }],
		});

		expect(diagnostics).toEqual([
			expect.objectContaining({
				title: "Duplicate output · Art-Net universe 7 · 10.0.0.17:6454",
				summary:
					"2 enabled output routes send Art-Net universe 7 to 10.0.0.17:6454. The same output target and address must be owned by one route.",
				action:
					"Open Setup → Outputs and disable or remove the duplicate route for Art-Net universe 7 at 10.0.0.17:6454.",
			}),
		]);
	});

	it("clears the diagnostic as soon as the duplicate is disabled", () => {
		const route = {
			protocol: "sacn",
			universe: 2,
			destination: "239.255.0.2:5568",
			enabled: true,
		};
		expect(
			currentOutputDiagnostics({
				outputRoutes: [route, { ...route, enabled: false }],
			}),
		).toEqual([]);
	});

	it("explains current duplicate USB device routing without using cumulative drop counts", () => {
		const usbRoute = (id: string, logicalUniverse: number, enabled = true) => ({
			kind: "output_route",
			id,
			revision: 1,
			updated_at: "2026-08-11T00:00:00Z",
			body: {
				target: { kind: "usb_endpoint" as const, endpoint_id: "rear-dmx" },
				protocol: "art_net" as const,
				logical_universe: logicalUniverse,
				destination_universe: 1,
				delivery_mode: "broadcast" as const,
				destination: null,
				enabled,
				minimum_slots: 512,
			},
		});
		const duplicate = currentOutputDiagnostics({ outputRoutes: [] }, [
			usbRoute("route-a", 1),
			usbRoute("route-b", 4),
		]);
		expect(duplicate).toEqual([
			expect.objectContaining({
				capabilityLoss: "dmx_output",
				title: "Duplicate output · USB DMX device · universes 1, 4",
				summary:
					"The same USB DMX device is targeted by 2 enabled routes for logical universes 1, 4. One USB DMX device can output one logical universe, so the desk suppresses output instead of choosing one.",
				action:
					"Open Setup → Outputs and disable or remove the extra device routes. Keep only the intended logical universe for this USB DMX device.",
			}),
		]);
		const operatorCopy = [
			duplicate[0]?.title,
			duplicate[0]?.summary,
			duplicate[0]?.action,
		].join(" ");
		expect(operatorCopy).not.toContain("rear-dmx");
		expect(operatorCopy).not.toMatch(/endpoint|claim/iu);
		expect(
			currentOutputDiagnostics({ outputRoutes: [] }, [
				usbRoute("route-a", 1),
				usbRoute("route-b", 4, false),
			]),
		).toEqual([]);
	});
});
