import type {
	ProgrammerPriorityActionOutcome,
	ProgrammerPriorityActionRequest,
	ProgrammerPriorityErrorKind,
	ProgrammerPriorityEventMessage,
	ProgrammerPriorityScope,
	ProgrammerPrioritySnapshot,
} from "./contracts";

export interface ProgrammerPriorityEventObserver {
	message(message: ProgrammerPriorityEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerPriorityEventStream {
	close(): void;
	repair(cursor: number): void;
}

export interface ProgrammerPriorityTransport {
	loadSnapshot(
		scope: ProgrammerPriorityScope,
	): Promise<ProgrammerPrioritySnapshot>;
	applyAction(
		scope: ProgrammerPriorityScope,
		request: ProgrammerPriorityActionRequest,
	): Promise<ProgrammerPriorityActionOutcome>;
	subscribe(
		scope: ProgrammerPriorityScope,
		afterSequence: number | null,
		observer: ProgrammerPriorityEventObserver,
	): ProgrammerPriorityEventStream;
}

/** A protocol error requires one narrow priority snapshot repair. */
export class ProgrammerPriorityProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerPriorityProtocolError";
	}
}

export class ProgrammerPriorityTransportError extends Error {
	constructor(
		message: string,
		readonly kind: ProgrammerPriorityErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "ProgrammerPriorityTransportError";
	}
}
