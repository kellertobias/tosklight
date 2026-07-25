import type { ProgrammingInteractionEventMessage } from "./contracts";

export interface ProgrammingEventScope {
	commandLine: boolean;
	selection: boolean;
}

export interface ProgrammingEventObserver {
	message(message: ProgrammingInteractionEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammingEventStream {
	close(): void;
	repair(cursor: number): void;
}

export class ProgrammingProtocolError extends Error {
	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammingProtocolError";
	}
}

export interface ProgrammingEventTransport {
	subscribe(
		deskId: string,
		scope: ProgrammingEventScope,
		afterSequence: number | null,
		observer: ProgrammingEventObserver,
	): ProgrammingEventStream;
}
