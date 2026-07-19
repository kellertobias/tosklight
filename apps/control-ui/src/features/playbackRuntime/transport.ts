import type {
	PlaybackIdentity,
	PlaybackRuntimeEventMessage,
} from "./contracts";

export interface PlaybackEventScope {
	identities: readonly PlaybackIdentity[];
	desk: boolean;
}

export interface PlaybackEventObserver {
	message(message: PlaybackRuntimeEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface PlaybackEventStream {
	close(): void;
	repair(cursor: number): void;
}

export class PlaybackProtocolError extends Error {
	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "PlaybackProtocolError";
	}
}

export interface PlaybackEventTransport {
	subscribe(
		deskId: string,
		scope: PlaybackEventScope,
		afterSequence: number | null,
		observer: PlaybackEventObserver,
	): PlaybackEventStream;
}
