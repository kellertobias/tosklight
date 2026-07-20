import type { ProgrammerLifecycleEventMessage } from "./contracts";

export interface ProgrammerLifecycleEventObserver {
	message(message: ProgrammerLifecycleEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerLifecycleEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A protocol error requires a narrow lifecycle snapshot repair. */
export class ProgrammerLifecycleProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerLifecycleProtocolError";
	}
}

/** The future authenticated adapter owns the exact aggregate subscription. */
export interface ProgrammerLifecycleEventTransport {
	subscribe(
		afterSequence: number | null,
		observer: ProgrammerLifecycleEventObserver,
	): ProgrammerLifecycleEventStream;
}
