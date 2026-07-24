import { describe, expect, it, vi } from "vitest";
import {
	SimulatedHardware,
	type SimulatedHardwareEndpoint,
} from "./hardwareScenario";

describe("simulated hardware acceptance intents", () => {
	it("subscribes and explicitly unsubscribes its owned client", async () => {
		const fixture = hardware();
		const controller = new SimulatedHardware(fixture.dependencies);

		await controller.connect("main", "controller-a");
		expect(controller.connected).toBe(true);
		expect(fixture.endpoint.subscribe).toHaveBeenCalledWith(
			"controller-a",
			"main",
		);

		await controller.disconnect();
		expect(controller.connected).toBe(false);
		expect(fixture.endpoint.unsubscribe).toHaveBeenCalledWith("controller-a");
		expect(fixture.endpoint.close).toHaveBeenCalledOnce();
	});

	it("is safe to disconnect twice", async () => {
		const fixture = hardware();
		const controller = new SimulatedHardware(fixture.dependencies);
		await controller.connect("main", "controller-a");

		await controller.disconnect();
		await controller.disconnect();

		expect(fixture.endpoint.unsubscribe).toHaveBeenCalledOnce();
		expect(fixture.endpoint.close).toHaveBeenCalledOnce();
	});

	it("closes an endpoint whose subscription fails", async () => {
		const fixture = hardware();
		fixture.endpoint.subscribe.mockRejectedValueOnce(new Error("rejected"));
		const controller = new SimulatedHardware(fixture.dependencies);

		await expect(controller.connect("main", "controller-a")).rejects.toThrow(
			"rejected",
		);
		expect(controller.connected).toBe(false);
		expect(fixture.endpoint.close).toHaveBeenCalledOnce();
	});

	it("unsubscribes before closing when global connection confirmation fails", async () => {
		const fixture = hardware();
		fixture.dependencies.connected = async () => false;
		const controller = new SimulatedHardware(fixture.dependencies, 0);

		await expect(controller.connect("main", "controller-a")).rejects.toThrow(
			/connected hardware/,
		);
		expect(fixture.endpoint.unsubscribe).toHaveBeenCalledWith("controller-a");
		expect(fixture.endpoint.close).toHaveBeenCalledOnce();
	});
});

function hardware() {
	const endpoint = {
		subscribe: vi.fn<SimulatedHardwareEndpoint["subscribe"]>(
			async () => undefined,
		),
		unsubscribe: vi.fn<SimulatedHardwareEndpoint["unsubscribe"]>(
			async () => undefined,
		),
		send: vi.fn<SimulatedHardwareEndpoint["send"]>(async () => undefined),
		mark: vi.fn<SimulatedHardwareEndpoint["mark"]>(() => 0),
		expectAfter: vi.fn<SimulatedHardwareEndpoint["expectAfter"]>(async () => ({
			address: "/feedback",
			arguments: [],
		})),
		close: vi.fn<SimulatedHardwareEndpoint["close"]>(async () => undefined),
	};
	return {
		endpoint,
		dependencies: {
			open: async () => endpoint,
			connected: async () => true,
		},
	};
}
