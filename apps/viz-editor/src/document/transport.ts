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
import { listen } from "@tauri-apps/api/event";

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
		expectedPatchRevision: number,
		mutation: PatchMutation,
	): Promise<PatchMutationOutcome> {
		try {
			return await invoke<PatchMutationOutcome>("patch_fixtures", {
				expectedPatchRevision,
				mutation: {
					requestId: mutation.requestId,
					fixtures: mutation.fixtures,
					removeFixtureIds: mutation.removeFixtureIds,
				},
			});
		} catch (reason) {
			const message = String(reason);
			const conflict = /revision|stale|changed/i.test(message);
			throw new PatchTransportError(message, conflict ? 409 : 400, null, conflict);
		}
	}

	subscribe(
		_showId: string,
		afterSequence: number,
		observer: PatchEventObserver,
	): PatchEventStream {
		let closed = false;
		let newest = afterSequence;
		const unlisten = listen<{ sceneRevision: number }>("cad-scene-delta", () => {
			if (closed) return;
			// CAD commits through the same Patch authority. Asking the Patch session to repair from a
			// complete snapshot makes its rows adopt the committed transforms without inventing a
			// second fixture DTO in the web renderer.
			observer.message({
				type: "gap",
				afterSequence: newest,
				oldestAvailable: newest + 1,
				latestSequence: newest + 1,
			});
			newest += 1;
		});
		void unlisten.catch((reason) => observer.error(new Error(String(reason))));
		// Deliver asynchronously so a subscriber that reads its own stream synchronously during
		// setup behaves the same here as against a real socket.
		queueMicrotask(() => {
			if (!closed) observer.message({ type: "ready", cursor: 0 });
		});
		return {
			repair: (cursor) => {
				newest = cursor;
				observer.message({ type: "repaired", cursor });
			},
			close: () => {
				closed = true;
				void unlisten.then((stop) => stop());
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
