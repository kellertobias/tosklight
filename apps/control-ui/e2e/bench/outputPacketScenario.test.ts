import { describe, expect, it, vi } from "vitest";
import {
	BrowserOutputPackets,
	type OutputPacketAssertion,
} from "./outputPacketScenario";
import type { DmxPacket, DmxProtocol } from "./protocols";

describe("wire output packet observations", () => {
	it("asserts latest Art-Net and sACN packets without reading logical DMX", async () => {
		const artnet = packet("artnet", 1, [10, 20]);
		const sacn = packet("sacn", 101, [30, 40]);
		const output = new BrowserOutputPackets({
			artnet: receiver([artnet]),
			sacn: receiver([sacn]),
		});
		await output.expect("artnet", 1, (observed) => {
			expect(Array.from(observed.slots)).toEqual([10, 20]);
		});
		await output.expect("sacn", 101, (observed) => observed.slots[0] === 30);
	});

	it("waits when no matching packet exists and reports wire evidence on failure", async () => {
		const expected = packet("artnet", 4, [77]);
		const artnet = receiver([]);
		artnet.nextAfter.mockResolvedValue(expected);
		const output = new BrowserOutputPackets({
			artnet,
			sacn: receiver([]),
		});
		const reject: OutputPacketAssertion = () => false;
		await expect(output.expect("artnet", 4, reject)).rejects.toThrow(
			"Art-Net universe 4 packet assertion failed; sequence 7",
		);
		expect(artnet.nextAfter).toHaveBeenCalledWith(0, "artnet", 4);
	});
});

function packet(
	protocol: DmxProtocol,
	universe: number,
	slots: number[],
): DmxPacket {
	return {
		protocol,
		universe,
		sequence: 7,
		slots: Uint8Array.from(slots),
		receivedAt: Date.parse("2020-01-01T00:00:00Z"),
	};
}

function receiver(packets: DmxPacket[]) {
	return {
		packets,
		nextAfter: vi.fn(async () => {
			const next = packets[0];
			if (!next) throw new Error("No packet");
			return next;
		}),
	};
}
