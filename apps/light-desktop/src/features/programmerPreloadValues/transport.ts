import type {
	ProgrammerPreloadValuesEventMessage,
	ProgrammerPreloadValuesScope,
} from "./contracts";

export interface ProgrammerPreloadValuesEventObserver {
	message(message: ProgrammerPreloadValuesEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerPreloadValuesEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A protocol error means the local projection must be repaired from a snapshot. */
export class ProgrammerPreloadValuesProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerPreloadValuesProtocolError";
	}
}

export interface ProgrammerPreloadValuesEventTransport {
	subscribe(
		scope: ProgrammerPreloadValuesScope,
		afterSequence: number | null,
		observer: ProgrammerPreloadValuesEventObserver,
	): ProgrammerPreloadValuesEventStream;
}
