import type { CueList, PlaybackDefinition } from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import {
	playbackTopologyTransportFailure,
	repairPlaybackTopologyConflict,
} from "./conflictRepair";
import type {
	ExistingPlaybackRevisionBasis,
	PlaybackTopologyAction,
	PlaybackTopologyActions,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyRevisionBasis,
	PlaybackTopologyTransport,
} from "./contracts";

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

/** Serializes each portable topology intent as one revision-checked Show action. */
export class PlaybackTopologyWriter implements PlaybackTopologyActions {
	private stopped = false;
	private tail: Promise<void> = Promise.resolve();

	constructor(private readonly options: PlaybackTopologyWriterOptions) {}

	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		return this.enqueue(() =>
			this.saveCueListNow(cueListId, expectedRevision, expectedObjectId, body),
		);
	}

	configureSlot(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		return this.enqueue(() =>
			this.configureSlotNow(page, slot, playback, revisionBasis),
		);
	}

	mapExistingPlayback(
		page: number,
		slot: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	) {
		return this.enqueue(() =>
			this.mapExistingPlaybackNow(page, slot, playbackNumber, revisionBasis),
		);
	}

	clearMappedPlayback(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		return this.enqueue(() =>
			this.clearMappedPlaybackNow(page, slot, revisionBasis),
		);
	}

	stop() {
		this.stopped = true;
	}

	private saveCueListNow(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		const snapshot = this.options.store.getSnapshot();
		if (!snapshot.readyCollections.has("cue_list"))
			return this.fail("Authoritative Cuelists are loading");
		return this.apply({
			type: "save_cue_list",
			cueListId,
			expectedRevision,
			expectedObjectId,
			body,
		});
	}

	private configureSlotNow(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions)
			return this.fail("Authoritative Playback topology is loading");
		return this.apply({
			type: "configure_slot",
			page,
			slot,
			...revisions,
			playback,
		});
	}

	private clearMappedPlaybackNow(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions)
			return this.fail("Authoritative Playback topology is loading");
		return this.apply({
			type: "clear_mapped_playback",
			page,
			slot,
			...revisions,
		});
	}

	private mapExistingPlaybackNow(
		page: number,
		slot: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	) {
		const revisions = this.existingPlaybackRevisions(
			page,
			playbackNumber,
			revisionBasis,
		);
		if (!revisions)
			return this.fail(
				`Authoritative Playback ${playbackNumber} is not available`,
			);
		return this.apply({
			type: "map_existing_playback",
			page,
			slot,
			playbackNumber,
			...revisions,
		});
	}

	private enqueue(operation: () => Promise<PlaybackTopologyOutcome | null>) {
		const result = this.tail.then(operation, operation);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private slotRevisions(page: number, slot: number) {
		const snapshot = this.options.store.getSnapshot();
		if (
			!snapshot.readyCollections.has("playback") ||
			!snapshot.readyCollections.has("playback_page")
		)
			return null;
		const pageObject = snapshot.playbackPages.find(
			(object) => object.body.number === page,
		);
		const playbackNumber = pageObject?.body.slots[String(slot)];
		const playback = snapshot.playbacks.find(
			(object) => object.body.number === playbackNumber,
		);
		return {
			expectedPageRevision: pageObject?.revision ?? 0,
			expectedPageObjectId: pageObject?.id ?? null,
			expectedPlaybackRevision: playback?.revision ?? 0,
			expectedPlaybackObjectId: playback?.id ?? null,
		};
	}

	private readySlotRevisions(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const current = this.slotRevisions(page, slot);
		return current ? (revisionBasis ?? current) : null;
	}

	private existingPlaybackRevisions(
		page: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	): ExistingPlaybackRevisionBasis | null {
		const snapshot = this.options.store.getSnapshot();
		if (
			!snapshot.readyCollections.has("playback") ||
			!snapshot.readyCollections.has("playback_page")
		)
			return null;
		if (revisionBasis) return revisionBasis;
		const pageObject = snapshot.playbackPages.find(
			(object) => object.body.number === page,
		);
		const source = snapshot.playbacks.find(
			(object) => object.body.number === playbackNumber,
		);
		if (!source || source.body.target.type !== "cue_list") return null;
		return {
			expectedPageRevision: pageObject?.revision ?? 0,
			expectedPageObjectId: pageObject?.id ?? null,
			expectedPlaybackRevision: source.revision,
			expectedPlaybackObjectId: source.id,
		};
	}

	private async apply(action: PlaybackTopologyAction) {
		if (this.stopped) return null;
		const snapshot = this.options.store.getSnapshot();
		const generation = snapshot.authorityGeneration;
		if (snapshot.showRevision == null)
			return this.fail("Authoritative Show revision is loading");
		const request = { requestId: crypto.randomUUID(), action };
		try {
			const outcome = await this.send(snapshot.showRevision, request);
			if (!this.isCurrent(generation)) return null;
			assertOutcome(request, outcome);
			this.install(outcome);
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.isCurrent(generation)) return null;
			const error = asError(reason);
			await repairPlaybackTopologyConflict(
				this.options,
				error,
				action,
				generation,
			);
			this.options.onError?.(error);
			return null;
		}
	}

	private async send(
		revision: number,
		request: PlaybackTopologyRequest,
	): Promise<PlaybackTopologyOutcome> {
		try {
			return await this.options.transport.apply(
				this.options.showId,
				revision,
				request,
			);
		} catch (reason) {
			if (!playbackTopologyTransportFailure(reason)?.retryable) throw reason;
			return this.options.transport.apply(
				this.options.showId,
				revision,
				request,
			);
		}
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

	private isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.options.store.getSnapshot().authorityGeneration === generation
		);
	}

	private fail(message: string): Promise<null> {
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
	const action = request.action;
	const resolution = outcome.resolution;
	if (action.type === "save_cue_list") {
		if (
			resolution.kind !== "cue_list" ||
			resolution.cueListId !== action.cueListId
		)
			throw new Error("Playback topology response Cuelist does not match");
		return;
	}
	if (
		resolution.kind !== "page_slot" ||
		resolution.page !== action.page ||
		resolution.slot !== action.slot
	)
		throw new Error("Playback topology response slot does not match");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
