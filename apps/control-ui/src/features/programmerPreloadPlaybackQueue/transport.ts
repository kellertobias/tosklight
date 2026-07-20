import type {
	ProgrammerPreloadPlaybackQueueEventMessage,
	ProgrammerPreloadPlaybackQueueScope,
} from "./contracts";

export interface ProgrammerPreloadPlaybackQueueEventObserver {
	message(message: ProgrammerPreloadPlaybackQueueEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerPreloadPlaybackQueueEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A protocol error requires repair from the exact user's narrow snapshot. */
export class ProgrammerPreloadPlaybackQueueProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerPreloadPlaybackQueueProtocolError";
	}
}

/** The future authenticated adapter owns the exact-user object subscription. */
export interface ProgrammerPreloadPlaybackQueueEventTransport {
	subscribe(
		scope: ProgrammerPreloadPlaybackQueueScope,
		afterSequence: number | null,
		observer: ProgrammerPreloadPlaybackQueueEventObserver,
	): ProgrammerPreloadPlaybackQueueEventStream;
}
