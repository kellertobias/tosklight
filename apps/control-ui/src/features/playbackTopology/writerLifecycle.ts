import type { ShowObjectsStore } from "../showObjects/store";
import { playbackTopologyTransportFailure } from "./conflictRepair";
import type {
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyTransport,
} from "./contracts";

type QueuedOperation = (
	generation: number,
) => Promise<PlaybackTopologyOutcome | null>;

export class PlaybackTopologyWriterLifecycle {
	private stopped = false;
	private tail: Promise<void> = Promise.resolve();

	constructor(
		private readonly showId: string,
		private readonly store: ShowObjectsStore,
		private readonly transport: PlaybackTopologyTransport,
	) {}

	enqueue(operation: QueuedOperation) {
		const generation = this.store.getSnapshot().authorityGeneration;
		const run = () =>
			this.isCurrent(generation)
				? operation(generation)
				: Promise.resolve(null);
		const result = this.tail.then(run, run);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	stop() {
		this.stopped = true;
	}

	isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.store.getSnapshot().authorityGeneration === generation
		);
	}

	async send(
		revision: number,
		request: PlaybackTopologyRequest,
		generation: number,
	): Promise<PlaybackTopologyOutcome> {
		try {
			return await this.transport.apply(this.showId, revision, request);
		} catch (reason) {
			if (!playbackTopologyTransportFailure(reason)?.retryable) throw reason;
			if (!this.isCurrent(generation)) throw reason;
			return this.transport.apply(this.showId, revision, request);
		}
	}
}
