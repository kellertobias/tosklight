import type {
	ProgrammerCaptureModeEventMessage,
	ProgrammerCaptureModeScope,
} from "./contracts";

export interface ProgrammerCaptureModeEventObserver {
	message(message: ProgrammerCaptureModeEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ProgrammerCaptureModeEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A protocol error means local capture-mode authority must be repaired. */
export class ProgrammerCaptureModeProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "ProgrammerCaptureModeProtocolError";
	}
}

export interface ProgrammerCaptureModeEventTransport {
	subscribe(
		scope: ProgrammerCaptureModeScope,
		afterSequence: number | null,
		observer: ProgrammerCaptureModeEventObserver,
	): ProgrammerCaptureModeEventStream;
}
