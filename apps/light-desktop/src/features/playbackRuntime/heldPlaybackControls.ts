import {
	assertPlaybackAddress,
	samePlaybackAddress,
	type PlaybackAddressValue,
} from "../../api/playbackAddress";

export interface PlaybackHold {
	address: PlaybackAddressValue;
	controlSource: { surface_id: string; control_id: number };
	holdToken: string;
}
export type PlaybackHoldSender = (
	hold: PlaybackHold,
	pressed: boolean,
) => Promise<unknown>;
interface CapturedHold {
	hold: PlaybackHold;
	send: PlaybackHoldSender;
}

/** Captures exact press identity; later page/layout changes never redirect releases. */
export class HeldPlaybackControls {
	private readonly held = new Map<string, CapturedHold>();
	private tail: Promise<unknown> = Promise.resolve();

	press(
		key: string,
		address: PlaybackAddressValue,
		controlSource: PlaybackHold["controlSource"],
		send: PlaybackHoldSender,
	): string {
		assertPlaybackAddress(address);
		const previous = this.held.get(key);
		if (
			previous &&
			samePlaybackAddress(previous.hold.address, address) &&
			previous.hold.controlSource.surface_id === controlSource.surface_id &&
			previous.hold.controlSource.control_id === controlSource.control_id
		)
			return previous.hold.holdToken;
		if (previous) this.release(key);
		const captured: CapturedHold = {
			hold: {
				address: { ...address },
				controlSource: { ...controlSource },
				holdToken: crypto.randomUUID(),
			},
			send,
		};
		this.held.set(key, captured);
		this.enqueue(captured, true);
		return captured.hold.holdToken;
	}

	release(key: string): void {
		const captured = this.held.get(key);
		if (!captured) return;
		this.held.delete(key);
		this.enqueue(captured, false);
	}

	releaseAll(): void {
		for (const key of [...this.held.keys()]) this.release(key);
	}

	settled(): Promise<unknown> {
		return this.tail;
	}

	private enqueue(captured: CapturedHold, pressed: boolean): void {
		const send = () => captured.send(captured.hold, pressed);
		this.tail = this.tail.then(send, send).catch(() => undefined);
	}
}
