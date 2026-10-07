import dgram, { type Socket } from "node:dgram";

/**
 * A PosiStageNet v2 sender for benches and manual testing.
 *
 * Mirrors `crates/shared/psn/src/encode.rs` byte for byte: every part of a packet is a chunk whose
 * little-endian 32-bit header carries the id (16 bits), the data length (15 bits) and the
 * has-subchunks bit; vectors are three little-endian f32 in metres; the packet header is a u64
 * timestamp, the version 2.0, the frame id and the frame's packet count. A frame that would exceed
 * the 1500-byte cap is split exactly as the Rust encoder splits it.
 *
 * Only erasable TypeScript is used so `tools/psn-sender.mjs` can import this file directly under
 * Node's type stripping.
 */

export const PSN_DEFAULT_GROUP = "236.10.10.10";
export const PSN_DEFAULT_PORT = 56565;
export const PSN_MAX_PACKET_BYTES = 1500;
export const PSN_VERSION_HIGH = 2;
export const PSN_VERSION_LOW = 0;

const PSN_DATA_PACKET = 0x6755;
const PSN_INFO_PACKET = 0x6756;
const PACKET_HEADER = 0x0000;
const INFO_SYSTEM_NAME = 0x0001;
const INFO_TRACKER_LIST = 0x0002;
const INFO_TRACKER_NAME = 0x0000;
const DATA_TRACKER_LIST = 0x0001;
const DATA_TRACKER_POS = 0x0000;
const DATA_TRACKER_SPEED = 0x0001;
const DATA_TRACKER_ORI = 0x0002;
const DATA_TRACKER_STATUS = 0x0003;
const DATA_TRACKER_ACCEL = 0x0004;
const DATA_TRACKER_TARGET_POS = 0x0005;
const DATA_TRACKER_TIMESTAMP = 0x0006;

/** A point or vector in the sender's own space: x right, y up, z depth, SI units. */
export type PsnVector3 = readonly [x: number, y: number, z: number];

export interface PsnTrackerData {
	id: number;
	position?: PsnVector3;
	speed?: PsnVector3;
	orientation?: PsnVector3;
	/** `PSN_DATA_TRACKER_STATUS`: the tracker's validity, 0..1. */
	validity?: number;
	acceleration?: PsnVector3;
	targetPosition?: PsnVector3;
	/** Per-tracker measurement time in microseconds. */
	timestampMicros?: bigint | number;
}

export interface PsnTrackerInfo {
	id: number;
	name?: string;
}

export interface PsnInfoPacket {
	timestampMicros?: bigint | number;
	frameId?: number;
	framePacketCount?: number;
	systemName?: string;
	trackers: PsnTrackerInfo[];
}

/** One chunk: its header word, then its data. Length is clamped to 15 bits like the Rust encoder. */
export function psnChunk(id: number, hasSubchunks: boolean, data: Uint8Array): Buffer {
	const length = Math.min(data.length, 0x7fff);
	const bytes = Buffer.alloc(4 + length);
	const header = ((id & 0xffff) | (length << 16) | (hasSubchunks ? 0x8000_0000 : 0)) >>> 0;
	bytes.writeUInt32LE(header, 0);
	bytes.set(data.subarray(0, length), 4);
	return bytes;
}

function u64(value: bigint | number): Buffer {
	const bytes = Buffer.alloc(8);
	bytes.writeBigUInt64LE(BigInt.asUintN(64, BigInt(value)));
	return bytes;
}

function headerChunk(timestampMicros: bigint | number, frameId: number, framePacketCount: number): Buffer {
	return psnChunk(
		PACKET_HEADER,
		false,
		Buffer.concat([
			u64(timestampMicros),
			Buffer.from([PSN_VERSION_HIGH, PSN_VERSION_LOW, frameId & 0xff, framePacketCount & 0xff]),
		]),
	);
}

function vectorChunk(id: number, vector: PsnVector3): Buffer {
	const data = Buffer.alloc(12);
	data.writeFloatLE(vector[0], 0);
	data.writeFloatLE(vector[1], 4);
	data.writeFloatLE(vector[2], 8);
	return psnChunk(id, false, data);
}

function trackerChunk(tracker: PsnTrackerData): Buffer {
	const body: Buffer[] = [];
	if (tracker.position) body.push(vectorChunk(DATA_TRACKER_POS, tracker.position));
	if (tracker.speed) body.push(vectorChunk(DATA_TRACKER_SPEED, tracker.speed));
	if (tracker.orientation) body.push(vectorChunk(DATA_TRACKER_ORI, tracker.orientation));
	if (tracker.validity !== undefined) {
		const status = Buffer.alloc(4);
		status.writeFloatLE(tracker.validity);
		body.push(psnChunk(DATA_TRACKER_STATUS, false, status));
	}
	if (tracker.acceleration) body.push(vectorChunk(DATA_TRACKER_ACCEL, tracker.acceleration));
	if (tracker.targetPosition) body.push(vectorChunk(DATA_TRACKER_TARGET_POS, tracker.targetPosition));
	if (tracker.timestampMicros !== undefined)
		body.push(psnChunk(DATA_TRACKER_TIMESTAMP, false, u64(tracker.timestampMicros)));
	return psnChunk(tracker.id, true, Buffer.concat(body));
}

/**
 * One frame as the datagrams a sender puts on the wire. Trackers are packed until the next one
 * would pass the 1500-byte cap; every packet carries the same frame id and the final count.
 */
export function encodePsnDataFrame(
	timestampMicros: bigint | number,
	frameId: number,
	trackers: readonly PsnTrackerData[],
): Buffer[] {
	const overhead = 4 + headerChunk(timestampMicros, frameId, 0).length + 4;
	const lists: Buffer[][] = [];
	let current: Buffer[] = [];
	let currentLength = 0;
	for (const tracker of trackers) {
		const encoded = trackerChunk(tracker);
		if (current.length > 0 && overhead + currentLength + encoded.length > PSN_MAX_PACKET_BYTES) {
			lists.push(current);
			current = [];
			currentLength = 0;
		}
		current.push(encoded);
		currentLength += encoded.length;
	}
	if (current.length > 0 || lists.length === 0) lists.push(current);
	const count = Math.min(lists.length, 0xff);
	return lists.map((list) =>
		psnChunk(
			PSN_DATA_PACKET,
			true,
			Buffer.concat([
				headerChunk(timestampMicros, frameId, count),
				psnChunk(DATA_TRACKER_LIST, true, Buffer.concat(list)),
			]),
		),
	);
}

/** One info packet: the sender's name and its trackers' names. */
export function encodePsnInfoPacket(packet: PsnInfoPacket): Buffer {
	const list = packet.trackers.map((tracker) =>
		psnChunk(tracker.id, true, psnChunk(INFO_TRACKER_NAME, false, Buffer.from(tracker.name ?? "", "utf8"))),
	);
	const body = [
		headerChunk(packet.timestampMicros ?? 0, packet.frameId ?? 0, Math.max(packet.framePacketCount ?? 0, 1)),
	];
	if (packet.systemName !== undefined)
		body.push(psnChunk(INFO_SYSTEM_NAME, false, Buffer.from(packet.systemName, "utf8")));
	body.push(psnChunk(INFO_TRACKER_LIST, true, Buffer.concat(list)));
	return psnChunk(PSN_INFO_PACKET, true, Buffer.concat(body));
}

export interface PsnSenderOptions {
	/** Destination address: a multicast group or a unicast host. */
	host?: string;
	port?: number;
	/** Local interface for multicast transmission. */
	multicastInterface?: string;
	/** Multicast TTL; 1 keeps the stream on the local segment. */
	ttl?: number;
	/** Whether this machine also receives its own multicast. */
	loopback?: boolean;
	systemName?: string;
}

/**
 * A UDP socket that speaks PSN to one destination. Frame ids advance per frame and wrap at 255;
 * the header timestamp is microseconds since the sender started, as a real tracking server's is.
 */
export class PsnSender {
	readonly host: string;
	readonly port: number;
	readonly systemName: string;
	private frameId = 0;
	private readonly startedAt = process.hrtime.bigint();
	private readonly socket: Socket;

	private constructor(socket: Socket, options: PsnSenderOptions) {
		this.socket = socket;
		this.host = options.host ?? PSN_DEFAULT_GROUP;
		this.port = options.port ?? PSN_DEFAULT_PORT;
		this.systemName = options.systemName ?? "ToskLight bench PSN sender";
	}

	static async open(options: PsnSenderOptions = {}): Promise<PsnSender> {
		const socket = dgram.createSocket({ type: "udp4", reuseAddr: true });
		await new Promise<void>((resolve, reject) => {
			socket.once("error", reject);
			socket.bind(0, () => {
				socket.off("error", reject);
				resolve();
			});
		});
		socket.setMulticastTTL(options.ttl ?? 1);
		socket.setMulticastLoopback(options.loopback ?? true);
		if (options.multicastInterface) socket.setMulticastInterface(options.multicastInterface);
		return new PsnSender(socket, options);
	}

	/** Microseconds since this sender opened. */
	timestampMicros(): bigint {
		return (process.hrtime.bigint() - this.startedAt) / 1000n;
	}

	/** Send one data frame (one or more datagrams). Returns the frame id used. */
	async sendFrame(trackers: readonly PsnTrackerData[]): Promise<number> {
		this.frameId = (this.frameId + 1) & 0xff;
		const frameId = this.frameId;
		for (const datagram of encodePsnDataFrame(this.timestampMicros(), frameId, trackers))
			await this.sendRaw(datagram);
		return frameId;
	}

	/** Send one info packet naming this sender and its trackers. */
	async sendInfo(trackers: readonly PsnTrackerInfo[]): Promise<void> {
		await this.sendRaw(
			encodePsnInfoPacket({
				timestampMicros: this.timestampMicros(),
				frameId: this.frameId,
				systemName: this.systemName,
				trackers: [...trackers],
			}),
		);
	}

	/** Any datagram to the same destination, e.g. a foreign protocol for ingress tests. */
	sendRaw(datagram: Uint8Array): Promise<void> {
		return new Promise((resolve, reject) => {
			this.socket.send(datagram, this.port, this.host, (error: Error | null) => (error ? reject(error) : resolve()));
		});
	}

	close(): Promise<void> {
		return new Promise((resolve) => {
			try {
				this.socket.close(() => resolve());
			} catch {
				resolve();
			}
		});
	}
}

/**
 * Keeps a set of trackers on the wire at a steady rate, as a tracking server does, until stopped.
 * `update` replaces what the next frame carries; `stop` is the "sender switched off" of the
 * scenarios.
 */
export class PsnStream {
	private timer?: ReturnType<typeof setInterval>;
	private infoTimer?: ReturnType<typeof setInterval>;
	private frames: PsnTrackerData[][] = [[]];
	private frameIndex = 0;
	private names: PsnTrackerInfo[] = [];
	private sending: Promise<unknown> = Promise.resolve();
	readonly sender: PsnSender;
	private readonly intervalMillis: number;

	constructor(sender: PsnSender, intervalMillis = 1000 / 60) {
		this.sender = sender;
		this.intervalMillis = intervalMillis;
	}

	update(trackers: readonly PsnTrackerData[], names?: readonly PsnTrackerInfo[]): void {
		this.cycle([trackers], names);
	}

	/** Send these frames in turn, one per tick, repeating: a marker jittering between positions. */
	cycle(frames: readonly (readonly PsnTrackerData[])[], names?: readonly PsnTrackerInfo[]): void {
		this.frames = frames.map((trackers) => trackers.map((tracker) => ({ ...tracker })));
		this.frameIndex = 0;
		if (names) this.names = [...names];
	}

	private nextFrame(): PsnTrackerData[] {
		const frame = this.frames[this.frameIndex % this.frames.length] ?? [];
		this.frameIndex += 1;
		return frame;
	}

	get running(): boolean {
		return this.timer !== undefined;
	}

	start(): void {
		if (this.timer) return;
		const frame = () => {
			this.sending = this.sending.then(() => this.sender.sendFrame(this.nextFrame())).catch(() => undefined);
		};
		const info = () => {
			if (this.names.length === 0) return;
			this.sending = this.sending.then(() => this.sender.sendInfo(this.names)).catch(() => undefined);
		};
		info();
		frame();
		this.timer = setInterval(frame, this.intervalMillis);
		this.infoTimer = setInterval(info, 1000);
	}

	async stop(): Promise<void> {
		if (this.timer) clearInterval(this.timer);
		if (this.infoTimer) clearInterval(this.infoTimer);
		this.timer = undefined;
		this.infoTimer = undefined;
		await this.sending;
	}

	async close(): Promise<void> {
		await this.stop();
		await this.sender.close();
	}
}
