import type {
	PatchEventMessage,
	PatchFixturePolicyAction,
	PatchFixtureUpdateAction,
	PatchMutation,
	PatchMutationOutcome,
	PatchSnapshot,
} from "./contracts";

export interface PatchEventObserver {
	message(message: PatchEventMessage): void;
	error(error: Error): void;
	closed(): void;
}

export interface PatchEventStream {
	repair(cursor: number): void;
	close(): void;
}

/** Port consumed by the Patch feature. Concrete HTTP/WebSocket code lives in api/. */
export interface PatchTransport {
	snapshot(showId: string): Promise<PatchSnapshot>;
	patchFixtures(
		showId: string,
		expectedPatchRevision: number,
		mutation: PatchMutation,
	): Promise<PatchMutationOutcome>;
	patchFixturePolicy?(
		showId: string,
		fixtureId: string,
		expectedPatchRevision: number,
		requestId: string,
		action: PatchFixturePolicyAction,
	): Promise<PatchMutationOutcome>;
	patchFixtureUpdate?(
		showId: string,
		fixtureId: string,
		expectedFixtureRevision: number,
		expectedPatchRevision: number,
		expectedShowRevision: number,
		requestId: string,
		multipatchInstanceId: string | null,
		action: PatchFixtureUpdateAction,
	): Promise<PatchMutationOutcome>;
	subscribe(
		showId: string,
		afterSequence: number,
		observer: PatchEventObserver,
	): PatchEventStream;
}

export class PatchTransportError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly currentRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
		this.name = "PatchTransportError";
	}
}
