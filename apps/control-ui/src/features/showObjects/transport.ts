import type {
	ShowObjectKind,
	ShowObjectsEventMessage,
} from "./contracts";

export interface ShowObjectEventIdentity {
	kind: ShowObjectKind;
	objectId: string;
}

/** Union of kind-wide and exact-object routes required by the mounted views. */
export interface ShowObjectsEventScope {
	kinds: readonly ShowObjectKind[];
	objects: readonly ShowObjectEventIdentity[];
}

export interface ShowObjectsEventObserver {
	message(message: ShowObjectsEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ShowObjectsEventStream {
	close(): void;
	repair(cursor: number): void;
}

/** A decoded event violated the generated wire contract. */
export class ShowObjectsProtocolError extends Error {
	constructor(message: string, readonly eventSequence: number | null = null) {
		super(message);
		this.name = "ShowObjectsProtocolError";
	}
}

export interface ShowObjectsEventTransport {
	subscribe(
		showId: string,
		scope: ShowObjectsEventScope,
		afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	): ShowObjectsEventStream;
}
