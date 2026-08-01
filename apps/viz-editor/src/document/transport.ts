import type {
	PatchEventObserver,
	PatchEventStream,
	PatchMutation,
	PatchMutationOutcome,
	PatchSnapshot,
	PatchTransport,
} from "@tosklight/patch";
import { PatchTransportError } from "@tosklight/patch";
import { invoke } from "@tauri-apps/api/core";

/**
 * The patch authority for a planning document.
 *
 * The desk reaches its patch over HTTP because many surfaces share one authority and any of them
 * may change it. This application is a single window over a single file: it is the only writer,
 * so the mutation outcome is the whole truth and there is nothing to stream. `subscribe` therefore
 * reports itself ready and then stays quiet, which is exactly what the patch store expects from a
 * source with no concurrent writers.
 */
export class TauriPatchTransport implements PatchTransport {
	async snapshot(_showId: string): Promise<PatchSnapshot> {
		return await call<PatchSnapshot>("patch_snapshot");
	}

	async patchFixtures(
		_showId: string,
		_expectedPatchRevision: number,
		mutation: PatchMutation,
	): Promise<PatchMutationOutcome> {
		return await call<PatchMutationOutcome>("patch_fixtures", {
			mutation: {
				requestId: mutation.requestId,
				fixtures: mutation.fixtures,
				removeFixtureIds: mutation.removeFixtureIds,
			},
		});
	}

	subscribe(
		_showId: string,
		_afterSequence: number,
		observer: PatchEventObserver,
	): PatchEventStream {
		let closed = false;
		// Deliver asynchronously so a subscriber that reads its own stream synchronously during
		// setup behaves the same here as against a real socket.
		queueMicrotask(() => {
			if (!closed) observer.message({ type: "ready", cursor: 0 });
		});
		return {
			repair: () => undefined,
			close: () => {
				closed = true;
				observer.closed();
			},
		};
	}
}

async function call<T>(
	command: string,
	args: Record<string, unknown> = {},
): Promise<T> {
	try {
		return (await invoke(command, args)) as T;
	} catch (reason) {
		// A planning document has no revisions in flight, so a rejected patch is never a
		// concurrency conflict the store should retry — it is a rule the operator has to see.
		throw new PatchTransportError(String(reason), 400, null, false);
	}
}
