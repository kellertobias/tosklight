import type { ApiDriver } from "./api";
import type { LightBench } from "./lightBench";
import type { OscHardware, OscMessage } from "./protocols";

export interface SimulatedHardwareEndpoint {
	subscribe(clientId: string, deskAlias: string): Promise<void>;
	unsubscribe(clientId: string): Promise<void>;
	send(
		address: string,
		arguments_?: Parameters<OscHardware["send"]>[1],
	): Promise<void>;
	mark(): number;
	expectAfter(mark: number, address: string): Promise<OscMessage>;
	close(): Promise<void>;
}

export interface SimulatedHardwareDependencies {
	open(): Promise<SimulatedHardwareEndpoint>;
	connected(): Promise<boolean>;
}

/**
 * Owns one simulated OSC controller subscription. The server's hardware_connected
 * flag is global, so disconnect deliberately verifies only this owned lifecycle.
 */
export class SimulatedHardware {
	private endpoint?: SimulatedHardwareEndpoint;
	private clientId?: string;

	constructor(
		private readonly dependencies: SimulatedHardwareDependencies,
		private readonly timeoutMillis = 2_000,
		private readonly defaultDeskAlias?: string,
	) {}

	get connected(): boolean {
		return this.endpoint !== undefined;
	}

	async connect(
		deskAlias = this.defaultDeskAlias,
		clientId = `e2e-${crypto.randomUUID()}`,
	): Promise<SimulatedHardwareEndpoint> {
		if (!deskAlias)
			throw new Error(
				"hardware.connect() needs a desk alias when no default desk is available",
			);
		if (this.endpoint)
			throw new Error("Simulated hardware is already connected");
		const endpoint = await this.dependencies.open();
		let subscribed = false;
		try {
			await endpoint.subscribe(clientId, deskAlias);
			subscribed = true;
			await this.waitForGlobalConnection();
		} catch (reason) {
			if (subscribed)
				await endpoint.unsubscribe(clientId).catch(() => undefined);
			await endpoint.close().catch(() => undefined);
			throw reason;
		}
		this.endpoint = endpoint;
		this.clientId = clientId;
		return endpoint;
	}

	async disconnect(): Promise<void> {
		const endpoint = this.endpoint;
		const clientId = this.clientId;
		if (!endpoint || !clientId) return;
		this.endpoint = undefined;
		this.clientId = undefined;
		try {
			await endpoint.unsubscribe(clientId);
		} finally {
			await endpoint.close();
		}
	}

	async send(address: string, arguments_?: Parameters<OscHardware["send"]>[1]) {
		if (!this.endpoint)
			throw new Error(
				"Simulated hardware is not connected; call hardware.connect() first",
			);
		await this.endpoint.send(address, arguments_);
	}

	mark(): number {
		if (!this.endpoint)
			throw new Error(
				"Simulated hardware is not connected; call hardware.connect() first",
			);
		return this.endpoint.mark();
	}

	expectAfter(mark: number, address: string): Promise<OscMessage> {
		if (!this.endpoint)
			throw new Error(
				"Simulated hardware is not connected; call hardware.connect() first",
			);
		return this.endpoint.expectAfter(mark, address);
	}

	private async waitForGlobalConnection(): Promise<void> {
		const deadline = Date.now() + this.timeoutMillis;
		do {
			if (await this.dependencies.connected()) return;
			await new Promise<void>((resolve) => setTimeout(resolve, 5));
		} while (Date.now() < deadline);
		throw new Error(
			"Timed out waiting for the desk to report connected hardware",
		);
	}
}

export function simulatedHardware(
	bench: Pick<LightBench, "osc">,
	api: ApiDriver,
): SimulatedHardware {
	return new SimulatedHardware(
		{
			open: () => bench.osc(),
			connected: async () =>
				(
					await api.request<{ hardware_connected: boolean }>(
						"GET",
						"/api/v2/bootstrap",
						undefined,
						false,
					)
				).hardware_connected,
		},
		2_000,
		api.session?.desk.osc_alias,
	);
}

export type ConnectedOscHardware = OscHardware;
