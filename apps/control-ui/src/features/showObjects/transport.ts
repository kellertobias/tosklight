import type { ShowObjectsEventMessage } from "./contracts";

export interface ShowObjectsEventObserver {
	message(message: ShowObjectsEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface ShowObjectsEventStream {
	close(): void;
}

export interface ShowObjectsEventTransport {
	subscribe(
		showId: string,
		afterSequence: number | null,
		observer: ShowObjectsEventObserver,
	): ShowObjectsEventStream;
}
