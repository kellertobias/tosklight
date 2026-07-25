import type {
	ProgrammerValuesEventMessage,
	ProgrammerValuesScope,
} from "./contracts";

export interface ProgrammerValuesEventObserver {
	message(message: ProgrammerValuesEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerValuesEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A protocol error means the local projection must be repaired from a snapshot. */
export class ProgrammerValuesProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerValuesProtocolError";
	}
}

export interface ProgrammerValuesEventTransport {
	subscribe(
		scope: ProgrammerValuesScope,
		afterSequence: number | null,
		observer: ProgrammerValuesEventObserver,
	): ProgrammerValuesEventStream;
}
