import { describe, expect, it } from "vitest";
import vectors from "./psnRustVectors.json";
import {
	encodePsnDataFrame,
	encodePsnInfoPacket,
	PSN_MAX_PACKET_BYTES,
	type PsnTrackerData,
	psnChunk,
} from "./psnSender";

/**
 * Byte-for-byte parity with the Rust encoder. `psnRustVectors.json` was produced by calling
 * `light_psn_wire::encode_data_frame` / `encode_info_packet` (crates/shared/psn/src/encode.rs) with
 * exactly the inputs below, which are the ones `encode_tests.rs` uses.
 */
const hex = (packets: Buffer[]) => packets.map((packet) => packet.toString("hex"));

describe("PSN sender encoder", () => {
	it("writes the chunk header as id | length << 16 | has-subchunks << 31, little-endian", () => {
		expect(psnChunk(0x6755, true, Buffer.from([1, 2, 3])).toString("hex")).toBe("55670380010203");
		expect(psnChunk(0x0003, false, Buffer.alloc(0)).toString("hex")).toBe("03000000");
	});

	it("encodes a fully described tracker exactly as the Rust encoder", () => {
		const full: PsnTrackerData = {
			id: 6,
			position: [1, 2, 3],
			speed: [0.25, 0, -0.5],
			orientation: [0, 1.5, 0],
			validity: 0.75,
			acceleration: [0, -9.81, 0],
			targetPosition: [8, 0, 8],
			timestampMicros: 12_000,
		};
		expect(hex(encodePsnDataFrame(12_345, 7, [full]))).toEqual(vectors.fullFrame);
	});

	it("leaves out what the caller did not describe", () => {
		expect(hex(encodePsnDataFrame(0, 1, [{ id: 1, position: [4, 1, 2] }]))).toEqual(vectors.positionOnlyFrame);
	});

	it("still writes one packet for a frame with no trackers", () => {
		expect(hex(encodePsnDataFrame(1, 1, []))).toEqual(vectors.emptyFrame);
	});

	it("splits a frame past the 1500-byte cap at the same tracker as the Rust encoder", () => {
		const trackers = Array.from({ length: 120 }, (_, index): PsnTrackerData => ({
			id: index,
			position: [index, 1, 2],
		}));
		const packets = encodePsnDataFrame(99, 3, trackers);
		expect(packets.length).toBe(2);
		for (const packet of packets) expect(packet.length).toBeLessThanOrEqual(PSN_MAX_PACKET_BYTES);
		expect(hex(packets)).toEqual(vectors.splitFrame);
	});

	it("encodes an info packet with the system name and tracker names", () => {
		const packet = encodePsnInfoPacket({
			timestampMicros: 5,
			systemName: "Rehearsal sender",
			trackers: [{ id: 1, name: "Presenter" }, { id: 2 }],
		});
		expect(packet.toString("hex")).toBe(vectors.infoPacket);
	});
});
