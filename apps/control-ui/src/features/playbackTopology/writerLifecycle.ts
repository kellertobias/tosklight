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
		// The generation is resolved when the operation actually runs: an authority
		// re-hydration of the same show between enqueue and run must not silently drop
		// an operator save. Only a store that no longer holds this writer's show (or a
		// stopped writer) refuses the write.
		const run = () => {
			const generation = this.runnableGeneration();
			return generation === null
				? Promise.resolve(null)
				: operation(generation);
		};
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

	private runnableGeneration(): number | null {
		if (this.stopped) return null;
		const snapshot = this.store.getSnapshot();
		return snapshot.showId === this.showId
			? snapshot.authorityGeneration
			: null;
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
