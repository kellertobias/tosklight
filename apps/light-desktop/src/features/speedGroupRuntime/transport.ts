import type {
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
	SpeedGroupErrorKind,
	SpeedGroupEventMessage,
	SpeedGroupRuntimeScope,
	SpeedGroupSnapshot,
} from "./contracts";

export interface SpeedGroupEventObserver {
	message(message: SpeedGroupEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface SpeedGroupEventStream {
	close(): void;
	repair(cursor: number): void;
}

export interface SpeedGroupRuntimeTransport {
	loadSnapshot(scope: SpeedGroupRuntimeScope): Promise<SpeedGroupSnapshot>;
	applyAction(
		scope: SpeedGroupRuntimeScope,
		request: SpeedGroupActionRequest,
	): Promise<SpeedGroupActionOutcome>;
	subscribe(
		scope: SpeedGroupRuntimeScope,
		afterSequence: number | null,
		observer: SpeedGroupEventObserver,
	): SpeedGroupEventStream;
}

export class SpeedGroupProtocolError extends Error {
	readonly requiresRepair = true;

	constructor(
		message: string,
		readonly eventSequence: number | null = null,
	) {
		super(message);
		this.name = "SpeedGroupProtocolError";
	}
}

export class SpeedGroupTransportError extends Error {
	constructor(
		message: string,
		readonly kind: SpeedGroupErrorKind,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "SpeedGroupTransportError";
	}
}
