import type {
	PatchChange,
	PatchEventObserver,
	PatchEventStream,
	PatchMutation,
	PatchMutationOutcome,
	PatchSnapshot,
	PatchTransport,
} from "@tosklight/patch";
import { PatchTransportError } from "@tosklight/patch";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PATCH_CHANGE_EVENT } from "./session";

/**
 * The patch authority for a planning document.
 *
 * The desk reaches its patch over HTTP because many surfaces share one authority and any of them
 * may change it. Here the authority is the Rust session, and the surfaces are this application's
 * windows: an operator may have two open on the same document. The mutation outcome answers the
 * window that made the edit, and the stream below carries the same change to the others, so a
 * second window is a reader of the same rig rather than a stale copy of it.
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
				placements: mutation.placements ?? [],
			},
		});
	}

	subscribe(
		_showId: string,
		afterSequence: number,
		observer: PatchEventObserver,
	): PatchEventStream {
		let closed = false;
		let unlisten: UnlistenFn | undefined;
		// Deliver asynchronously so a subscriber that reads its own stream synchronously during
		// setup behaves the same here as against a real socket.
		queueMicrotask(() => {
			if (!closed) observer.message({ type: "ready", cursor: afterSequence });
		});
		listen<PatchChange>(PATCH_CHANGE_EVENT, (event) => {
			if (closed) return;
			const change = event.payload;
			// An edit made in this window never arrives here — the command that made it already
			// returned the outcome. A sequence the store cannot follow makes it re-read the
			// snapshot, so an event that overtakes another repairs itself.
			observer.message({
				type: "event",
				sequence: change.eventSequence ?? 0,
				change,
			});
		})
			.then((stop) => {
				if (closed) stop();
				else unlisten = stop;
			})
			.catch((reason) => {
				if (!closed) observer.error(new Error(String(reason)));
			});
		return {
			repair: () => undefined,
			close: () => {
				closed = true;
				unlisten?.();
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
