import type { LightBench } from "../core/lightBench";
import type { DmxPacket, DmxProtocol } from "../core/protocols";

export type OutputPacketAssertion = (
	packet: Readonly<DmxPacket>,
) => unknown | Promise<unknown>;

interface PacketReceiver {
	readonly packets: readonly DmxPacket[];
	nextAfter(
		mark: number,
		protocol: DmxProtocol,
		universe: number,
		timeout?: number,
	): Promise<DmxPacket>;
}

interface OutputPacketSource {
	readonly artnet: PacketReceiver;
	readonly sacn: PacketReceiver;
}

/** Wire-level Art-Net/sACN observations, deliberately separate from logical DMX. */
export class BrowserOutputPackets {
	constructor(private readonly source: OutputPacketSource) {}

	async expect(
		protocol: DmxProtocol,
		universe: number,
		assertion: OutputPacketAssertion,
	): Promise<void> {
		if (!Number.isInteger(universe) || universe < 0 || universe > 65_535)
			throw new Error(
				"Output packet universe must be an integer from 0 through 65535",
			);
		const receiver =
			protocol === "artnet" ? this.source.artnet : this.source.sacn;
		const deadline = Date.now() + 2_000;
		let packet: Readonly<DmxPacket> | undefined;
		let assertionError: unknown;
		do {
			packet = [...receiver.packets]
				.reverse()
				.find(
					(candidate) =>
						candidate.protocol === protocol && candidate.universe === universe,
				);
			if (!packet) {
				packet = await receiver.nextAfter(0, protocol, universe);
			}
			try {
				const accepted = await assertion(packet);
				if (accepted === false)
					throw new Error("packet assertion returned false");
				return;
			} catch (error) {
				assertionError = error;
			}
			await new Promise((resolve) => setTimeout(resolve, 5));
		} while (Date.now() < deadline);
		throw new Error(
			`${protocol === "artnet" ? "Art-Net" : "sACN"} universe ${universe} packet assertion failed; sequence ${packet.sequence}, received ${new Date(packet.receivedAt).toISOString()}, ${packet.slots.length} slots, first 32 bytes ${JSON.stringify(Array.from(packet.slots.slice(0, 32)))}`,
			{ cause: assertionError },
		);
	}
}

export function browserOutputPackets(bench: LightBench): BrowserOutputPackets {
	return new BrowserOutputPackets(bench);
}
