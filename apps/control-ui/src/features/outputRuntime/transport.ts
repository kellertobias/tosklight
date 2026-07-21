import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
	OutputRuntimeErrorKind,
	OutputRuntimeEventMessage,
	OutputRuntimeScope,
	OutputRuntimeSnapshot,
} from "./contracts";

export interface OutputRuntimeEventObserver {
	message(message: OutputRuntimeEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface OutputRuntimeEventStream {
	close(): void;
	repair(cursor: number): void;
}

export interface OutputRuntimeTransport {
	loadSnapshot(scope: OutputRuntimeScope): Promise<OutputRuntimeSnapshot>;
	applyAction(
		scope: OutputRuntimeScope,
		request: OutputRuntimeActionRequest,
	): Promise<OutputRuntimeActionOutcome>;
	subscribe(
		scope: OutputRuntimeScope,
		afterSequence: number | null,
		observer: OutputRuntimeEventObserver,
	): OutputRuntimeEventStream;
}

/** A malformed or contradictory scoped message requires one narrow repair. */
export class OutputRuntimeProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "OutputRuntimeProtocolError";
	}
}

export class OutputRuntimeTransportError extends Error {
	constructor(
		message: string,
		readonly kind: OutputRuntimeErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "OutputRuntimeTransportError";
	}
}
