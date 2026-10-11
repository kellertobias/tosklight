import type { CueList, PlaybackDefinition } from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import {
	playbackTopologyTransportFailure,
	repairPlaybackTopologyConflict,
} from "./conflictRepair";
import type {
	ExistingPlaybackPageRevisionBasis,
	PlaybackPageRevisionBasis,
	PlaybackAddress,
	PlaybackTopologyAction,
	PlaybackTopologyActions,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyTransport,
} from "./contracts";
import { existingPageRevisions, readyPageRevisions } from "./pageAuthority";
import { normalizePlaybackPageName } from "./pageNames";
import { PlaybackTopologyWriterLifecycle } from "./writerLifecycle";

export interface PlaybackTopologyWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: PlaybackTopologyTransport;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	onError?: (error: Error | null) => void;
}

/** Serializes page-backed portable assignments as revision-checked Show intents. */
export class PlaybackTopologyWriter implements PlaybackTopologyActions {
	private readonly lifecycle: PlaybackTopologyWriterLifecycle;
	constructor(private readonly options: PlaybackTopologyWriterOptions) {
		this.lifecycle = new PlaybackTopologyWriterLifecycle(
			options.showId,
			options.store,
			options.transport,
		);
	}
	stop() {
		this.lifecycle.stop();
	}
	createPage(
		page: number,
		basis?: PlaybackPageRevisionBasis,
		name = `Page ${page}`,
	) {
		return this.pageIntent("create_playback_page", page, name, basis);
	}
	renamePage(
		page: number,
		name: string,
		basis?: ExistingPlaybackPageRevisionBasis,
	) {
		return this.pageIntent("rename_playback_page", page, name, basis);
	}
	deletePage(page: number, basis?: ExistingPlaybackPageRevisionBasis) {
		return this.lifecycle.enqueue((generation) => {
			const revisions = existingPageRevisions(this.options.store, page, basis);
			return revisions
				? this.apply(
						{ type: "delete_playback_page", number: page, ...revisions },
						generation,
					)
				: this.fail(
						`Authoritative Playback Page ${page} is not available`,
						generation,
					);
		});
	}
	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.apply(
				{
					type: "save_cue_list",
					cueListId,
					expectedRevision,
					expectedObjectId,
					body,
				},
				generation,
			),
		);
	}
	undoCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.apply(
				{
					type: "undo_cue_list",
					cueListId,
					expectedRevision,
					expectedObjectId,
				},
				generation,
			),
		);
	}
	redoCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.apply(
				{
					type: "redo_cue_list",
					cueListId,
					expectedRevision,
					expectedObjectId,
				},
				generation,
			),
		);
	}
	assignPlayback(
		address: PlaybackAddress,
		definition: PlaybackDefinition,
		basis?: PlaybackPageRevisionBasis,
	) {
		return this.assignmentIntent("assign_playback", address, definition, basis);
	}
	updatePlayback(
		address: PlaybackAddress,
		definition: PlaybackDefinition,
		basis?: PlaybackPageRevisionBasis,
	) {
		return this.assignmentIntent("update_playback", address, definition, basis);
	}
	clearPlayback(address: PlaybackAddress, basis?: PlaybackPageRevisionBasis) {
		return this.lifecycle.enqueue((generation) => {
			const revisions = readyPageRevisions(
				this.options.store,
				address.page,
				basis,
			);
			return revisions
				? this.apply(
						{ type: "clear_playback", address, ...revisions },
						generation,
					)
				: this.fail("Authoritative Playback Pages are loading", generation);
		});
	}
	assignGroupMaster(
		groupObjectId: string,
		expectedGroupRevision: number,
		address: PlaybackAddress,
		basis?: PlaybackPageRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) => {
			const group = this.options.store
				.getSnapshot()
				.groups.find((object) => object.id === groupObjectId);
			if (!group || group.revision !== expectedGroupRevision)
				return this.fail(
					"Authoritative Group is stale or unavailable",
					generation,
				);
			const revisions = readyPageRevisions(
				this.options.store,
				address.page,
				basis,
			);
			return revisions
				? this.apply(
						{
							type: "assign_group_master",
							groupObjectId,
							expectedGroupRevision,
							address,
							...revisions,
						},
						generation,
					)
				: this.fail("Authoritative Playback Pages are loading", generation);
		});
	}
	private assignmentIntent(
		type: "assign_playback" | "update_playback",
		address: PlaybackAddress,
		definition: PlaybackDefinition,
		basis?: PlaybackPageRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) => {
			const revisions = readyPageRevisions(
				this.options.store,
				address.page,
				basis,
			);
			return revisions
				? this.apply({ type, address, definition, ...revisions }, generation)
				: this.fail("Authoritative Playback Pages are loading", generation);
		});
	}
	private pageIntent(
		type: "create_playback_page" | "rename_playback_page",
		number: number,
		name: string,
		basis?: PlaybackPageRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) => {
			const normalized = normalizePlaybackPageName(name);
			if (!normalized)
				return this.fail("Playback Page name is invalid", generation);
			const revisions = readyPageRevisions(this.options.store, number, basis);
			return revisions
				? this.apply(
						{ type, number, name: normalized, ...revisions },
						generation,
					)
				: this.fail("Authoritative Playback Pages are loading", generation);
		});
	}
	private async apply(
		action: PlaybackTopologyAction,
		generation: number,
		retried = false,
	): Promise<PlaybackTopologyOutcome | null> {
		if (!this.lifecycle.isCurrent(generation)) return null;
		const snapshot = this.options.store.getSnapshot();
		if (snapshot.showRevision == null)
			return this.fail("Authoritative Show revision is loading", generation);
		const request = { requestId: crypto.randomUUID(), action };
		try {
			const outcome = await this.lifecycle.send(
				snapshot.showRevision,
				request,
				generation,
			);
			if (!this.lifecycle.isCurrent(generation)) return null;
			assertOutcome(request, outcome);
			this.install(outcome);
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.lifecycle.isCurrent(generation)) return null;
			const error = asError(reason);
			await repairPlaybackTopologyConflict(
				this.options,
				error,
				action,
				generation,
			);
			if (!this.lifecycle.isCurrent(generation)) return null;
			// An unrelated write may bump the Show revision between this writer's snapshot
			// and its send. When the repaired authority shows the targeted objects still at
			// their expected revisions, only the Show revision was stale: retry once with
			// the fresh revision instead of dropping the operator's save.
			if (
				!retried &&
				playbackTopologyTransportFailure(error)?.status === 409 &&
				this.expectationsStillCurrent(action)
			) {
				return this.apply(action, generation, true);
			}
			this.options.onError?.(error);
			return null;
		}
	}

	private expectationsStillCurrent(action: PlaybackTopologyAction) {
		const snapshot = this.options.store.getSnapshot();
		if ("cueListId" in action) {
			const current = snapshot.cueLists.find(
				(object) => object.body.id === action.cueListId,
			);
			return (
				(current?.id ?? null) === action.expectedObjectId &&
				(current?.revision ?? 0) === action.expectedRevision
			);
		}
		if (action.type === "assign_group_master") {
			const group = snapshot.groups.find(
				(object) => object.id === action.groupObjectId,
			);
			if (!group || group.revision !== action.expectedGroupRevision)
				return false;
		}
		const number = "address" in action ? action.address.page : action.number;
		const page = snapshot.playbackPages.find(
			(object) => object.body.number === number,
		);
		return (
			(page?.id ?? null) === action.expectedPageObjectId &&
			(page?.revision ?? 0) === action.expectedPageRevision
		);
	}
	private install(outcome: PlaybackTopologyOutcome) {
		const installs = outcome.objects.map((object) => ({
			kind: object.kind,
			objectId: object.objectId,
			object:
				object.state === "deleted"
					? null
					: ({
							kind: object.kind,
							id: object.objectId,
							revision: object.objectRevision,
							updated_at: "",
							body: object.body,
						} as ShowObject),
		}));
		this.options.store.installObjects(
			this.options.showId,
			installs,
			outcome.status === "changed" ? outcome.eventSequence : null,
			outcome.showRevision,
			outcome.status === "changed" ? "seal" : "floor",
		);
	}

	private fail(message: string, generation: number): Promise<null> {
		if (this.lifecycle.isCurrent(generation))
			this.options.onError?.(new Error(message));
		return Promise.resolve(null);
	}
}

function assertOutcome(
	request: PlaybackTopologyRequest,
	outcome: PlaybackTopologyOutcome,
) {
	if (request.requestId !== outcome.requestId)
		throw new Error("Playback topology response request ID does not match");
	const { action } = request;
	const { resolution } = outcome;
	if ("cueListId" in action) {
		if (
			resolution.kind !== "cue_list" ||
			resolution.cueListId !== action.cueListId
		)
			throw new Error("Playback topology response Cuelist does not match");
	} else if ("address" in action) {
		if (
			resolution.kind !== "playback" ||
			resolution.page !== action.address.page ||
			resolution.position !== action.address.position
		)
			throw new Error("Playback topology response address does not match");
	} else if (resolution.kind !== "page" || resolution.page !== action.number) {
		throw new Error("Playback topology response Page does not match");
	}
}
function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
